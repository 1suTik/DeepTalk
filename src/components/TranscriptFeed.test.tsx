import { describe, expect, it } from "vitest";
import { render, screen } from "@testing-library/react";
import { TranscriptFeed } from "./TranscriptFeed";
import type { TranscriptSegment } from "../types/domain";

const seg = (overrides: Partial<TranscriptSegment> = {}): TranscriptSegment => ({
  id: "s1",
  speaker: "remote",
  text: "请介绍一下项目",
  startedAtMs: 1000,
  endedAtMs: 2000,
  isFinal: true,
  ...overrides,
});

describe("TranscriptFeed", () => {
  it("shows empty state", () => {
    render(<TranscriptFeed segments={[]} />);
    expect(screen.getByTestId("transcript-empty")).toBeVisible();
  });

  it("renders segments with speaker labels", () => {
    render(
      <TranscriptFeed
        segments={[
          seg({ id: "a", text: "临时文本", isFinal: false }),
          seg({ id: "b", text: "最终文本", speaker: "local" }),
        ]}
      />,
    );
    const items = screen.getAllByRole("listitem");
    expect(items).toHaveLength(2);
    expect(items[0]).toHaveAttribute("data-final", "false");
    expect(items[1]).toHaveAttribute("data-final", "true");
    expect(items[1]).toHaveAttribute("data-speaker", "local");
    expect(screen.getByText("本机")).toBeVisible();
    expect(screen.getByText("远端")).toBeVisible();
  });
});
