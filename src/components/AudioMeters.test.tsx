import { describe, expect, it } from "vitest";
import { render, screen } from "@testing-library/react";
import { AudioMeters } from "./AudioMeters";

describe("AudioMeters", () => {
  it("renders both channels with meter values", () => {
    render(
      <AudioMeters
        system={{ rms: 0.3, peak: 0.6 }}
        microphone={{ rms: 0.1, peak: 0.2 }}
      />,
    );
    const sys = screen.getByRole("meter", { name: "系统音频音量" });
    const mic = screen.getByRole("meter", { name: "麦克风音量" });
    expect(sys).toHaveAttribute("aria-valuenow", "30");
    expect(mic).toHaveAttribute("aria-valuenow", "10");
    expect(screen.getByText("系统音频")).toBeVisible();
    expect(screen.getByText("麦克风")).toBeVisible();
  });

  it("renders inactive channels at zero", () => {
    render(<AudioMeters />);
    expect(screen.getByRole("meter", { name: "系统音频音量" })).toHaveAttribute(
      "aria-valuenow",
      "0",
    );
    expect(screen.getByRole("meter", { name: "麦克风音量" })).toHaveAttribute(
      "aria-valuenow",
      "0",
    );
  });
});
