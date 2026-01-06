use tauri::{async_runtime::Mutex, State};

use crate::{
    igdb::{IgdbApiClient, IgdbGame},
    steam::SteamApiClient,
};

#[tauri::command]
pub async fn get_games(
    steam_client: State<'_, SteamApiClient>,
    igdb_client: State<'_, Mutex<IgdbApiClient>>,
) -> Result<Vec<IgdbGame>, String> {
    let games_res = steam_client.get_games().await.map_err(|e| e.to_string())?;
    let mut locked_client = igdb_client.lock().await;

    let res = locked_client
        .get_games(games_res.iter().map(|game| game.appid).collect())
        .await?;

    Ok(res)
}

#[tauri::command]
pub async fn get_game(
    igdb_client: State<'_, Mutex<IgdbApiClient>>,
    steam_game_id: u64,
) -> Result<IgdbGame, String> {
    let mut state = igdb_client.lock().await;
    let games_res = state
        .get_game(steam_game_id)
        .await
        .map_err(|e| e.to_string())?;

    Ok(games_res)
}
