import { useCallback, useEffect, useRef, useState } from "react";
import { AnswerCard } from "../../components/AnswerCard";
import { AudioMeters } from "../../components/AudioMeters";
import { CaptureIndicator } from "../../components/CaptureIndicator";
import { TranscriptFeed } from "../../components/TranscriptFeed";
import { EVENTS } from "../../lib/events";
import {
  cancelCurrentAnswer,
  generateAnswer,
  getSessionState,
  onEvent,
  pinCurrentAnswer,
  startSession,
  stopSession,
} from "../../lib/tauri";
import type {
  AnswerDraft,
  CaptureSource,
  DetectedQuestion,
  PipelineState,
  SessionState,
  Speaker,
  TranscriptSegment,
} from "../../types/domain";

interface Meters {
  system?: { rms: number; peak: number };
  microphone?: { rms: number; peak: number };
}

function toPipelineState(session: SessionState | null): PipelineState {
  if (!session) return "idle";
  if (session === "capturing") return "capturing";
  if (typeof session === "object" && "failed" in session) return "error";
  return "idle";
}

let segmentSeq = 0;

/** 主会议页：开始/停止会话，实时展示音量、转写、问题与流式答案。 */
export function MeetingPage() {
  const [sessionState, setSessionState] = useState<SessionState | null>(null);
  const [segments, setSegments] = useState<TranscriptSegment[]>([]);
  const [questions, setQuestions] = useState<DetectedQuestion[]>([]);
  const [answers, setAnswers] = useState<Record<string, AnswerDraft>>({});
  const [meters, setMeters] = useState<Meters>({});
  const [fontScale, setFontScale] = useState(1);
  const [copied, setCopied] = useState(false);
  const currentQuestionRef = useRef<DetectedQuestion | null>(null);
  const copyTimer = useRef<number | undefined>(undefined);

  useEffect(() => {
    void getSessionState().then(setSessionState);
    const unsubs = [
      onEvent(EVENTS.captureState, (p) => {
        // 采集停止即回到待机（与 stop_session 的返回值一致）
        if (!p.active) {
          setSessionState("idle");
        }
      }),
      onEvent(EVENTS.audioLevel, (p) => {
        setMeters((m) => ({
          ...m,
          [p.source]: { rms: p.rms, peak: p.peak },
        }));
      }),
      onEvent(EVENTS.transcriptPending, (p) => {
        setSegments((list) => [...list, { ...p, isFinal: false }]);
      }),
      onEvent(EVENTS.transcriptFinal, (p) => {
        const id = p.id || `seg-${Date.now()}-${++segmentSeq}`;
        setSegments((list) => [
          ...list,
          { ...p, id, speaker: p.speaker as Speaker, isFinal: true },
        ]);
      }),
      onEvent(EVENTS.questionDetected, (q) => {
        setQuestions((list) => [...list, q]);
      }),
      onEvent(EVENTS.answerStarted, (p) => {
        setAnswers((map) => ({
          ...map,
          [p.questionId]: {
            questionId: p.questionId,
            shortAnswer: "",
            keyPoints: [],
            followUps: [],
            status: "streaming",
          },
        }));
      }),
      onEvent(EVENTS.answerDelta, (p) => {
        setAnswers((map) => {
          const prev = map[p.questionId];
          if (!prev) return map;
          return {
            ...map,
            [p.questionId]: { ...prev, shortAnswer: prev.shortAnswer + p.delta },
          };
        });
      }),
      onEvent(EVENTS.answerCompleted, (draft) => {
        setAnswers((map) => ({ ...map, [draft.questionId]: draft }));
      }),
    ];
    return () => unsubs.forEach((u) => u());
  }, []);

  const handleStart = useCallback(async () => {
    const s = await startSession();
    setSessionState(s);
  }, []);

  const handleStop = useCallback(async () => {
    const s = await stopSession();
    setSessionState(s);
  }, []);

  const handleCopy = useCallback(async () => {
    const current = currentQuestionRef.current;
    const text = current ? answers[current.id]?.shortAnswer : "";
    if (!text) return;
    await navigator.clipboard.writeText(text);
    setCopied(true);
    window.clearTimeout(copyTimer.current);
    copyTimer.current = window.setTimeout(() => setCopied(false), 2000);
  }, [answers]);

  const currentQuestion =
    questions.length > 0 ? questions[questions.length - 1] : null;
  currentQuestionRef.current = currentQuestion;
  const currentAnswer = currentQuestion ? answers[currentQuestion.id] : undefined;

  const running = sessionState === "capturing" || sessionState === "starting";
  const pipeline = toPipelineState(sessionState);
  const captureSource: CaptureSource = running ? "system" : "none";

  return (
    <div className="meeting-page" data-testid="meeting-page">
      <header className="meeting-page__header">
        <h1>会议助手</h1>
        <span className="meeting-page__ai" role="status" aria-label="AI 状态">
          {pipeline === "idle" && "AI 待机"}
          {pipeline === "capturing" && "AI 辅助运行中"}
          {pipeline === "error" && "AI 异常"}
        </span>
        <CaptureIndicator source={captureSource} />
        <button
          type="button"
          className="meeting-page__toggle"
          onClick={() => void (running ? handleStop() : handleStart())}
        >
          {running ? "停止会话" : "开始会话"}
        </button>
      </header>

      <AudioMeters system={meters.system} microphone={meters.microphone} />

      {currentQuestion && currentAnswer && (
        <AnswerCard
          question={currentQuestion}
          answer={currentAnswer}
          copied={copied}
          onCancel={() => void cancelCurrentAnswer()}
          onRegenerate={() => void generateAnswer()}
          onCopy={() => void handleCopy()}
          onPin={() => void pinCurrentAnswer()}
          onFontSize={(d) => setFontScale((f) => Math.min(1.6, Math.max(0.8, f + d * 0.1)))}
        />
      )}
      {currentQuestion && !currentAnswer && (
        <p className="meeting-page__hint" data-testid="maybe-hint">
          检测到可能的问题：{currentQuestion.normalizedText}
          <button type="button" onClick={() => void generateAnswer()}>
            生成答案
          </button>
        </p>
      )}

      <div className="meeting-page__transcript" style={{ fontSize: `${fontScale}em` }}>
        <TranscriptFeed segments={segments} />
      </div>
    </div>
  );
}
