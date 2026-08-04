import { describe, expect, it, vi } from "vitest";
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { App } from "./App";
import type { AppSettings } from "../types/domain";

vi.mock("../lib/tauri", () => ({
  getSessionState: vi.fn(async () => "idle"),
  startSession: vi.fn(async () => "capturing"),
  stopSession: vi.fn(async () => "idle"),
  pinCurrentAnswer: vi.fn(async () => undefined),
  generateAnswer: vi.fn(async () => undefined),
  cancelCurrentAnswer: vi.fn(async () => undefined),
  onEvent: vi.fn(() => () => undefined),
  getSettings: vi.fn(async () => ({
    providerKind: "deepseek",
    baseUrl: "https://api.deepseek.com/v1",
    model: "deepseek-v4-flash",
    hasApiKey: false,
    retentionDays: 7,
    microphoneEnabled: false,
    asrModelId: "",
  }) as AppSettings),
  saveSettings: vi.fn(async () => undefined),
  testProviderConnection: vi.fn(async () => "ok"),
  clearAllData: vi.fn(async () => undefined),
  listModels: vi.fn(async () => []),
  scanAndImportModels: vi.fn(async () => []),
  setOverlayVisible: vi.fn(async () => undefined),
  listPromptPresets: vi.fn(async () => []),
  setActivePromptPreset: vi.fn(async () => undefined),
  savePromptPreset: vi.fn(async () => "custom-1"),
  deletePromptPreset: vi.fn(async () => undefined),
}));

describe("App (main window)", () => {
  it("shows meeting page and navigates to settings", async () => {
    render(<App />);
    expect(screen.getByTestId("app-shell")).toBeVisible();
    expect(screen.getByTestId("mouse-glow")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "主界面" })).toHaveAttribute("aria-current", "page");
    fireEvent.click(screen.getByRole("button", { name: "设置" }));
    await waitFor(() => expect(screen.getByTestId("settings-page")).toBeVisible());
    expect(screen.getByRole("button", { name: "设置" })).toHaveAttribute("aria-current", "page");
  });
});
