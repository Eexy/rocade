use core::panic;
use std::collections::HashMap;

use tauri::{async_runtime::Mutex, Manager};

use crate::{
    igdb::IgdbApiClient,
    steam::{SteamApiClient, SteamState},
    twitch::TwitchApiClient,
};

mod dotenv;
mod game;
mod igdb;
mod steam;
mod twitch;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_http::init())
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

            match (app_config.get("STEAM_API_KEY"), app_config.get("STEAM_PROFILE_ID")) {
                (Some(key), Some(profile_id)) => {
                    let steam_state = SteamState::new(key.clone(), profile_id.clone());
                    let steam_api_client = SteamApiClient::new(key.clone(), profile_id.clone());
                    app.manage::<SteamState>(steam_state);
                    app.manage::<SteamApiClient>(steam_api_client);
                },
                _ => {
                    panic!("Unable to load steam config. Missing STEAM_KEY or STEAM_PROFILE_ID in dotenv file")
                }
            }


            match (app_config.get("TWITCH_CLIENT_ID"), app_config.get("TWITCH_CLIENT_SECRET")) {
                (Some(client_id), Some(client_secret)) => {
                        let twitch_api_client = TwitchApiClient::new(client_id.clone(), client_secret.clone());
                        let  igdb_api_client = Mutex::new(IgdbApiClient::new(twitch_api_client)) ;
                        app.manage::<Mutex<IgdbApiClient>>(igdb_api_client);
                },
                _ => {
                    panic!("Unable to load twitch config. Missing TWITCH_CLIENT_ID or TWITCH_CLIENT_SECRET in dotenv file")
                }
            }

            Ok(())
        })
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![game::get_games, game::get_game])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
