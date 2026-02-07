use crate::{
    db::{
        game::{Game, GameRepository},
        DatabaseState,
    },
    igdb::{IgdbApiClient, IgdbGame},
    steam::{SteamApiClient, SteamClient},
};
use tauri::{async_runtime::Mutex, AppHandle, State};

#[tauri::command]
pub async fn get_games(db_state: State<'_, DatabaseState>) -> Result<Vec<Game>, String> {
    let games = GameRepository::get_games(&db_state.pool)
        .await
        .map_err(|e| e.to_string())?;

    Ok(games)
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
        let id = GameRepository::insert_complete_game(&db_state.pool, game).await?;
        dbg!(&id);
    }

    Ok(())
}

#[tauri::command]
pub async fn get_game(
    steam_client: State<'_, SteamClient>,
    db_state: State<'_, DatabaseState>,
    game_id: i64,
) -> Result<Game, String> {
    let mut game = GameRepository::get_game_by_id(&db_state.pool, game_id)
        .await
        .map_err(|e| e.to_string())?;

    let mut is_installed = false;

    if let Some(store_id) = game.store_id.clone() {
        is_installed = steam_client.is_steam_game_install(store_id);
    }

    game.is_installed = Some(is_installed);

    Ok(game)
}

#[tauri::command]
pub async fn install_game(
    steam_client: State<'_, SteamClient>,
    db_state: State<'_, DatabaseState>,
    app: AppHandle,
    game_id: i64,
) -> Result<bool, String> {
    let store_id: Option<String> =
        sqlx::query_scalar("select store_id from games_store where game_id = $1")
            .bind(game_id)
            .fetch_one(&db_state.pool)
            .await
            .map_err(|e| e.to_string())?;

    if let Some(id) = store_id {
        steam_client
            .install_game(app, id)
            .map_err(|e| e.to_string())?;
        return Ok(true);
    }

    Err("unable to install game".to_string())
}

#[tauri::command]
pub async fn uninstall_game(
    steam_client: State<'_, SteamClient>,
    db_state: State<'_, DatabaseState>,
    app: AppHandle,
    game_id: i64,
) -> Result<bool, String> {
    let store_id: Option<String> =
        sqlx::query_scalar("select store_id from games_store where game_id = $1")
            .bind(game_id)
            .fetch_one(&db_state.pool)
            .await
            .map_err(|e| e.to_string())?;

    if let Some(id) = store_id {
        steam_client
            .uninstall_game(app, id)
            .map_err(|e| e.to_string())?;
        return Ok(true);
    }

    Err("unable to uninstall game".to_string())
}
