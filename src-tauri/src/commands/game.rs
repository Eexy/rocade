use std::collections::HashSet;

use crate::{
    db::{
        game::{Game, GameRepository},
        DatabaseState,
    },
    igdb::{IgdbApiClient, IgdbGame},
    steam::{SteamApiClient, SteamClient},
};
use serde::Deserialize;
use tauri::{async_runtime::Mutex, AppHandle, State};

#[derive(Deserialize, Debug)]
pub struct GameQuery {
    name: Option<String>,
}

#[tauri::command]
pub async fn get_games(
    db_state: State<'_, DatabaseState>,
    query: Option<GameQuery>,
) -> Result<Vec<Game>, String> {
    let mut games = GameRepository::get_games(&db_state.pool)
        .await
        .map_err(|e| e.to_string())?;

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
    let mut hashset: HashSet<String> = HashSet::new();

    for i in 0..s_with_spaces.len() - 2 {
        hashset.insert(s_with_spaces[i..i + 3].to_string());
    }

    return hashset;
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
) -> Result<(), String> {
    let games_res = steam_client.get_games().await.map_err(|e| e.to_string())?;
    let mut locked_client = igdb_client.lock().await;

    let igdb_games = locked_client
        .get_games(games_res.iter().map(|game| game.appid).collect())
        .await?;

    prepare_db(db_state.clone())
        .await
        .map_err(|e| e.to_string())?;

    insert_games(db_state.clone(), igdb_games)
        .await
        .map_err(|e| e.to_string())?;

    Ok(())
}

async fn prepare_db(db_state: State<'_, DatabaseState>) -> Result<(), sqlx::Error> {
    db_state.clean().await
}

async fn insert_games(
    db_state: State<'_, DatabaseState>,
    games: Vec<IgdbGame>,
) -> Result<(), sqlx::Error> {
    for game in games {
        let _id = GameRepository::insert_complete_game(&db_state.pool, game).await?;
    }

    Ok(())
}

#[tauri::command]
pub async fn get_game(db_state: State<'_, DatabaseState>, game_id: i64) -> Result<Game, String> {
    let mut game = GameRepository::get_game_by_id(&db_state.pool, game_id)
        .await
        .map_err(|e| e.to_string())?;

    let mut is_installed = false;

    if let Some(store_id) = game.store_id.clone() {
        is_installed = SteamClient::is_steam_game_install(store_id);
    }

    game.is_installed = Some(is_installed);

    Ok(game)
}

#[tauri::command]
pub async fn install_game(
    db_state: State<'_, DatabaseState>,
    app: AppHandle,
    game_id: i64,
) -> Result<bool, String> {
    let store_id = GameRepository::get_game_store_id(&db_state.pool, game_id)
        .await
        .map_err(|e| e.to_string())?;

    SteamClient::install_game(app, store_id).map_err(|e| e.to_string())?;

    Ok(true)
}

#[tauri::command]
pub async fn uninstall_game(
    db_state: State<'_, DatabaseState>,
    app: AppHandle,
    game_id: i64,
) -> Result<bool, String> {
    let store_id = GameRepository::get_game_store_id(&db_state.pool, game_id)
        .await
        .map_err(|e| e.to_string())?;

    SteamClient::uninstall_game(app, store_id).map_err(|e| e.to_string())?;
    Ok(true)
}
