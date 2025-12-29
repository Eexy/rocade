use tauri::{async_runtime::Mutex, State};

use crate::{
    igdb::{IgdbApiClient, IgdbGame},
    steam::{Game, SteamApiClient},
};

#[tauri::command]
pub async fn get_games(steam_client: State<'_, SteamApiClient>) -> Result<Vec<Game>, String> {
    let games_res = steam_client.get_games().await.map_err(|e| e.to_string())?;

    Ok(games_res)
}

#[tauri::command]
pub async fn get_game(
    igdb_client: State<'_, Mutex<IgdbApiClient>>,
    steam_game_id: i64,
) -> Result<IgdbGame, String> {
    let mut state = igdb_client.lock().await;
    let games_res = state
        .get_game(steam_game_id)
        .await
        .map_err(|e| e.to_string())?;

    Ok(games_res)
}
