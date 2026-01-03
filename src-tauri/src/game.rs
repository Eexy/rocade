use std::time::Duration;

use futures::{stream, StreamExt};
use tauri::{async_runtime::Mutex, State};
use tokio::time::sleep;

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
    let mut igdb_games = Vec::new();

    for chunk in games_res.chunks(4) {
        let appids: Vec<u64> = chunk.iter().map(|game| game.appid).collect();

        let results: Vec<_> = stream::iter(appids)
            .map(|appid| {
                let client = igdb_client.clone();
                async move {
                    let mut lock_client = client.lock().await;
                    let res = lock_client.get_game(appid).await;
                    dbg!(&res);
                    res
                }
            })
            .buffer_unordered(4)
            .filter_map(|result| async move { result.ok() })
            .collect()
            .await;

        igdb_games.extend(results);

        sleep(Duration::from_secs(1)).await;
    }

    Ok(igdb_games)
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
