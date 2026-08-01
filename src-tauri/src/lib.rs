pub mod answer;
pub mod asr;
pub mod audio;
pub mod profile;
pub mod question;
mod commands;
mod state;
pub mod storage;
pub mod vad;

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
        .manage(state::SessionManager::new())
        .invoke_handler(tauri::generate_handler![
            commands::start_session,
            commands::stop_session,
            commands::session_state,
            commands::get_settings,
            commands::save_settings,
            commands::test_provider_connection,
            commands::clear_all_data,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
