use serde::Serialize;
use sqlx::{QueryBuilder, Sqlite};
use tauri::{async_runtime::Mutex, State};

use crate::{
    db::DatabaseState,
    igdb::{IgdbApiClient, IgdbGame},
    steam::SteamApiClient,
};

#[derive(sqlx::FromRow, Serialize)]
pub struct Game {
    id: i64,
    name: String,
    summary: Option<String>,
}

#[tauri::command]
pub async fn get_games(
    steam_client: State<'_, SteamApiClient>,
    igdb_client: State<'_, Mutex<IgdbApiClient>>,
    db_state: State<'_, DatabaseState>,
) -> Result<Vec<Game>, String> {
    let games_res = steam_client.get_games().await.map_err(|e| e.to_string())?;
    let mut locked_client = igdb_client.lock().await;

    let igdb_games = locked_client
        .get_games(games_res.iter().map(|game| game.appid).collect())
        .await?;

    let mut pool = match db_state.pool.acquire().await {
        Ok(p) => p,
        Err(_) => return Err("unable to acquire db connection".to_string()),
    };

    sqlx::query("delete from games")
        .execute(&mut *pool)
        .await
        .map_err(|_| "unable to delete game from games table".to_string())?;

    let mut query_builder: QueryBuilder<Sqlite> =
        QueryBuilder::new("insert into games (name, summary) ");

    query_builder.push_values(&igdb_games, |mut query_builder, record| {
        query_builder
            .push_bind(record.name.clone())
            .push_bind(record.summary.clone());
    });

    let query = query_builder.build();
    query.execute(&mut *pool).await.map_err(|e| e.to_string())?;

    let games = sqlx::query_as!(Game, r#"select * from games"#)
        .fetch_all(&mut *pool)
        .await
        .map_err(|e| e.to_string())?;

    Ok(games)
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
