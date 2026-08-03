import { useEffect, useState } from "react";
import { EVENTS } from "../../lib/events";
import { getSessionState, onEvent } from "../../lib/tauri";
import type {
  AnswerDraft,
  DetectedQuestion,
  SessionState,
} from "../../types/domain";

export interface Meters {
  system?: { rms: number; peak: number };
  microphone?: { rms: number; peak: number };
}

/** 订阅后端会话事件，维护会话状态 / 问题 / 答案 / 音量。主窗口与置顶窗口共用。 */
export function useSessionEvents() {
  const [sessionState, setSessionState] = useState<SessionState | null>(null);
  const [questions, setQuestions] = useState<DetectedQuestion[]>([]);
  const [answers, setAnswers] = useState<Record<string, AnswerDraft>>({});
  const [meters, setMeters] = useState<Meters>({});

  useEffect(() => {
    void getSessionState().then(setSessionState);
    const unsubs = [
      // 采集状态事件驱动：active=true → capturing；active=false → idle。
      // 置顶小窗没有开始按钮，必须依赖此事件同步主界面的会话状态。
      onEvent(EVENTS.captureState, (p) => {
        setSessionState(p.active ? "capturing" : "idle");
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

  const currentQuestion = questions.length > 0 ? questions[questions.length - 1] : null;
  const currentAnswer = currentQuestion ? answers[currentQuestion.id] : undefined;

  return {
    sessionState,
    setSessionState,
    questions,
    answers,
    meters,
    currentQuestion,
    currentAnswer,
  };
}
