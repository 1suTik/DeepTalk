import { CaptureIndicator } from "../../components/CaptureIndicator";
import type { AnswerDraft, CaptureSource, DetectedQuestion, PipelineState } from "../../types/domain";

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
  currentQuestion?: DetectedQuestion | null;
  currentAnswer?: AnswerDraft | null;
}

/** 始终置顶的会议面板：标题栏持续显示 AI 与采集状态，正文显示最新问题与流式短答。 */
export function OverlayPage({
  initialState,
  captureSource = initialState === "idle" || initialState === "error"
    ? "none"
    : "system",
  currentQuestion = null,
  currentAnswer = null,
}: OverlayPageProps) {
  return (
    <div className="overlay" data-testid="overlay">
      <header className="overlay__statusbar">
        <span className="overlay__ai-status" role="status" aria-label="AI 状态">
          {aiStatusLabel(initialState)}
        </span>
        <CaptureIndicator source={captureSource} />
      </header>
      {currentQuestion && (
        <div className="overlay__content" data-testid="overlay-content">
          <p className="overlay__question" title={currentQuestion.normalizedText}>
            {currentQuestion.normalizedText}
          </p>
          {currentAnswer && (
            <p
              className="overlay__answer"
              data-status={currentAnswer.status}
              role="status"
              aria-label="短答"
            >
              {currentAnswer.shortAnswer || "正在生成…"}
            </p>
          )}
        </div>
      )}
    </div>
  );
}
