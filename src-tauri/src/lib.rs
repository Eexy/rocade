use core::panic;
use std::{collections::HashMap, env};

use tauri::{async_runtime::Mutex, Manager};

use crate::{
    db::DatabaseState,
    igdb::IgdbApiClient,
    steam::{SteamApiClient, SteamClient},
    twitch::TwitchApiClient,
};

mod commands;
mod db;
mod igdb;
mod steam;
mod twitch;

pub use commands::{get_game, get_games, refresh_games};

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

             dotenvy::dotenv().ok();

            let mut config = HashMap::new();

            for (key, val) in env::vars() {
                config.insert(key, val);
            }

            app.manage::<HashMap<String, String>>(config);

            let app_config = app.state::<HashMap<String, String>>();

            tauri::async_runtime::block_on(  async {
                let handle = app.app_handle();
               let db_state = db::DatabaseState::new(handle).await.expect("unable to init local db");
                app.manage::<DatabaseState>(db_state)
            });


            match (app_config.get("STEAM_API_KEY"), app_config.get("STEAM_PROFILE_ID")) {
                (Some(key), Some(profile_id)) => {
                    let steam_api_client = SteamApiClient::new(key.clone(), profile_id.clone());
                    app.manage::<SteamApiClient>(steam_api_client);
                    let steam_client = SteamClient::new();
                    app.manage::<SteamClient>(steam_client);
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
        .invoke_handler(tauri::generate_handler![get_games, refresh_games, get_game ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
