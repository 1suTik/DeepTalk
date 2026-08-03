import type { CaptureSource } from "../types/domain";

const LABELS: Record<CaptureSource, string> = {
  none: "未采集",
  system: "系统音频采集中",
  microphone: "麦克风采集中",
  both: "系统音频与麦克风采集中",
};

export function CaptureIndicator({ source }: { source: CaptureSource }) {
  const active = source !== "none";
  return (
    <span
      className="capture-indicator"
      role="status"
      data-active={active}
      aria-label={LABELS[source]}
    >
      <span className="capture-indicator__dot" aria-hidden="true" />
      {LABELS[source]}
    </span>
  );
}
