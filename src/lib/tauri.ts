import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type { EventPayloads } from "./events";
import type { AppSettings, SessionState } from "../types/domain";

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
