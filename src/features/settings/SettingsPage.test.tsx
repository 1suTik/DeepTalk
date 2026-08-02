import { describe, expect, it, vi } from "vitest";
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { SettingsPage } from "./SettingsPage";
import type { AppSettings } from "../../types/domain";

const base: AppSettings = {
  providerKind: "deepseek",
  baseUrl: "https://api.deepseek.com",
  model: "deepseek-v4-flash",
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
  listModels: vi.fn(async () => [
    {
      id: "ggml-large-v3-turbo-q5_0",
      name: "Whisper large-v3-turbo (Q5_0)",
      sizeBytes: 574041195,
      tier: "high",
      imported: true,
      sha256Ok: true,
    },
    {
      id: "silero-vad-v6",
      name: "Silero VAD v6 (16k ONNX)",
      sizeBytes: 2327524,
      tier: "vad",
      imported: false,
      sha256Ok: false,
    },
  ]),
  scanAndImportModels: vi.fn(async () => [
    {
      id: "silero-vad-v6",
      fileName: "silero_vad.onnx",
      sha256: "2623a2...",
      sizeBytes: 2327524,
      importedAtMs: 0,
    },
  ]),
}));

import {
  clearAllData,
  getSettings,
  saveSettings,
  scanAndImportModels,
  testProviderConnection,
} from "../../lib/tauri";

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
    expect(await screen.findByLabelText(/模型 ID/)).toHaveValue("deepseek-v4-flash");
  });

  it("shows model import status and scans for new models", async () => {
    render(<SettingsPage initial={base} />);
    expect(await screen.findByTestId("models-section")).toBeVisible();
    expect(screen.getByText("Whisper large-v3-turbo (Q5_0)")).toBeVisible();
    expect(screen.getByText("已导入 ✓")).toBeVisible();
    expect(screen.getByText("未导入")).toBeVisible();
    fireEvent.click(screen.getByRole("button", { name: "扫描并校验" }));
    await waitFor(() => expect(scanAndImportModels).toHaveBeenCalled());
    expect(await screen.findByTestId("scan-result")).toHaveTextContent("已导入 1 个模型");
  });
});
