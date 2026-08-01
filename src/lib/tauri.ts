import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type { EventPayloads } from "./events";
import type { SessionState } from "../types/domain";

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
