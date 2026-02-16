use std::{env, path::PathBuf};

use tauri::{async_runtime::Mutex, Manager};

use crate::{
    client::steam::SteamClient,
    config::{RocadeConfig, RocadeConfigError},
    db::{game::GameRepository, DatabaseState},
    igdb::IgdbApiClient,
    service::steam::SteamApiClient,
    twitch::TwitchApiClient,
};

mod client;
mod commands;
mod config;
mod db;
mod igdb;
mod service;
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

            let rocade_config = RocadeConfig {
                steam_api_key: env::var("STEAM_API_KEY").map_err(|_| {
                    RocadeConfigError::EnvError("STEAM_API_KEY not set".to_string())
                })?,
                steam_profile_id: env::var("STEAM_PROFILE_ID").map_err(|_| {
                    RocadeConfigError::EnvError("STEAM_PROFILE_ID not set".to_string())
                })?,
                twitch_client_id: env::var("TWITCH_CLIENT_ID").map_err(|_| {
                    RocadeConfigError::EnvError("TWITCH_CLIENT_ID not set".to_string())
                })?,
                twitch_client_secret: env::var("TWITCH_CLIENT_SECRET").map_err(|_| {
                    RocadeConfigError::EnvError("TWITCH_CLIENT_SECRET not set".to_string())
                })?,
            };

            tauri::async_runtime::block_on(async {
                let app_dir = app.app_handle().path().app_data_dir().map_err(|_| {
                    RocadeConfigError::ConfigError("unable to get app directory".to_string())
                })?;
                let db_state = db::DatabaseState::new(app_dir).await?;
                let game_repository = GameRepository::new(db_state.pool.clone());
                app.manage::<DatabaseState>(db_state);
                app.manage::<GameRepository>(game_repository);

                Ok::<(), RocadeConfigError>(())
            })?;

            let steam_api_client =
                SteamApiClient::new(rocade_config.steam_api_key, rocade_config.steam_profile_id);
            app.manage::<SteamApiClient>(steam_api_client);

            let home_path = app.path().home_dir().map_err(|_| {
                RocadeConfigError::ConfigError("unable to get home directory".to_string())
            })?;

            let steam_path = home_path
                .join(r".local")
                .join("share")
                .join("Steam")
                .join("steamapps");

            steam_path.try_exists().map_err(|_| {
                RocadeConfigError::ConfigError(
                    "steam client directory doesn't not exist or is not found".to_string(),
                )
            })?;

            let steam_client = SteamClient::new(steam_path);
            app.manage::<SteamClient>(steam_client);

            let twitch_api_client = TwitchApiClient::new(
                rocade_config.twitch_client_id,
                rocade_config.twitch_client_secret,
            );
            let igdb_api_client = Mutex::new(IgdbApiClient::new(twitch_api_client));
            app.manage::<Mutex<IgdbApiClient>>(igdb_api_client);

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
