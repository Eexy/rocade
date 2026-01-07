use futures::{stream, StreamExt};
use serde::Serialize;
use sqlx::{Database, QueryBuilder, Sqlite};
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

    prepare_db(db_state.clone())
        .await
        .map_err(|e| e.to_string())?;

    insert_games(db_state.clone(), igdb_games)
        .await
        .map_err(|e| e.to_string())?;

    let games = sqlx::query_as!(Game, r#"select * from games"#)
        .fetch_all(&mut *pool)
        .await
        .map_err(|e| e.to_string())?;

    Ok(games)
}

async fn prepare_db(db_state: State<'_, DatabaseState>) -> Result<(), sqlx::Error> {
    let mut pool = db_state.pool.acquire().await?;

    sqlx::query("delete from games").execute(&mut *pool).await?;

    Ok(())
}

async fn insert_games(
    db_state: State<'_, DatabaseState>,
    games: Vec<IgdbGame>,
) -> Result<(), sqlx::Error> {
    let res: Vec<_> = stream::iter(&games)
        .map(|game| {
            let state = db_state.clone();
            async move {
                let mut pool = state.pool.acquire().await?;
                let id = sqlx::query!(
                    r#"insert into games (name, summary) values ( ?1, ?2)"#,
                    game.name,
                    game.summary
                )
                .execute(&mut *pool)
                .await?
                .last_insert_rowid();
                Ok::<i64, sqlx::Error>(id)
            }
        })
        .filter_map(|item| async move { item.await.ok() })
        .collect()
        .await;
    dbg!(res);
    Ok(())
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
