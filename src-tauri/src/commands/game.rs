use std::collections::HashMap;

use serde::Serialize;
use tauri::{async_runtime::Mutex, State};

use crate::{
    db::{artwork::ArtworkRepository, cover::CoverRepository, game::GameRepository, DatabaseState},
    igdb::{IgdbApiClient, IgdbGame},
    steam::SteamApiClient,
};

#[derive(Serialize)]
pub struct Game {
    id: i64,
    name: String,
    summary: Option<String>,
    cover: Option<String>,
    artworks: Option<Vec<String>>,
}

#[tauri::command]
pub async fn get_games(db_state: State<'_, DatabaseState>) -> Result<Vec<Game>, String> {
    let games = get_games_from_db(db_state.clone())
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
    CoverRepository::delete_covers(&db_state.pool).await?;
    ArtworkRepository::delete_artworks(&db_state.pool).await?;
    GameRepository::delete_games(&db_state.pool).await?;

    Ok(())
}

async fn get_games_from_db(db_state: State<'_, DatabaseState>) -> Result<Vec<Game>, sqlx::Error> {
    let games = GameRepository::get_games(&db_state.pool).await?;

    let covers = CoverRepository::get_covers(&db_state.pool).await?;
    let mut covers_map = HashMap::new();

    for cover in covers {
        covers_map.insert(cover.game_id, cover.cover_id);
    }

    let artworks = ArtworkRepository::get_artworks(&db_state.pool).await?;
    let mut artworks_map = HashMap::new();

    for artwork in artworks {
        artworks_map
            .entry(artwork.game_id)
            .or_insert_with(Vec::new)
            .push(artwork.artwork_id);
    }

    let games: Vec<Game> = games
        .into_iter()
        .map(|game| Game {
            id: game.id,
            name: game.name,
            summary: game.summary,
            cover: covers_map.get(&game.id).cloned(),
            artworks: artworks_map.get(&game.id).cloned(),
        })
        .collect();

    Ok(games)
}

async fn insert_games(
    db_state: State<'_, DatabaseState>,
    games: Vec<IgdbGame>,
) -> Result<(), sqlx::Error> {
    for game in games {
        let id = GameRepository::insert_game(&db_state.pool, game.name, game.summary).await?;

        CoverRepository::insert_cover(&db_state.pool, id, game.cover.image_id).await?;

        if let Some(artworks) = game.artworks {
            if !artworks.is_empty() {
                let artworks_ids: Vec<_> = artworks
                    .into_iter()
                    .map(|artwork| artwork.image_id)
                    .collect();

                ArtworkRepository::bulk_insert_artworks(&db_state.pool, id, artworks_ids).await?;
            }
        }
    }

    Ok(())
}
