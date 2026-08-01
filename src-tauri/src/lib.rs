pub mod audio;
mod commands;
mod state;

pub fn run() {
    tauri::Builder::default()
        .manage(state::SessionManager::new())
        .invoke_handler(tauri::generate_handler![
            commands::start_session,
            commands::stop_session,
            commands::session_state,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

