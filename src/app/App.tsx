import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
import { OverlayPage } from "../features/meeting/OverlayPage";
import { MeetingPage } from "../features/meeting/MeetingPage";
import type { PipelineState } from "../types/domain";

function isOverlayWindow(): boolean {
  try {
    return getCurrentWebviewWindow().label === "overlay";
  } catch {
    return false;
  }
}

/** 按窗口类型渲染：overlay 显示会议面板，main 显示主界面（会议页）。 */
export function App() {
  if (isOverlayWindow()) {
    return <OverlayPage initialState={"capturing" as PipelineState} />;
  }
  return <MeetingPage />;
}
