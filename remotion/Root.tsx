import React from "react";
import {
  AbsoluteFill,
  Composition,
  OffthreadVideo,
  useCurrentFrame,
  useVideoConfig,
} from "remotion";
import { getInputProps, registerRoot } from "remotion";

const FPS = 30;

function AnimatedOverlay() {
  const frame = useCurrentFrame();
  const { durationInFrames } = useVideoConfig();
  const opacity = 0.15 + 0.05 * Math.sin((frame / 20) * Math.PI * 2);
  return (
    <AbsoluteFill style={{ justifyContent: "flex-end", pointerEvents: "none" }}>
      <div
        style={{
          height: 120,
          width: "100%",
          background: `linear-gradient(180deg, transparent 0%, rgba(0,0,0,${opacity}) 100%)`,
        }}
      />
      <div
        style={{
          position: "absolute",
          bottom: 24,
          left: 24,
          right: 24,
          height: 48,
          background: "linear-gradient(90deg, rgba(244,63,94,0.25), rgba(251,146,60,0.25))",
          borderRadius: 12,
          transform: `scaleX(${0.7 + 0.05 * Math.sin((frame / 30) * Math.PI * 2)})`,
          transformOrigin: "left center",
        }}
      />
    </AbsoluteFill>
  );
}

function VibeOverlayComposition() {
  const inputProps = getInputProps() as { videoPath: string };
  const videoPath = inputProps?.videoPath ?? "";
  return (
    <AbsoluteFill>
      <OffthreadVideo src={videoPath} />
      <AnimatedOverlay />
    </AbsoluteFill>
  );
}

type Props = { videoPath: string; durationInFrames: number };

export const RemotionRoot: React.FC = () => {
  return (
    <>
      <Composition
        id="VibeOverlay"
        component={VibeOverlayComposition}
        durationInFrames={300}
        fps={FPS}
        width={1920}
        height={1080}
        defaultProps={{ videoPath: "", durationInFrames: 300 }}
        calculateMetadata={({ props }: { props: Props }) => ({
          durationInFrames: props.durationInFrames ?? 300,
        })}
      />
    </>
  );
};

registerRoot(RemotionRoot);
