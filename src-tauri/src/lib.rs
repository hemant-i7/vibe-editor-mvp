use ez_ffmpeg::{FfmpegContext, FfmpegScheduler};
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use std::process::Command;
use tauri::State;

const DB_URL: &str = "sqlite://vibe.db";
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

fn watermark_filter() -> String {
  "drawtext=text='TRIAL':x=16:y=16:fontsize=24:fontcolor=white".to_string()
}

fn fallback_filters(prompt: &str) -> Vec<String> {
  let prompt = prompt.to_lowercase();
  if prompt.contains("energetic") || prompt.contains("fast") {
    vec![
      "setpts=0.85*PTS".to_string(),
      "hue=s=1.25".to_string(),
      "drawtext=text='VIBE: ENERGETIC':x=16:y=16:fontsize=24:fontcolor=white".to_string(),
    ]
  } else if prompt.contains("chill") || prompt.contains("calm") {
    vec![
      "setpts=1.05*PTS".to_string(),
      "hue=s=0.8".to_string(),
      "drawtext=text='VIBE: CHILL':x=16:y=16:fontsize=24:fontcolor=white".to_string(),
    ]
  } else {
    vec![
      "setpts=1.0*PTS".to_string(),
      "hue=s=1.0".to_string(),
      "drawtext=text='VIBE: ACTION':x=16:y=16:fontsize=24:fontcolor=white".to_string(),
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

async fn init_db() -> Result<SqlitePool, sqlx::Error> {
  let pool = SqlitePool::connect(DB_URL).await?;
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
  let context = FfmpegContext::builder()
    .input(&input_path)
    .filter_desc(&filter_desc)
    .output(&output)
    .build()
    .map_err(|e| e.to_string())?;

  FfmpegScheduler::new(context)
    .start()
    .map_err(|e| e.to_string())?
    .wait()
    .map_err(|e| e.to_string())?;

  sqlx::query(
    "INSERT INTO projects (input_path, output_path, prompt) VALUES (?, ?, ?)",
  )
  .bind(&input_path)
  .bind(&output)
  .bind(&prompt)
  .execute(&db.0)
  .await
  .map_err(|e| e.to_string())?;

  Ok(VibeEditResult {
    output_path: output,
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
      let pool =
        tauri::async_runtime::block_on(init_db()).expect("failed to initialize database");
      app.manage(Db(pool));
      Ok(())
    })
    .plugin(tauri_plugin_dialog::init())
    .invoke_handler(tauri::generate_handler![vibe_edit, check_license])
    .run(tauri::generate_context!())
    .expect("error while running tauri application");
}
