import { useState } from "react";
import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
import { OverlayPage } from "../features/meeting/OverlayPage";
import { MeetingPage } from "../features/meeting/MeetingPage";
import { SettingsPage } from "../features/settings/SettingsPage";
import { MouseGlow } from "../components/MouseGlow";
import { useSessionEvents } from "../features/meeting/useSessionEvents";
import type { PipelineState } from "../types/domain";

type MainView = "meeting" | "settings";

function isOverlayWindow(): boolean {
  try {
    return getCurrentWebviewWindow().label === "overlay";
  } catch {
    return false;
  }
}

/** 置顶小窗口：订阅会话事件，实时显示最新问题与流式短答。 */
function OverlayWindow() {
  const { sessionState, currentQuestion, currentAnswer } = useSessionEvents();
  const running = sessionState === "capturing" || sessionState === "starting";
  return (
    <OverlayPage
      initialState={(running ? "capturing" : "idle") as PipelineState}
      currentQuestion={currentQuestion}
      currentAnswer={currentAnswer}
    />
  );
}

/** 按窗口类型渲染：overlay 显示精简会议面板，main 显示主界面（会议页 + 设置页导航）。 */
export function App() {
  const [view, setView] = useState<MainView>("meeting");
  if (isOverlayWindow()) {
    return <OverlayWindow />;
  }
  return (
    <div className="app-shell" data-testid="app-shell">
      <MouseGlow />
      <nav className="app-shell__nav" aria-label="主导航">
        <button
          type="button"
          className={view === "meeting" ? "app-shell__tab app-shell__tab--active" : "app-shell__tab"}
          aria-current={view === "meeting" ? "page" : undefined}
          onClick={() => setView("meeting")}
        >
          主界面
        </button>
        <button
          type="button"
          className={view === "settings" ? "app-shell__tab app-shell__tab--active" : "app-shell__tab"}
          aria-current={view === "settings" ? "page" : undefined}
          onClick={() => setView("settings")}
        >
          设置
        </button>
      </nav>
      {view === "meeting" ? <MeetingPage /> : <SettingsPage />}
    </div>
  );
}
