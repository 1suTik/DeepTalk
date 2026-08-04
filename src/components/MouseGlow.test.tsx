import { describe, expect, it } from "vitest";
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { MouseGlow } from "./MouseGlow";

describe("MouseGlow", () => {
  it("renders a decorative glow layer that never intercepts input", () => {
    const { container } = render(<MouseGlow />);
    const glow = screen.getByTestId("mouse-glow");
    expect(glow).toHaveAttribute("aria-hidden", "true");
    expect(glow).not.toHaveClass("mouse-glow--visible");
    expect(container.firstElementChild).toBe(glow);
  });

  it("tracks the pointer and only becomes visible after the first move", async () => {
    render(<MouseGlow />);
    const glow = screen.getByTestId("mouse-glow");

    fireEvent.pointerMove(window, { clientX: 120, clientY: 80 });

    await waitFor(() => expect(glow).toHaveClass("mouse-glow--visible"));
    expect(glow.style.getPropertyValue("--spot-x")).toBe("120px");
    expect(glow.style.getPropertyValue("--spot-y")).toBe("80px");
  });
});
