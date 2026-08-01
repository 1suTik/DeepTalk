import { describe, expect, it, vi } from "vitest";
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { SettingsPage } from "./SettingsPage";
import type { AppSettings } from "../../types/domain";

const base: AppSettings = {
  providerKind: "deepseek",
  baseUrl: "https://api.deepseek.com/v1",
  model: "deepseek-chat",
  hasApiKey: false,
  retentionDays: 7,
  microphoneEnabled: false,
  asrModelId: "",
};

vi.mock("../../lib/tauri", () => ({
  getSettings: vi.fn(async () => base),
  saveSettings: vi.fn(async () => undefined),
  testProviderConnection: vi.fn(async () => "连接成功，首个输出：你好"),
  clearAllData: vi.fn(async () => undefined),
}));

import { clearAllData, getSettings, saveSettings, testProviderConnection } from "../../lib/tauri";

describe("SettingsPage", () => {
  it("switching provider updates default base url", () => {
    render(<SettingsPage initial={base} />);
    fireEvent.change(screen.getByLabelText(/答案服务/), {
      target: { value: "custom" },
    });
    expect(screen.getByLabelText(/Base URL/)).toHaveValue("http://127.0.0.1:11434/v1");
    fireEvent.change(screen.getByLabelText(/答案服务/), {
      target: { value: "openai" },
    });
    expect(screen.getByLabelText(/Base URL/)).toHaveValue("https://api.openai.com/v1");
  });

  it("never echoes a saved api key", () => {
    render(<SettingsPage initial={{ ...base, hasApiKey: true }} />);
    const input = screen.getByLabelText(/API Key/) as HTMLInputElement;
    expect(input.value).toBe("");
    expect(input.type).toBe("password");
    expect(input.placeholder).toContain("已保存");
  });

  it("saves settings with new key only when provided", async () => {
    render(<SettingsPage initial={base} />);
    fireEvent.click(screen.getByRole("button", { name: "保存设置" }));
    await waitFor(() => expect(saveSettings).toHaveBeenCalledWith(base, null));
    const input = screen.getByLabelText(/API Key/);
    fireEvent.change(input, { target: { value: "sk-new" } });
    fireEvent.click(screen.getByRole("button", { name: "保存设置" }));
    await waitFor(() => expect(saveSettings).toHaveBeenCalledWith(base, "sk-new"));
  });

  it("runs connection test and shows result", async () => {
    render(<SettingsPage initial={base} />);
    fireEvent.click(screen.getByRole("button", { name: "连接测试" }));
    await waitFor(() => expect(testProviderConnection).toHaveBeenCalled());
    expect(await screen.findByTestId("test-result")).toHaveTextContent("连接成功");
  });

  it("clears all data only after confirmation", async () => {
    const confirmSpy = vi.spyOn(window, "confirm").mockReturnValue(false);
    render(<SettingsPage initial={base} />);
    fireEvent.click(screen.getByRole("button", { name: "清除全部数据" }));
    await waitFor(() => expect(confirmSpy).toHaveBeenCalled());
    expect(clearAllData).not.toHaveBeenCalled();
    confirmSpy.mockReturnValue(true);
    fireEvent.click(screen.getByRole("button", { name: "清除全部数据" }));
    await waitFor(() => expect(clearAllData).toHaveBeenCalled());
  });

  it("loads settings from backend when no initial provided", async () => {
    render(<SettingsPage />);
    await waitFor(() => expect(getSettings).toHaveBeenCalled());
    expect(await screen.findByLabelText(/模型 ID/)).toHaveValue("deepseek-chat");
  });
});
