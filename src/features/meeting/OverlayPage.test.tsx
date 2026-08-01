import { describe, expect, it } from "vitest";
import { render, screen } from "@testing-library/react";
import { OverlayPage } from "./OverlayPage";

describe("OverlayPage", () => {
  it("always shows AI and capture state", () => {
    render(<OverlayPage initialState="capturing" />);
    expect(screen.getByText("AI 辅助运行中")).toBeVisible();
    expect(screen.getByText("系统音频采集中")).toBeVisible();
  });
});
