import { describe, expect, it } from "vitest";
import { render, screen } from "@testing-library/react";
import { OverlayPage } from "./OverlayPage";
import type { AnswerDraft, DetectedQuestion } from "../../types/domain";

const question: DetectedQuestion = {
  id: "q-1",
  sourceSegmentIds: ["seg-1"],
  normalizedText: "请介绍一下你负责的项目",
  confidence: 0.85,
  detectedAtMs: 1000,
  level: "auto",
};

const answer: AnswerDraft = {
  questionId: "q-1",
  shortAnswer: "我负责音频采集模块。",
  keyPoints: [],
  followUps: [],
  status: "streaming",
  createdAtMs: 2000,
};

describe("OverlayPage", () => {
  it("always shows AI and capture state", () => {
    render(<OverlayPage initialState="capturing" />);
    expect(screen.getByText("AI 辅助运行中")).toBeVisible();
    expect(screen.getByText("系统音频采集中")).toBeVisible();
  });

  it("shows empty state when idle without question", () => {
    render(<OverlayPage initialState="idle" />);
    expect(screen.getByText("等待识别问题…")).toBeVisible();
  });

  it("renders latest question and streaming short answer", () => {
    render(
      <OverlayPage
        initialState="capturing"
        currentQuestion={question}
        currentAnswer={answer}
      />,
    );
    expect(screen.getByText("请介绍一下你负责的项目")).toBeVisible();
    expect(screen.getByLabelText("短答")).toHaveTextContent("我负责音频采集模块。");
  });

  it("shows streaming placeholder without answer content", () => {
    render(
      <OverlayPage
        initialState="capturing"
        currentQuestion={question}
        currentAnswer={{ ...answer, shortAnswer: "" }}
      />,
    );
    expect(screen.getByLabelText("短答")).toHaveTextContent("正在生成…");
  });
});
