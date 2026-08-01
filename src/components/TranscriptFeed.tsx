import type { TranscriptSegment } from "../types/domain";

export interface TranscriptFeedProps {
  segments: TranscriptSegment[];
}

/** 转写流：临时文本与最终文本按来源标记。 */
export function TranscriptFeed({ segments }: TranscriptFeedProps) {
  if (segments.length === 0) {
    return <p className="transcript-feed__empty" data-testid="transcript-empty">暂无转写</p>;
  }
  return (
    <ul className="transcript-feed" data-testid="transcript-feed">
      {segments.map((seg) => (
        <li
          key={seg.id}
          className="transcript-feed__item"
          data-final={seg.isFinal}
          data-speaker={seg.speaker}
        >
          <span className="transcript-feed__speaker">
            {seg.speaker === "local" ? "本机" : "远端"}
          </span>
          <span className="transcript-feed__text">{seg.text}</span>
        </li>
      ))}
    </ul>
  );
}
