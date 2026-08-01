use tauri::State;

use crate::state::{SessionManager, SessionState};

#[tauri::command]
pub fn start_session(manager: State<'_, SessionManager>) -> Result<SessionState, String> {
    manager.start().map_err(|e| format!("{e}"))?;
    Ok(manager.state())
}

#[tauri::command]
pub fn stop_session(manager: State<'_, SessionManager>) -> Result<SessionState, String> {
    manager.stop().map_err(|e| format!("{e}"))?;
    Ok(manager.state())
}

#[tauri::command]
pub fn session_state(manager: State<'_, SessionManager>) -> SessionState {
    manager.state()
}
