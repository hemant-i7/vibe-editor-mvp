import { bundle } from "@remotion/bundler";
import path from "path";
import { fileURLToPath } from "url";
import { renderMedia, selectComposition } from "@remotion/renderer";

const __dirname = path.dirname(fileURLToPath(import.meta.url));

const [inputPath, outputPath, durationSec] = process.argv.slice(2);
if (!inputPath || !outputPath) {
  console.error("Usage: node render.mjs <inputVideoPath> <outputPath> [durationSeconds]");
  process.exit(1);
}

const durationInFrames = Math.ceil((Number(durationSec) || 30) * 30);
const root = path.join(__dirname, "Root.tsx");

(async () => {
  const bundleLocation = await bundle({
    entryPoint: root,
    onProgress: () => {},
  });

  const videoSrc =
    inputPath.startsWith("file://") ? inputPath : `file://${path.resolve(inputPath)}`;
  const inputProps = { videoPath: videoSrc, durationInFrames };

  const composition = await selectComposition({
    serveUrl: bundleLocation,
    id: "VibeOverlay",
    inputProps,
  });

  await renderMedia({
    composition: { ...composition, durationInFrames },
    serveUrl: bundleLocation,
    codec: "h264",
    outputLocation: outputPath,
    inputProps,
  });

  console.log("Rendered:", outputPath);
})().catch((err) => {
  console.error(err);
  process.exit(1);
});
