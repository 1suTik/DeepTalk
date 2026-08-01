import { describe, expect, it, vi } from "vitest";
import { act, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { MeetingPage } from "./MeetingPage";
import type { EventPayloads } from "../../lib/events";
import type { AnswerDraft, DetectedQuestion, TranscriptSegment } from "../../types/domain";

const emitter: { fire: <K extends keyof EventPayloads>(k: K, p: EventPayloads[K]) => void } = {
  fire: () => undefined,
};

vi.mock("../../lib/tauri", () => ({
  getSessionState: vi.fn(async () => "idle"),
  startSession: vi.fn(async () => "capturing"),
  stopSession: vi.fn(async () => "idle"),
  pinCurrentAnswer: vi.fn(async () => undefined),
  generateAnswer: vi.fn(async () => undefined),
  cancelCurrentAnswer: vi.fn(async () => undefined),
  onEvent: vi.fn(
    <K extends keyof EventPayloads>(_event: K, handler: (p: EventPayloads[K]) => void) => {
      const orig = emitter.fire;
      emitter.fire = <K2 extends keyof EventPayloads>(k: K2, p: EventPayloads[K2]) => {
        if ((k as string) === (_event as string)) {
          (handler as (p: EventPayloads[K]) => void)(p as EventPayloads[K]);
        }
        orig(k, p);
      };
      return () => undefined;
    },
  ),
}));

import { cancelCurrentAnswer, generateAnswer, pinCurrentAnswer, startSession, stopSession } from "../../lib/tauri";

const finalSegment: TranscriptSegment = {
  id: "seg-1",
  speaker: "remote",
  text: "请介绍一下你负责的项目",
  startedAtMs: 1000,
  endedAtMs: 2000,
  isFinal: true,
};

const question: DetectedQuestion = {
  id: "q-1",
  sourceSegmentIds: ["seg-1"],
  normalizedText: "请介绍一下你负责的项目",
  confidence: 0.85,
  detectedAtMs: 1000,
  level: "auto",
};

describe("MeetingPage", () => {
  it("starts and stops the session", async () => {
    render(<MeetingPage />);
    fireEvent.click(screen.getByRole("button", { name: "开始会话" }));
    await waitFor(() => expect(startSession).toHaveBeenCalled());
    fireEvent.click(screen.getByRole("button", { name: "停止会话" }));
    await waitFor(() => expect(stopSession).toHaveBeenCalled());
  });

  it("renders full event flow: transcript, question, streaming answer", async () => {
    render(<MeetingPage />);
    await act(async () => {
      emitter.fire("transcript-final", finalSegment);
      emitter.fire("question-detected", question);
      emitter.fire("answer-started", { questionId: "q-1", atMs: 2000 });
      emitter.fire("answer-delta", { questionId: "q-1", delta: "我负责音频模块" });
      emitter.fire("answer-delta", { questionId: "q-1", delta: "。\n" });
      const draft: AnswerDraft = {
        questionId: "q-1",
        shortAnswer: "我负责音频模块。\n",
        keyPoints: ["要点一"],
        followUps: ["追问一"],
        status: "complete",
      };
      emitter.fire("answer-completed", draft);
    });
    expect(screen.getAllByText("请介绍一下你负责的项目").length).toBeGreaterThan(0);
    expect(screen.getByLabelText("短答")).toHaveTextContent("我负责音频模块。");
    expect(screen.getByText("要点（1）")).toBeInTheDocument();
    expect(screen.getByText("追问一")).toBeInTheDocument();
    await waitFor(() => expect(screen.getAllByRole("listitem").length).toBeGreaterThan(0));
  });

  it("shows maybe question with manual generate button", async () => {
    render(<MeetingPage />);
    await act(async () => {
      emitter.fire("question-detected", { ...question, id: "q-2", level: "maybe" });
    });
    expect(screen.getByTestId("maybe-hint")).toHaveTextContent("可能的问题");
    fireEvent.click(screen.getByRole("button", { name: "生成答案" }));
    await waitFor(() => expect(generateAnswer).toHaveBeenCalled());
  });

  it("wires answer card actions", async () => {
    render(<MeetingPage />);
    await act(async () => {
      emitter.fire("question-detected", question);
      emitter.fire("answer-started", { questionId: "q-1", atMs: 2000 });
      emitter.fire("answer-delta", { questionId: "q-1", delta: "部分内容" });
    });
    fireEvent.click(screen.getByRole("button", { name: /固定记录/ }));
    await waitFor(() => expect(pinCurrentAnswer).toHaveBeenCalled());
    fireEvent.click(screen.getByRole("button", { name: /重新生成/ }));
    await waitFor(() => expect(generateAnswer).toHaveBeenCalled());
    fireEvent.click(screen.getByRole("button", { name: /取消/ }));
    await waitFor(() => expect(cancelCurrentAnswer).toHaveBeenCalled());
  });
});
