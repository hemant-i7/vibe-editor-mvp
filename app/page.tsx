"use client";

import { convertFileSrc, invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import { useEffect, useMemo, useRef, useState } from "react";

type VibeEditResult = {
  output_path: string;
  filters: string[];
  used_gemini: boolean;
  trial_watermark: boolean;
};

const presets = [
  { label: "Energetic", prompt: "make energetic, punchy vibe" },
  { label: "Chill", prompt: "make chill, smooth vibe" },
  { label: "Action", prompt: "make action, bold vibe" },
];

export default function Home() {
  const [filePath, setFilePath] = useState("");
  const [videoSrc, setVideoSrc] = useState("");
  const [prompt, setPrompt] = useState("make energetic, punchy vibe");
  const [licenseKey, setLicenseKey] = useState("");
  const [addOverlay, setAddOverlay] = useState(false);
  const [result, setResult] = useState<VibeEditResult | null>(null);
  const [isProcessing, setIsProcessing] = useState(false);
  const [error, setError] = useState("");

  const videoRef = useRef<HTMLVideoElement | null>(null);
  const canvasRef = useRef<HTMLCanvasElement | null>(null);

  const canEdit = useMemo(() => filePath && prompt, [filePath, prompt]);

  useEffect(() => {
    const video = videoRef.current;
    const canvas = canvasRef.current;
    if (!video || !canvas) return;
    const context = canvas.getContext("2d");
    if (!context) return;

    const setSize = () => {
      canvas.width = video.videoWidth || 640;
      canvas.height = video.videoHeight || 360;
    };

    const drawFrame = () => {
      if (video.paused || video.ended) return;
      context.drawImage(video, 0, 0, canvas.width, canvas.height);
      if ("requestVideoFrameCallback" in video) {
        (video as HTMLVideoElement & { requestVideoFrameCallback: (cb: () => void) => void })
          .requestVideoFrameCallback(drawFrame);
      } else {
        requestAnimationFrame(drawFrame);
      }
    };

    const handlePlay = () => drawFrame();

    video.addEventListener("loadedmetadata", setSize);
    video.addEventListener("play", handlePlay);
    return () => {
      video.removeEventListener("loadedmetadata", setSize);
      video.removeEventListener("play", handlePlay);
    };
  }, [videoSrc]);

  const pickVideo = async () => {
    setError("");
    const selected = await open({
      multiple: false,
      filters: [{ name: "Video", extensions: ["mp4", "mov"] }],
    });
    if (typeof selected === "string") {
      setFilePath(selected);
      setVideoSrc(convertFileSrc(selected));
      setResult(null);
    }
  };

  const runEdit = async () => {
    if (!canEdit) return;
    setError("");
    setIsProcessing(true);
    try {
      const response = await invoke<VibeEditResult>("vibe_edit", {
        inputPath: filePath,
        prompt,
        licenseKey: licenseKey || null,
        addOverlay: addOverlay || null,
      });
      setResult(response);
    } catch (err) {
      setError(String(err));
    } finally {
      setIsProcessing(false);
    }
  };

  return (
    <div className="min-h-screen bg-[radial-gradient(circle_at_top,_#fef2f2,_#fff7ed_30%,_#f8fafc_70%)] text-slate-900">
      <div className="mx-auto flex max-w-5xl flex-col gap-8 px-6 py-12">
        <header className="flex flex-col gap-4">
          <p className="text-sm uppercase tracking-[0.4em] text-rose-500">
            Vibe Video Editor MVP
          </p>
          <h1 className="text-4xl font-semibold leading-tight md:text-5xl">
            AI-first video vibes for fast Mac edits.
          </h1>
          <p className="max-w-2xl text-lg text-slate-600">
            Drop a clip, describe the vibe, and export an MP4 in minutes. Runs
            locally with Gemini-assisted FFmpeg filters.
          </p>
        </header>

        <section className="grid gap-6 md:grid-cols-[1.1fr_0.9fr]">
          <div className="rounded-3xl border border-rose-100 bg-white/70 p-6 shadow-[0_20px_60px_-40px_rgba(15,23,42,0.6)]">
            <div className="flex flex-col gap-4">
              <button
                onClick={pickVideo}
                className="rounded-2xl border border-rose-200 bg-rose-50 px-4 py-3 text-sm font-semibold uppercase tracking-[0.2em] text-rose-600 transition hover:bg-rose-100"
              >
                Pick Video
              </button>
              <div className="rounded-2xl border border-dashed border-rose-200 bg-rose-50/60 p-4 text-sm text-rose-700">
                {filePath ? (
                  <p className="break-all">Loaded: {filePath}</p>
                ) : (
                  <p>MP4/MOV up to 5 minutes. Local files only.</p>
                )}
              </div>

              <div className="flex flex-wrap gap-2">
                {presets.map((preset) => (
                  <button
                    key={preset.label}
                    onClick={() => setPrompt(preset.prompt)}
                    className="rounded-full border border-slate-200 px-4 py-2 text-xs font-semibold uppercase tracking-[0.2em] text-slate-500 transition hover:border-rose-300 hover:text-rose-500"
                  >
                    {preset.label}
                  </button>
                ))}
              </div>

              <textarea
                value={prompt}
                onChange={(event) => setPrompt(event.target.value)}
                className="min-h-[120px] w-full rounded-2xl border border-slate-200 bg-white px-4 py-3 text-sm shadow-sm focus:border-rose-300 focus:outline-none"
                placeholder="Describe the vibe (e.g. energetic, add animation, transparent overlay)"
              />
              <label className="flex items-center gap-2 text-sm text-slate-600">
                <input
                  type="checkbox"
                  checked={addOverlay}
                  onChange={(e) => setAddOverlay(e.target.checked)}
                  className="rounded border-slate-300"
                />
                Add Remotion animation/overlay on output
              </label>

              <input
                value={licenseKey}
                onChange={(event) => setLicenseKey(event.target.value)}
                className="w-full rounded-2xl border border-slate-200 bg-white px-4 py-3 text-sm shadow-sm focus:border-rose-300 focus:outline-none"
                placeholder="License key (optional)"
              />

              <button
                disabled={!canEdit || isProcessing}
                onClick={runEdit}
                className="rounded-2xl bg-slate-900 px-4 py-4 text-sm font-semibold uppercase tracking-[0.2em] text-white transition hover:bg-slate-800 disabled:cursor-not-allowed disabled:bg-slate-400"
              >
                {isProcessing ? "Rendering..." : "AI Vibe Edit & Export"}
              </button>
              {error ? (
                <p className="rounded-2xl border border-rose-200 bg-rose-50 px-4 py-3 text-sm text-rose-600">
                  {error}
                </p>
              ) : null}
            </div>
          </div>

          <div className="flex flex-col gap-4 rounded-3xl border border-slate-100 bg-white/80 p-6">
            <div className="rounded-2xl border border-slate-200 bg-slate-950/90 p-3">
              <video
                ref={videoRef}
                src={videoSrc || undefined}
                controls
                className="h-64 w-full rounded-xl bg-black object-contain"
              />
            </div>
            <canvas
              ref={canvasRef}
              className="h-40 w-full rounded-2xl border border-slate-200 bg-white"
            />
            <div className="rounded-2xl border border-slate-100 bg-slate-50 px-4 py-3 text-xs uppercase tracking-[0.2em] text-slate-500">
              Canvas preview (frame capture)
            </div>
          </div>
        </section>

        <section className="rounded-3xl border border-slate-100 bg-white/80 p-6">
          <h2 className="text-lg font-semibold">Export Status</h2>
          <p className="mt-2 text-sm text-slate-600">
            Output is generated locally via FFmpeg and saved next to your input
            video as <span className="font-semibold">vibe_output.mp4</span>.
          </p>
          {result ? (
            <div className="mt-4 grid gap-3 text-sm text-slate-700">
              <div className="rounded-2xl border border-slate-200 bg-white px-4 py-3">
                Output: {result.output_path}
              </div>
              <div className="rounded-2xl border border-slate-200 bg-white px-4 py-3">
                Filters: {result.filters.join(" | ")}
              </div>
              <div className="rounded-2xl border border-slate-200 bg-white px-4 py-3">
                Gemini: {result.used_gemini ? "Used" : "Fallback"}{" "}
                {result.trial_watermark ? "• Trial watermark on" : ""}
              </div>
            </div>
          ) : (
            <p className="mt-4 text-sm text-slate-500">
              Run an edit to see the applied filters and export path.
            </p>
          )}
        </section>
      </div>
    </div>
  );
}
