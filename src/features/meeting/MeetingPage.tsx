import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { AudioMeters } from "../../components/AudioMeters";
import { CaptureIndicator } from "../../components/CaptureIndicator";
import { AnswerCard } from "../../components/AnswerCard";
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
} from "../../types/domain";

type ChatItem =
  | { kind: "question"; item: DetectedQuestion }
  | { kind: "answer"; item: AnswerDraft };

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

function itemTime(item: ChatItem): number {
  return item.kind === "question" ? item.item.detectedAtMs : item.item.createdAtMs;
}

/** 主会议页：对话式界面——仅展示识别到的问题与流式答案（不展示每一条转写）。 */
export function MeetingPage() {
  const [sessionState, setSessionState] = useState<SessionState | null>(null);
  const [questions, setQuestions] = useState<DetectedQuestion[]>([]);
  const [answers, setAnswers] = useState<Record<string, AnswerDraft>>({});
  const [meters, setMeters] = useState<Meters>({});
  const [fontScale, setFontScale] = useState(1);
  const [copied, setCopied] = useState(false);
  const copyTimer = useRef<number | undefined>(undefined);
  const chatEndRef = useRef<HTMLDivElement | null>(null);

  useEffect(() => {
    void getSessionState().then(setSessionState);
    const unsubs = [
      onEvent(EVENTS.captureState, (p) => {
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
            createdAtMs: p.atMs,
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

  // 自动滚动到底部
  useEffect(() => {
    chatEndRef.current?.scrollIntoView?.({ behavior: "smooth", block: "end" });
  }, [questions.length, answers]);

  const messages: ChatItem[] = useMemo(() => {
    const items: ChatItem[] = [];
    for (const q of questions) items.push({ kind: "question", item: q });
    for (const a of Object.values(answers)) items.push({ kind: "answer", item: a });
    items.sort((x, y) => itemTime(x) - itemTime(y));
    return items;
  }, [questions, answers]);

  const currentQuestion = questions.length > 0 ? questions[questions.length - 1] : null;
  const currentAnswer = currentQuestion ? answers[currentQuestion.id] : undefined;

  const handleStart = useCallback(async () => {
    const s = await startSession();
    setSessionState(s);
  }, []);

  const handleStop = useCallback(async () => {
    const s = await stopSession();
    setSessionState(s);
  }, []);

  const handleCopy = useCallback(async () => {
    const text = currentAnswer?.shortAnswer ?? "";
    if (!text) return;
    await navigator.clipboard.writeText(text);
    setCopied(true);
    window.clearTimeout(copyTimer.current);
    copyTimer.current = window.setTimeout(() => setCopied(false), 2000);
  }, [currentAnswer]);

  const running = sessionState === "capturing" || sessionState === "starting";
  const pipeline = toPipelineState(sessionState);
  const captureSource: CaptureSource = running ? "system" : "none";

  return (
    <div className="meeting-page" data-testid="meeting-page">
      <header className="meeting-page__header">
        <div className="meeting-page__brand">
          <h1>会议助手</h1>
          <span className="meeting-page__ai" role="status" aria-label="AI 状态">
            {pipeline === "idle" && "AI 待机"}
            {pipeline === "capturing" && "AI 辅助运行中"}
            {pipeline === "error" && "AI 异常"}
          </span>
        </div>
        <div className="meeting-page__controls">
          <CaptureIndicator source={captureSource} />
          <AudioMeters system={meters.system} microphone={meters.microphone} />
          <button
            type="button"
            className="meeting-page__toggle"
            onClick={() => void (running ? handleStop() : handleStart())}
          >
            {running ? "停止会话" : "开始会话"}
          </button>
        </div>
      </header>

      <main className="chat-area" data-testid="chat-area">
        {messages.length === 0 ? (
          <div className="chat-area__empty">
            <p className="chat-area__empty-title">等待识别问题…</p>
            <p className="chat-area__empty-hint">
              点击「开始会话」并播放会议音频，识别到问题后会自动生成答案
            </p>
          </div>
        ) : (
          messages.map((msg, i) =>
            msg.kind === "question" ? (
              <div
                key={`q-${msg.item.id}`}
                className="chat-item chat-item--question"
                data-level={msg.item.level}
              >
                <span className="chat-item__who">面试官</span>
                <div className="chat-item__bubble">
                  {msg.item.normalizedText}
                  {msg.item.level === "maybe" && (
                    <button
                      type="button"
                      className="chat-item__generate"
                      onClick={() => void generateAnswer()}
                    >
                      生成答案
                    </button>
                  )}
                </div>
              </div>
            ) : (
              <div
                key={`a-${msg.item.questionId}-${i}`}
                className="chat-item chat-item--answer"
                data-status={msg.item.status}
              >
                <span className="chat-item__who">助手</span>
                <AnswerCard
                  question={
                    questions.find((q) => q.id === msg.item.questionId) ?? {
                      id: msg.item.questionId,
                      sourceSegmentIds: [],
                      normalizedText: "",
                      confidence: 0,
                      detectedAtMs: msg.item.createdAtMs,
                      level: "auto",
                    }
                  }
                  answer={msg.item}
                  copied={copied}
                  hideQuestion
                  onCancel={() => void cancelCurrentAnswer()}
                  onRegenerate={() => void generateAnswer()}
                  onCopy={() => void handleCopy()}
                  onPin={() => void pinCurrentAnswer()}
                  onFontSize={(d) =>
                    setFontScale((f) => Math.min(1.6, Math.max(0.8, f + d * 0.1)))
                  }
                />
              </div>
            ),
          )
        )}
        <div ref={chatEndRef} />
      </main>

      <footer className="meeting-page__footer" style={{ fontSize: `${fontScale}em` }}>
        <span className="meeting-page__footer-hint">
          {running ? "正在聆听会议音频…" : "会话已停止"}
        </span>
      </footer>
    </div>
  );
}
