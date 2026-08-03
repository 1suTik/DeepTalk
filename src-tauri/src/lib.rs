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
