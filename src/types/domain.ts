/** 前后端领域契约：仅通过稳定事件传输领域数据，不暴露数据库或音频实现细节。 */

export type Speaker = "remote" | "local";

export type PipelineState =
  | "idle"
  | "capturing"
  | "transcribing"
  | "generating"
  | "error";

/** Rust SessionState 的镜像（serde lowercase + Failed 结构体变体）。 */
export type SessionState =
  | "idle"
  | "starting"
  | "capturing"
  | "stopping"
  | { failed: { message: string } };

export type CaptureSource = "none" | "system" | "microphone" | "both";

export interface TranscriptSegment {
  id: string;
  speaker: Speaker;
  text: string;
  startedAtMs: number;
  endedAtMs: number;
  isFinal: boolean;
}

export interface DetectedQuestion {
  id: string;
  sourceSegmentIds: string[];
  normalizedText: string;
  confidence: number;
  detectedAtMs: number;
}

export interface AnswerDraft {
  questionId: string;
  shortAnswer: string;
  keyPoints: string[];
  followUps: string[];
  status: "streaming" | "complete" | "cancelled" | "failed";
}

/** 设置页领域模型（与 Rust AppSettings 对齐，API Key 永不回传）。 */
export interface AppSettings {
  providerKind: "deepseek" | "openai" | "custom";
  baseUrl: string;
  model: string;
  hasApiKey: boolean;
  retentionDays: number;
  microphoneEnabled: boolean;
  asrModelId: string;
}
