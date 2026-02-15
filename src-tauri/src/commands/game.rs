use std::collections::HashSet;

use crate::{
    db::{
        game::{Game, GameRepository},
        DatabaseState,
    },
    igdb::{IgdbApiClient, IgdbGame},
    steam::{SteamApiClient, SteamClient},
};
use serde::{Deserialize, Serialize};
use tauri::{async_runtime::Mutex, window, AppHandle, State};
use thiserror::Error;

#[derive(Debug, Serialize, Error)]
pub enum RocadeError {
    #[error("database error: {0}")]
    Database(String),
    #[error("steam error: {0}")]
    Steam(String),
    #[error("igddb error: {0}")]
    Igdb(String),
}

#[derive(Deserialize, Debug)]
pub struct GameQuery {
    name: Option<String>,
}

#[tauri::command]
pub async fn get_games(
    game_repository: State<'_, GameRepository>,
    query: Option<GameQuery>,
) -> Result<Vec<Game>, RocadeError> {
    let mut games = game_repository
        .get_games()
        .await
        .map_err(|e| RocadeError::Database(e.to_string()))?;

    if let Some(name) = query.and_then(|q| q.name) {
        games = games
            .into_iter()
            .filter(|game| {
                let game_name_lower = game.name.to_ascii_lowercase();

                if game_name_lower.contains(&name.to_ascii_lowercase()) {
                    return true;
                }

                similarity(
                    name.clone().to_ascii_lowercase(),
                    game.name.clone().to_ascii_lowercase(),
                ) > 0.4
            })
            .collect();
    }

    Ok(games)
}

pub fn trigrams(s: String) -> HashSet<String> {
    let s_with_spaces = format!("  {} ", s);
    let chars: Vec<char> = s_with_spaces.chars().collect();
    let mut hashset: HashSet<String> = HashSet::new();

    for window in chars.windows(3) {
        hashset.insert(window.iter().collect());
    }

    hashset
}

pub fn similarity(a: String, b: String) -> f64 {
    let tri_a = trigrams(a);
    let tri_b = trigrams(b);

    return tri_a.intersection(&tri_b).count() as f64 / tri_a.len() as f64;
}

#[tauri::command]
pub async fn refresh_games(
    steam_client: State<'_, SteamApiClient>,
    igdb_client: State<'_, Mutex<IgdbApiClient>>,
    db_state: State<'_, DatabaseState>,
    game_repository: State<'_, GameRepository>,
) -> Result<(), RocadeError> {
    let games_res = steam_client
        .get_games()
        .await
        .map_err(|e| RocadeError::Steam(e.to_string()))?;
    let mut locked_client = igdb_client.lock().await;

    let igdb_games = locked_client
        .get_games(games_res.iter().map(|game| game.appid).collect())
        .await
        .map_err(|e| RocadeError::Igdb(e.to_string()))?;

    prepare_db(db_state.clone())
        .await
        .map_err(|e| RocadeError::Database(e.to_string()))?;

    insert_games(game_repository, igdb_games)
        .await
        .map_err(|e| RocadeError::Database(e.to_string()))?;

    Ok(())
}

async fn prepare_db(db_state: State<'_, DatabaseState>) -> Result<(), sqlx::Error> {
    db_state.clean().await
}

async fn insert_games(
    game_repository: State<'_, GameRepository>,
    games: Vec<IgdbGame>,
) -> Result<(), sqlx::Error> {
    for game in games {
        let _id = game_repository.insert_complete_game(game).await?;
    }

    Ok(())
}

#[tauri::command]
pub async fn get_game(
    game_repository: State<'_, GameRepository>,
    game_id: i64,
) -> Result<Game, RocadeError> {
    let mut game = game_repository
        .get_game_by_id(game_id)
        .await
        .map_err(|e| RocadeError::Database(e.to_string()))?;

    let mut is_installed = false;

    if let Some(store_id) = game.store_id.clone() {
        is_installed = SteamClient::is_steam_game_installed(store_id);
    }

    game.is_installed = Some(is_installed);

    Ok(game)
}

#[tauri::command]
pub async fn install_game(
    game_repository: State<'_, GameRepository>,
    app: AppHandle,
    game_id: i64,
) -> Result<bool, RocadeError> {
    let store_id = game_repository
        .get_game_store_id(game_id)
        .await
        .map_err(|e| RocadeError::Database(e.to_string()))?;

    SteamClient::install_game(app, store_id).map_err(|e| RocadeError::Steam(e.to_string()))?;

    Ok(true)
}

#[tauri::command]
pub async fn uninstall_game(
    game_repository: State<'_, GameRepository>,
    app: AppHandle,
    game_id: i64,
) -> Result<bool, RocadeError> {
    let store_id = game_repository
        .get_game_store_id(game_id)
        .await
        .map_err(|e| RocadeError::Database(e.to_string()))?;

    SteamClient::uninstall_game(app, store_id).map_err(|e| RocadeError::Steam(e.to_string()))?;
    Ok(true)
}
