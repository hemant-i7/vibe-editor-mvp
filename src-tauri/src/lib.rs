use serde::{Deserialize, Serialize};
use sqlx::{sqlite::SqliteConnectOptions, SqlitePool};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::str::FromStr;
use tauri::{Manager, State};

const SCHEMA_SQL: &str = include_str!("../sql/schema.sql");

#[derive(Clone)]
struct Db(SqlitePool);

#[derive(Serialize)]
struct VibeEditResult {
  output_path: String,
  filters: Vec<String>,
  used_gemini: bool,
  trial_watermark: bool,
}

#[derive(Deserialize)]
struct GeminiResponse {
  candidates: Vec<GeminiCandidate>,
}

#[derive(Deserialize)]
struct GeminiCandidate {
  content: GeminiContent,
}

#[derive(Deserialize)]
struct GeminiContent {
  parts: Vec<GeminiPart>,
}

#[derive(Deserialize)]
struct GeminiPart {
  text: String,
}

fn ensure_three_filters(mut filters: Vec<String>) -> Vec<String> {
  while filters.len() < 3 {
    filters.push("hue=s=1".to_string());
  }
  filters.truncate(3);
  filters
}

fn drawtext_font() -> &'static str {
  #[cfg(target_os = "macos")]
  return "fontfile=/System/Library/Fonts/Supplemental/Arial.ttf";
  #[cfg(not(target_os = "macos"))]
  return "fontfile=FreeSerif.ttf";
}

fn watermark_filter() -> String {
  format!(
    "drawtext={}:text='TRIAL':x=16:y=16:fontsize=24:fontcolor=white",
    drawtext_font()
  )
}

fn video_duration_seconds(path: &Path) -> Result<f64, String> {
  let out = Command::new("ffprobe")
    .args([
      "-v",
      "error",
      "-show_entries",
      "format=duration",
      "-of",
      "default=noprint_wrappers=1:nokey=1",
      path.to_str().unwrap_or(""),
    ])
    .output()
    .map_err(|e| e.to_string())?;
  if !out.status.success() {
    return Err(String::from_utf8_lossy(&out.stderr).to_string());
  }
  let s = String::from_utf8_lossy(&out.stdout);
  s.trim().parse::<f64>().map_err(|_| "invalid duration".to_string())
}

fn wants_overlay(prompt: &str) -> bool {
  let p = prompt.to_lowercase();
  p.contains("add animation")
    || p.contains("animation in between")
    || p.contains("transparent overlay")
    || p.contains("overlay")
}

fn fallback_filters(prompt: &str) -> Vec<String> {
  let prompt = prompt.to_lowercase();
  if prompt.contains("energetic") || prompt.contains("fast") {
    vec![
      "setpts=0.85*PTS".to_string(),
      "hue=s=1.25".to_string(),
      format!("drawtext={}:text='VIBE: ENERGETIC':x=16:y=16:fontsize=24:fontcolor=white", drawtext_font()),
    ]
  } else if prompt.contains("chill") || prompt.contains("calm") {
    vec![
      "setpts=1.05*PTS".to_string(),
      "hue=s=0.8".to_string(),
      format!("drawtext={}:text='VIBE: CHILL':x=16:y=16:fontsize=24:fontcolor=white", drawtext_font()),
    ]
  } else {
    vec![
      "setpts=1.0*PTS".to_string(),
      "hue=s=1.0".to_string(),
      format!("drawtext={}:text='VIBE: ACTION':x=16:y=16:fontsize=24:fontcolor=white", drawtext_font()),
    ]
  }
}

fn gemini_filters(prompt: &str) -> Result<Vec<String>, String> {
  let api_key =
    std::env::var("GEMINI_API_KEY").map_err(|_| "GEMINI_API_KEY not set".to_string())?;
  let request_body = serde_json::json!({
    "contents": [
      {
        "parts": [
          {
            "text": format!(
              "Return ONLY JSON: {{\"filters\":[\"ffmpeg_filter_1\",\"ffmpeg_filter_2\",\"ffmpeg_filter_3\"]}} for this prompt: {}",
              prompt
            )
          }
        ]
      }
    ]
  });

  let output = Command::new("curl")
    .arg("-sS")
    .arg("-H")
    .arg("Content-Type: application/json")
    .arg("-H")
    .arg(format!("x-goog-api-key: {}", api_key))
    .arg("-d")
    .arg(request_body.to_string())
    .arg("https://generativelanguage.googleapis.com/v1beta/models/gemini-2.5-flash:generateContent")
    .output()
    .map_err(|e| e.to_string())?;

  if !output.status.success() {
    return Err(String::from_utf8_lossy(&output.stderr).to_string());
  }

  let response: GeminiResponse =
    serde_json::from_slice(&output.stdout).map_err(|e| e.to_string())?;
  let text = response
    .candidates
    .get(0)
    .and_then(|c| c.content.parts.get(0))
    .map(|p| p.text.clone())
    .ok_or_else(|| "Gemini response missing text".to_string())?;

  let json_value: serde_json::Value =
    serde_json::from_str(&text).map_err(|e| e.to_string())?;
  let filters = json_value["filters"]
    .as_array()
    .ok_or_else(|| "Gemini JSON missing filters".to_string())?
    .iter()
    .filter_map(|f| f.as_str().map(|s| s.to_string()))
    .collect::<Vec<_>>();

  Ok(filters)
}

async fn init_db(db_path: &PathBuf) -> Result<SqlitePool, sqlx::Error> {
  let mut opts = SqliteConnectOptions::from_str("sqlite://")?;
  opts = opts.filename(db_path).create_if_missing(true);
  let pool = SqlitePool::connect_with(opts).await?;
  sqlx::query(SCHEMA_SQL).execute(&pool).await?;
  Ok(pool)
}

async fn is_license_valid(key: &str, db: &SqlitePool) -> Result<bool, String> {
  let row = sqlx::query(
    "SELECT license_key FROM licenses WHERE license_key = ? AND valid = 1 LIMIT 1",
  )
  .bind(key)
  .fetch_optional(db)
  .await
  .map_err(|e| e.to_string())?;
  Ok(row.is_some())
}

#[tauri::command]
async fn check_license(key: String, db: State<'_, Db>) -> Result<bool, String> {
  is_license_valid(&key, &db.0).await
}

#[tauri::command]
async fn vibe_edit(
  input_path: String,
  prompt: String,
  license_key: Option<String>,
  add_overlay: Option<bool>,
  db: State<'_, Db>,
) -> Result<VibeEditResult, String> {
  let licensed = if let Some(key) = license_key {
    is_license_valid(&key, &db.0).await?
  } else {
    false
  };

  let (filters, used_gemini) = match gemini_filters(&prompt) {
    Ok(filters) => (filters, true),
    Err(_) => (fallback_filters(&prompt), false),
  };

  let mut filters = ensure_three_filters(filters);
  let trial_watermark = !licensed;
  if trial_watermark {
    if filters.len() >= 3 {
      filters[2] = watermark_filter();
    } else {
      filters.push(watermark_filter());
    }
  }

  let input = std::path::PathBuf::from(&input_path);
  let output = input
    .with_file_name("vibe_output.mp4")
    .to_string_lossy()
    .to_string();

  let filter_desc = filters.join(",");
  let ffmpeg_out = Command::new("ffmpeg")
    .arg("-y")
    .arg("-i")
    .arg(&input_path)
    .arg("-vf")
    .arg(&filter_desc)
    .arg("-c:v")
    .arg("libx264")
    .arg("-preset")
    .arg("veryfast")
    .arg("-c:a")
    .arg("aac")
    .arg(&output)
    .output()
    .map_err(|e| e.to_string())?;

  if !ffmpeg_out.status.success() {
    let stderr = String::from_utf8_lossy(&ffmpeg_out.stderr);
    let msg = if stderr.is_empty() {
      "FFmpeg failed (no stderr). Check ffmpeg is installed and path is valid."
        .to_string()
    } else {
      format!("FFmpeg failed: {}", stderr.lines().take(5).collect::<Vec<_>>().join(" "))
    };
    return Err(msg);
  }

  let mut final_output = output.clone();

  let run_overlay = add_overlay.unwrap_or_else(|| wants_overlay(&prompt));
  if run_overlay {
    let duration_sec = video_duration_seconds(Path::new(&output)).unwrap_or(30.0);
    let overlay_out = input
      .with_file_name("vibe_output_overlay.mp4")
      .to_string_lossy()
      .to_string();
    let project_root = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let script = project_root.join("remotion").join("render.mjs");
    if script.exists() {
      let node_out = Command::new("node")
        .arg(script)
        .arg(&output)
        .arg(&overlay_out)
        .arg(format!("{:.2}", duration_sec))
        .current_dir(&project_root)
        .output()
        .map_err(|e| e.to_string())?;
      if node_out.status.success() {
        final_output = overlay_out;
      }
    }
  }

  sqlx::query(
    "INSERT INTO projects (input_path, output_path, prompt) VALUES (?, ?, ?)",
  )
  .bind(&input_path)
  .bind(&final_output)
  .bind(&prompt)
  .execute(&db.0)
  .await
  .map_err(|e| e.to_string())?;

  Ok(VibeEditResult {
    output_path: final_output,
    filters,
    used_gemini,
    trial_watermark,
  })
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
  tauri::Builder::default()
    .setup(|app| {
      if cfg!(debug_assertions) {
        app.handle().plugin(
          tauri_plugin_log::Builder::default()
            .level(log::LevelFilter::Info)
            .build(),
        )?;
      }
      let db_path = app
        .path()
        .app_data_dir()
        .expect("failed to resolve app data dir")
        .join("vibe.db");
      if let Some(parent) = db_path.parent() {
        std::fs::create_dir_all(parent).expect("failed to create app data dir");
      }
      let pool = tauri::async_runtime::block_on(init_db(&db_path))
        .expect("failed to initialize database");
      app.manage(Db(pool));
      Ok(())
    })
    .plugin(tauri_plugin_dialog::init())
    .invoke_handler(tauri::generate_handler![vibe_edit, check_license])
    .run(tauri::generate_context!())
    .expect("error while running tauri application");
}
