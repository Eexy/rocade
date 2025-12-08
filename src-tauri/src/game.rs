use tauri::State;

use crate::steam::{Game, SteamApiClient};

#[tauri::command]
pub async fn greet(steam_client: State<'_, SteamApiClient>) -> Result<Vec<Game>, String> {
    let games_res = steam_client.get_games().await.map_err(|e| e.to_string())?;

    Ok(games_res)
}
