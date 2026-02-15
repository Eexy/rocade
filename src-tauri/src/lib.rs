use std::env;

use tauri::{async_runtime::Mutex, Manager};

use crate::{
    config::RocadeConfig,
    db::{game::GameRepository, DatabaseState},
    igdb::IgdbApiClient,
    steam::SteamApiClient,
    twitch::TwitchApiClient,
};

mod commands;
mod config;
mod db;
mod igdb;
mod steam;
mod twitch;

pub use commands::{get_game, get_games, install_game, refresh_games, uninstall_game};

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

            let mut rocade_config = RocadeConfig::new();

            for (key, val) in env::vars() {
                if key == "STEAM_API_KEY" {
                    rocade_config.steam_api_key = val;
                } else if key == "STEAM_PROFILE_ID" {
                    rocade_config.steam_profile_id = val;
                } else if key == "TWITCH_CLIENT_SECRET" {
                    rocade_config.twitch_client_secret = val;
                } else if key == "TWITCH_CLIENT_ID" {
                    rocade_config.twitch_client_id = val
                }
            }

            tauri::async_runtime::block_on(async {
                let handle = app.app_handle();
                let db_state = db::DatabaseState::new(handle)
                    .await
                    .expect("unable to init local db");
                let game_repository = GameRepository::new(db_state.pool.clone());
                app.manage::<DatabaseState>(db_state);
                app.manage::<GameRepository>(game_repository)
            });

            let steam_api_client = SteamApiClient::new(
                rocade_config.steam_api_key.clone(),
                rocade_config.steam_profile_id.clone(),
            );
            app.manage::<SteamApiClient>(steam_api_client);

            let twitch_api_client = TwitchApiClient::new(
                rocade_config.twitch_client_id.clone(),
                rocade_config.twitch_client_secret.clone(),
            );
            let igdb_api_client = Mutex::new(IgdbApiClient::new(twitch_api_client));
            app.manage::<Mutex<IgdbApiClient>>(igdb_api_client);

            app.manage::<RocadeConfig>(rocade_config);

            Ok(())
        })
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            get_games,
            refresh_games,
            get_game,
            install_game,
            uninstall_game
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
