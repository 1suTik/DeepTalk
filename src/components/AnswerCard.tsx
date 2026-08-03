import { Check, Copy, Pin, RefreshCw, Square, ZoomIn, ZoomOut } from "lucide-react";
import type { AnswerDraft, DetectedQuestion } from "../types/domain";

export interface AnswerCardProps {
  question: DetectedQuestion;
  answer: AnswerDraft;
  onCancel?: () => void;
  onRegenerate?: () => void;
  onCopy?: () => void;
  onPin?: () => void;
  onFontSize?: (delta: number) => void;
  copied?: boolean;
  /** 对话流中问题气泡已展示问题文本，答案卡不再重复显示标题。 */
  hideQuestion?: boolean;
}

/** 答案卡：流式短答 + 折叠的要点/追问 + 操作按钮。 */
export function AnswerCard({
  question,
  answer,
  onCancel,
  onRegenerate,
  onCopy,
  onPin,
  onFontSize,
  copied = false,
  hideQuestion = false,
}: AnswerCardProps) {
  const busy = answer.status === "streaming";
  return (
    <section className="answer-card" data-testid="answer-card" data-status={answer.status}>
      {!hideQuestion && (
        <header className="answer-card__header">
          <h3 className="answer-card__question">{question.normalizedText}</h3>
          <span className="answer-card__badge">
            {question.level === "maybe" ? "可能的问题" : "已识别"}
          </span>
        </header>
      )}

      <div className="answer-card__toolbar" aria-label="答案操作">
        <button
          type="button"
          className="answer-card__btn"
          title={busy ? "取消生成" : "已结束"}
          disabled={!busy}
          onClick={onCancel}
        >
          <Square size={14} /> 取消
        </button>
        <button
          type="button"
          className="answer-card__btn"
          title="重新生成"
          onClick={onRegenerate}
        >
          <RefreshCw size={14} /> 重新生成
        </button>
        <button
          type="button"
          className="answer-card__btn"
          title="复制短答"
          onClick={onCopy}
        >
          {copied ? <Check size={14} /> : <Copy size={14} />} {copied ? "已复制" : "复制"}
        </button>
        <button
          type="button"
          className="answer-card__btn"
          title="固定此答案（后续新问题进入等待队列）"
          onClick={onPin}
        >
          <Pin size={14} /> 固定记录
        </button>
        <button
          type="button"
          className="answer-card__btn"
          title="增大字体"
          onClick={() => onFontSize?.(1)}
        >
          <ZoomIn size={14} />
        </button>
        <button
          type="button"
          className="answer-card__btn"
          title="减小字体"
          onClick={() => onFontSize?.(-1)}
        >
          <ZoomOut size={14} />
        </button>
      </div>

      <p
        className="answer-card__short"
        data-streaming={busy}
        data-status={answer.status}
        role="status"
        aria-label="短答"
      >
        {answer.shortAnswer || (busy ? "正在生成…" : "（无内容）")}
        {answer.status === "failed" && " — 生成失败，可重新生成"}
      </p>

      {answer.keyPoints.length > 0 && (
        <details className="answer-card__details">
          <summary>要点（{answer.keyPoints.length}）</summary>
          <ul>
            {answer.keyPoints.map((k, i) => (
              <li key={i}>{k}</li>
            ))}
          </ul>
        </details>
      )}
      {answer.followUps.length > 0 && (
        <details className="answer-card__details">
          <summary>可能追问（{answer.followUps.length}）</summary>
          <ul>
            {answer.followUps.map((k, i) => (
              <li key={i}>{k}</li>
            ))}
          </ul>
        </details>
      )}
    </section>
  );
}
