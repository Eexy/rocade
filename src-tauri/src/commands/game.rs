use std::collections::HashSet;

use crate::{
    client::steam::SteamClient,
    db::{
        game::{Game, GameRepository},
        DatabaseState,
    },
    igdb::{IgdbApiClient, IgdbError, IgdbGame},
    service::steam::{SteamApiClient, SteamError},
};
use serde::{Deserialize, Serialize};
use tauri::{async_runtime::Mutex, AppHandle, State};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum RocadeError {
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),
    #[error("steam error: {0}")]
    Steam(#[from] SteamError),
    #[error("igdb error: {0}")]
    Igdb(#[from] IgdbError),
}

impl Serialize for RocadeError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
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
    let mut games = game_repository.get_games().await?;

    if let Some(name) = query.and_then(|q| q.name) {
        games = games
            .into_iter()
            .filter(|game| {
                let game_name_lower = game.name.to_ascii_lowercase();

                if game_name_lower.contains(&name.to_ascii_lowercase()) {
                    return true;
                }

                similarity(&name.to_ascii_lowercase(), &game.name.to_ascii_lowercase()) > 0.4
            })
            .collect();
    }

    Ok(games)
}

pub fn trigrams(s: &str) -> HashSet<String> {
    let s_with_spaces = format!("  {} ", s);
    let chars: Vec<char> = s_with_spaces.chars().collect();
    let mut hashset: HashSet<String> = HashSet::new();

    for window in chars.windows(3) {
        hashset.insert(window.iter().collect());
    }

    hashset
}

pub fn similarity(a: &str, b: &str) -> f64 {
    let tri_a = trigrams(a);
    let tri_b = trigrams(b);

    tri_a.intersection(&tri_b).count() as f64 / tri_a.len() as f64
}

#[tauri::command]
pub async fn refresh_games(
    steam_client: State<'_, SteamApiClient>,
    igdb_client: State<'_, Mutex<IgdbApiClient>>,
    db_state: State<'_, DatabaseState>,
    game_repository: State<'_, GameRepository>,
) -> Result<(), RocadeError> {
    let games_res = steam_client.get_games().await?;

    let mut locked_client = igdb_client.lock().await;

    let igdb_games = locked_client
        .get_games(games_res.iter().map(|game| game.appid).collect())
        .await?;

    prepare_db(db_state.clone()).await?;

    insert_games(game_repository, igdb_games).await?;

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
    steam_client: State<'_, SteamClient>,
    game_id: i64,
) -> Result<Game, RocadeError> {
    let mut game = game_repository.get_game_by_id(game_id).await?;

    let mut is_installed = false;

    if let Some(store_id) = game.store_id.clone() {
        is_installed = steam_client.is_steam_game_installed(store_id);
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
    let store_id = game_repository.get_game_store_id(game_id).await?;

    SteamClient::install_game(app, store_id)?;

    Ok(true)
}

#[tauri::command]
pub async fn uninstall_game(
    game_repository: State<'_, GameRepository>,
    app: AppHandle,
    game_id: i64,
) -> Result<bool, RocadeError> {
    let store_id = game_repository.get_game_store_id(game_id).await?;

    SteamClient::uninstall_game(app, store_id)?;
    Ok(true)
}
