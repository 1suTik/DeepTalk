import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type { EventPayloads } from "./events";
import type { AppSettings, PromptPreset, SessionState } from "../types/domain";

/** 非 Tauri 环境（jsdom 单测、纯浏览器）下安全降级。 */
export function isTauri(): boolean {
  return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
}

export async function startSession(): Promise<SessionState> {
  return invoke("start_session");
}

export async function stopSession(): Promise<SessionState> {
  return invoke("stop_session");
}

export async function getSessionState(): Promise<SessionState> {
  return invoke("session_state");
}

export async function getSettings(): Promise<AppSettings> {
  return invoke("get_settings");
}

export async function saveSettings(
  settings: AppSettings,
  apiKey: string | null,
): Promise<void> {
  return invoke("save_settings", { settings, apiKey });
}

export async function testProviderConnection(
  settings: AppSettings,
  apiKey: string | null,
): Promise<string> {
  return invoke("test_provider_connection", { settings, apiKey });
}

export async function clearAllData(): Promise<void> {
  return invoke("clear_all_data");
}

export async function pinCurrentAnswer(): Promise<void> {
  return invoke("pin_current_answer");
}

/** 手动/重新生成最近检测到的问题（Maybe 级别问题由用户点击生成）。 */
export async function generateAnswer(): Promise<void> {
  return invoke("generate_answer");
}

/** 取消当前答案生成（保留已收到的内容，标记为 cancelled）。 */
export async function cancelCurrentAnswer(): Promise<void> {
  return invoke("cancel_current_answer");
}

export interface ModelStatus {
  id: string;
  name: string;
  sizeBytes: number;
  tier: string;
  imported: boolean;
  sha256Ok: boolean;
}

export interface ImportedModel {
  id: string;
  fileName: string;
  sha256: string;
  sizeBytes: number;
  importedAtMs: number;
}

/** 模型清单与本地导入状态（设置页模型卡片）。 */
export async function listModels(): Promise<ModelStatus[]> {
  return invoke("list_models");
}

/** 扫描模型目录并按清单校验导入（用户本地导入，不做自动下载）。 */
export async function scanAndImportModels(): Promise<ImportedModel[]> {
  return invoke("scan_and_import_models");
}

/** 显示/隐藏置顶小窗（主界面开关）。 */
export async function setOverlayVisible(visible: boolean): Promise<void> {
  return invoke("set_overlay_visible", { visible });
}

/** 提示词方案列表（内置 + 自定义，含激活标记）。 */
export async function listPromptPresets(): Promise<PromptPreset[]> {
  return invoke("list_prompt_presets");
}

/** 切换激活方案（下一轮答案立即生效）。 */
export async function setActivePromptPreset(id: string): Promise<void> {
  return invoke("set_active_prompt_preset", { id });
}

/** 新建或更新自定义方案；返回方案 id。 */
export async function savePromptPreset(
  id: string | null,
  name: string,
  systemPrompt: string,
  userPrompt: string,
): Promise<string> {
  return invoke("save_prompt_preset", { id, name, systemPrompt, userPrompt });
}

/** 删除自定义方案（内置不可删；删除激活方案自动回退默认）。 */
export async function deletePromptPreset(id: string): Promise<void> {
  return invoke("delete_prompt_preset", { id });
}

/** 订阅后端事件，返回取消订阅函数；非 Tauri 环境返回空操作。 */
export function onEvent<K extends keyof EventPayloads>(
  event: K,
  handler: (payload: EventPayloads[K]) => void,
): () => void {
  if (!isTauri()) return () => undefined;
  let unlisten: (() => void) | undefined;
  void listen<EventPayloads[K]>(event, (e) => handler(e.payload)).then((u) => {
    unlisten = u;
  });
  return () => unlisten?.();
}
