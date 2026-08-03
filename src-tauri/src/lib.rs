pub mod answer;
pub mod asr;
pub mod audio;
mod commands;
pub mod pipeline;
pub mod profile;
pub mod question;
pub mod session;
mod state;
pub mod storage;
pub mod vad;

use std::sync::Arc;

use tauri::Manager;

pub use commands::AppState;

pub fn run() {
    let app_state = match AppState::new() {
        Ok(s) => s,
        Err(e) => {
            tracing::error!("初始化应用状态失败：{e}");
            std::process::exit(1);
        }
    };
    tauri::Builder::default()
        .manage(app_state)
        .manage(Arc::new(state::SessionManager::new()))
        .setup(|app| {
            // overlay 小窗的「关闭」改为隐藏：窗口销毁后无法再次 show() 唤起，
            // 保持窗口存活（后台继续监听事件），X 按钮等价于「关闭小窗」。
            if let Some(win) = app.get_webview_window("overlay") {
                let win_hidden = win.clone();
                win.on_window_event(move |event| {
                    if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                        api.prevent_close();
                        let _ = win_hidden.hide();
                    }
                });
            }
            // overlay 永不销毁，主窗口关闭时显式退出整个应用（否则进程常驻后台）。
            if let Some(main_win) = app.get_webview_window("main") {
                let app_handle = app.handle().clone();
                main_win.on_window_event(move |event| {
                    if let tauri::WindowEvent::Destroyed = event {
                        app_handle.exit(0);
                    }
                });
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::start_session,
            commands::stop_session,
            commands::session_state,
            commands::pin_current_answer,
            commands::generate_answer,
            commands::cancel_current_answer,
            commands::get_settings,
            commands::save_settings,
            commands::test_provider_connection,
            commands::clear_all_data,
            commands::list_models,
            commands::scan_and_import_models,
            commands::set_overlay_visible,
            commands::list_prompt_presets,
            commands::set_active_prompt_preset,
            commands::save_prompt_preset,
            commands::delete_prompt_preset,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
