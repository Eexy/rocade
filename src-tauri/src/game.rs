use futures::{stream, StreamExt};
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

#[derive(sqlx::FromRow)]
pub struct Artwork {
    id: i64,
    game_id: i64,
    artwork_id: String,
}

#[derive(sqlx::FromRow)]
pub struct Cover {
    id: i64,
    game_id: i64,
    cover_id: String,
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

    sqlx::query("delete from artworks")
        .execute(&mut *pool)
        .await?;
    sqlx::query("delete from covers")
        .execute(&mut *pool)
        .await?;
    sqlx::query("delete from games").execute(&mut *pool).await?;

    Ok(())
}

async fn insert_games(
    db_state: State<'_, DatabaseState>,
    games: Vec<IgdbGame>,
) -> Result<(), sqlx::Error> {
    let _res: Vec<_> = stream::iter(&games)
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

                sqlx::query!(
                    r#"insert into covers (game_id, cover_id) values ( ?1, ?2)"#,
                    id,
                    game.cover.image_id
                )
                .execute(&mut *pool)
                .await?;

                if let Some(artworks) = &game.artworks {
                    if !artworks.is_empty() {
                        let mut artwork_query_builder: QueryBuilder<Sqlite> =
                            QueryBuilder::new("insert into artworks (game_id, artwork_id) ");
                        artwork_query_builder.push_values(
                            artworks,
                            |mut query_builder, artwork| {
                                query_builder
                                    .push_bind(id)
                                    .push_bind(artwork.image_id.clone());
                            },
                        );
                        let query = artwork_query_builder.build();
                        query.execute(&mut *pool).await?;
                    }
                }

                Ok::<i64, sqlx::Error>(id)
            }
        })
        .collect()
        .await;

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
