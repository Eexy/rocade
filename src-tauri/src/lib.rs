use core::panic;
use std::collections::HashMap;

use tauri::Manager;

use crate::steam::SteamState;

mod dotenv;
mod steam;

// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/
#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            #[cfg(debug_assertions)]
            {
                let window = app.get_webview_window("main").unwrap();
                window.open_devtools();
                window.close_devtools();
            }

            let config = dotenv::dotenv();

            match config {
                Ok(c) => {
                    app.manage::<HashMap<String, String>>(c);
                }
                Err(_) => {
                    panic!("unable to read config");
                }
            }

            let app_config = app.state::<HashMap<String, String>>();

            if let Some(steam_api_key) = app_config.get("STEAM_API_KEY") {
                let steam_state = SteamState::new(steam_api_key.clone());
                app.manage::<SteamState>(steam_state);
            } else {
                panic!("unable to get steam api key from config");
            }

            #[cfg(debug_assertions)]
            {
                let app_config = app.state::<HashMap<String, String>>();
                dbg!(app_config);
                let steam_state = app.state::<SteamState>();
                dbg!(steam_state);
            }

            Ok(())
        })
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![greet])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
