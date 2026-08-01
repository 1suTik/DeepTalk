import { CaptureIndicator } from "../../components/CaptureIndicator";
import type { CaptureSource, PipelineState } from "../../types/domain";

function aiStatusLabel(state: PipelineState): string {
  switch (state) {
    case "idle":
      return "AI 待机";
    case "capturing":
    case "transcribing":
    case "generating":
      return "AI 辅助运行中";
    case "error":
      return "AI 异常";
  }
}

export interface OverlayPageProps {
  initialState: PipelineState;
  captureSource?: CaptureSource;
}

/** 始终置顶的会议面板：标题栏持续显示 AI 与采集状态。 */
export function OverlayPage({
  initialState,
  captureSource = initialState === "idle" || initialState === "error"
    ? "none"
    : "system",
}: OverlayPageProps) {
  return (
    <div className="overlay" data-testid="overlay">
      <header className="overlay__statusbar">
        <span className="overlay__ai-status" role="status" aria-label="AI 状态">
          {aiStatusLabel(initialState)}
        </span>
        <CaptureIndicator source={captureSource} />
      </header>
    </div>
  );
}
