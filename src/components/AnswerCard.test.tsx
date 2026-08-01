import { describe, expect, it, vi } from "vitest";
import { fireEvent, render, screen } from "@testing-library/react";
import { AnswerCard } from "./AnswerCard";
import type { AnswerDraft, DetectedQuestion } from "../types/domain";

const question: DetectedQuestion = {
  id: "q-1",
  sourceSegmentIds: ["seg-1"],
  normalizedText: "请介绍一下你负责的项目",
  confidence: 0.85,
  detectedAtMs: 1000,
  level: "auto",
};

const answer = (overrides: Partial<AnswerDraft> = {}): AnswerDraft => ({
  questionId: "q-1",
  shortAnswer: "我负责音频采集模块。",
  keyPoints: ["低延迟采集", "独立声道"],
  followUps: ["如何优化延迟？"],
  status: "complete",
  ...overrides,
});

describe("AnswerCard", () => {
  it("renders question and short answer with collapsed details", () => {
    render(<AnswerCard question={question} answer={answer()} />);
    expect(screen.getByText("请介绍一下你负责的项目")).toBeVisible();
    expect(screen.getByLabelText("短答")).toHaveTextContent("我负责音频采集模块。");
    expect(screen.getByText("要点（2）")).toBeVisible();
    expect(screen.getByText("可能追问（1）")).toBeVisible();
  });

  it("shows streaming placeholder while generating", () => {
    render(<AnswerCard question={question} answer={answer({ shortAnswer: "", status: "streaming" })} />);
    expect(screen.getByLabelText("短答")).toHaveTextContent("正在生成…");
  });

  it("shows failure hint", () => {
    render(<AnswerCard question={question} answer={answer({ status: "failed" })} />);
    expect(screen.getByLabelText("短答")).toHaveTextContent("生成失败");
  });

  it("disables cancel when not streaming and calls actions", () => {
    const onCancel = vi.fn();
    const onRegenerate = vi.fn();
    const onCopy = vi.fn();
    const onPin = vi.fn();
    const onFontSize = vi.fn();
    render(
      <AnswerCard
        question={question}
        answer={answer()}
        onCancel={onCancel}
        onRegenerate={onRegenerate}
        onCopy={onCopy}
        onPin={onPin}
        onFontSize={onFontSize}
      />,
    );
    expect(screen.getByRole("button", { name: /取消/ })).toBeDisabled();
    fireEvent.click(screen.getByRole("button", { name: /重新生成/ }));
    expect(onRegenerate).toHaveBeenCalled();
    fireEvent.click(screen.getByRole("button", { name: /复制/ }));
    expect(onCopy).toHaveBeenCalled();
    fireEvent.click(screen.getByRole("button", { name: /固定记录/ }));
    expect(onPin).toHaveBeenCalled();
    fireEvent.click(screen.getByTitle("增大字体"));
    expect(onFontSize).toHaveBeenCalledWith(1);
  });

  it("shows copied state", () => {
    render(<AnswerCard question={question} answer={answer()} copied />);
    expect(screen.getByRole("button", { name: /已复制/ })).toBeVisible();
  });
});
