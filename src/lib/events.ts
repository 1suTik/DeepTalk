import type {
  AnswerDraft,
  CaptureSource,
  DetectedQuestion,
  TranscriptSegment,
} from "../types/domain";

export const EVENTS = {
  captureState: "capture-state",
  audioLevel: "audio-level",
  transcriptPending: "transcript-pending",
  transcriptFinal: "transcript-final",
  questionDetected: "question-detected",
  answerStarted: "answer-started",
  answerDelta: "answer-delta",
  answerCompleted: "answer-completed",
} as const;

export interface EventPayloads {
  "capture-state": { source: CaptureSource; active: boolean; atMs: number };
  "audio-level": { source: CaptureSource; rms: number; peak: number; atMs: number };
  "transcript-pending": TranscriptSegment;
  "transcript-final": TranscriptSegment;
  "question-detected": DetectedQuestion;
  "answer-started": { questionId: string; atMs: number };
  "answer-delta": { questionId: string; delta: string };
  "answer-completed": AnswerDraft;
}
