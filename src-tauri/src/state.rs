use serde::Serialize;
use std::sync::Mutex;

use crate::session::SessionControl;

/// 会话状态机：只允许 `Idle -> Starting -> Capturing -> Stopping -> Idle`；
/// 失败状态（`Failed`）可以 `stop` 回到 `Idle`。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum SessionState {
    Idle,
    Starting,
    Capturing,
    Stopping,
    #[allow(dead_code)]
    Failed {
        message: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum SessionError {
    #[error("session is already running")]
    SessionAlreadyRunning,
    #[error("session is not running")]
    NotRunning,
    #[error("invalid transition from {from:?} to {to}")]
    InvalidTransition {
        from: SessionState,
        to: &'static str,
    },
}

/// 运行中会话的句柄：控制信号 + 后台任务。
#[derive(Clone)]
pub struct SessionHandle {
    pub ctl: SessionControl,
    pub task: tokio::task::AbortHandle,
}

pub struct SessionManager {
    state: Mutex<SessionState>,
    handle: Mutex<Option<SessionHandle>>,
}

impl SessionManager {
    pub fn new() -> Self {
        Self {
            state: Mutex::new(SessionState::Idle),
            handle: Mutex::new(None),
        }
    }

    /// 附加运行中会话的句柄（start_session 编排成功后）。
    pub fn attach(&self, handle: SessionHandle) {
        *self.handle.lock().expect("handle lock poisoned") = Some(handle);
    }

    /// 取走会话句柄（stop_session 时）。
    pub fn take_handle(&self) -> Option<SessionHandle> {
        self.handle.lock().expect("handle lock poisoned").take()
    }

    /// 当前会话句柄的克隆（pin/generate 命令用）。
    pub fn handle(&self) -> Option<SessionHandle> {
        self.handle.lock().expect("handle lock poisoned").clone()
    }

    pub fn state(&self) -> SessionState {
        self.state.lock().expect("state lock poisoned").clone()
    }

    /// `Idle -> Starting -> Capturing`；运行中重复启动返回 `SessionAlreadyRunning`。
    pub fn start(&self) -> Result<(), SessionError> {
        let mut state = self.state.lock().expect("state lock poisoned");
        match *state {
            SessionState::Idle => {
                *state = SessionState::Starting;
                *state = SessionState::Capturing;
                Ok(())
            }
            SessionState::Starting | SessionState::Capturing | SessionState::Stopping => {
                Err(SessionError::SessionAlreadyRunning)
            }
            SessionState::Failed { .. } => Err(SessionError::InvalidTransition {
                from: state.clone(),
                to: "starting",
            }),
        }
    }

    /// `Capturing -> Stopping -> Idle`；`Failed -> Idle`。
    pub fn stop(&self) -> Result<(), SessionError> {
        let mut state = self.state.lock().expect("state lock poisoned");
        match *state {
            SessionState::Capturing => {
                *state = SessionState::Stopping;
                *state = SessionState::Idle;
                Ok(())
            }
            SessionState::Failed { .. } => {
                *state = SessionState::Idle;
                Ok(())
            }
            SessionState::Idle | SessionState::Starting | SessionState::Stopping => {
                Err(SessionError::NotRunning)
            }
        }
    }

    /// `Starting`/`Capturing` 期间出错进入 `Failed`。
    #[allow(dead_code)]
    pub fn fail(&self, message: impl Into<String>) -> Result<(), SessionError> {
        let mut state = self.state.lock().expect("state lock poisoned");
        match *state {
            SessionState::Starting | SessionState::Capturing => {
                *state = SessionState::Failed {
                    message: message.into(),
                };
                Ok(())
            }
            _ => Err(SessionError::NotRunning),
        }
    }
}

impl Default for SessionManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn legal_transitions_reach_capturing_and_back_to_idle() {
        let manager = SessionManager::new();
        assert_eq!(manager.state(), SessionState::Idle);
        assert!(manager.start().is_ok());
        assert_eq!(manager.state(), SessionState::Capturing);
        assert!(manager.stop().is_ok());
        assert_eq!(manager.state(), SessionState::Idle);
    }

    #[test]
    fn duplicate_start_returns_session_already_running() {
        let manager = SessionManager::new();
        assert!(manager.start().is_ok());
        assert_eq!(manager.start(), Err(SessionError::SessionAlreadyRunning));
        assert_eq!(manager.state(), SessionState::Capturing);
    }

    #[test]
    fn duplicate_start_while_starting_or_stopping_is_rejected() {
        let manager = SessionManager::new();
        {
            let mut state = manager.state.lock().unwrap();
            *state = SessionState::Starting;
        }
        assert_eq!(manager.start(), Err(SessionError::SessionAlreadyRunning));
        {
            let mut state = manager.state.lock().unwrap();
            *state = SessionState::Stopping;
        }
        assert_eq!(manager.start(), Err(SessionError::SessionAlreadyRunning));
    }

    #[test]
    fn failed_state_can_stop_back_to_idle_and_restart() {
        let manager = SessionManager::new();
        manager.start().unwrap();
        manager.fail("boom").unwrap();
        assert!(matches!(manager.state(), SessionState::Failed { message } if message == "boom"));
        assert!(manager.stop().is_ok());
        assert_eq!(manager.state(), SessionState::Idle);
        assert!(manager.start().is_ok());
        assert_eq!(manager.state(), SessionState::Capturing);
    }

    #[test]
    fn start_from_failed_without_stop_is_invalid() {
        let manager = SessionManager::new();
        manager.start().unwrap();
        manager.fail("boom").unwrap();
        assert!(matches!(
            manager.start(),
            Err(SessionError::InvalidTransition { .. })
        ));
    }

    #[test]
    fn stop_when_not_running_is_rejected() {
        let manager = SessionManager::new();
        assert_eq!(manager.stop(), Err(SessionError::NotRunning));
    }

    #[test]
    fn fail_when_not_running_is_rejected() {
        let manager = SessionManager::new();
        assert_eq!(manager.fail("x"), Err(SessionError::NotRunning));
    }
}
