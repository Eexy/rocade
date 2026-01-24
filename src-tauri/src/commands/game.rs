use std::collections::HashMap;

use serde::Serialize;
use tauri::{async_runtime::Mutex, AppHandle, State};

use crate::{
    db::{
        artwork::ArtworkRepository, cover::CoverRepository, game::GameRepository,
        game_store::GameStoreRepository, genre::GenreRepository, DatabaseState,
    },
    igdb::{IgdbApiClient, IgdbGame},
    steam::{SteamApiClient, SteamClient},
};

#[derive(Serialize)]
pub struct Game {
    id: i64,
    name: String,
    summary: Option<String>,
    store_id: Option<String>,
    cover: Option<String>,
    is_installed: Option<bool>,
    artworks: Option<Vec<String>>,
    release_date: Option<i64>,
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
    GameStoreRepository::delete_games_store(&db_state.pool).await?;
    GenreRepository::delete_genres(&db_state.pool).await?;
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

    let games_stores = GameStoreRepository::get_games_store(&db_state.pool).await?;
    let mut games_stores_map = HashMap::new();

    for game_store in games_stores {
        games_stores_map.insert(game_store.game_id, game_store.store_id);
    }

    let games: Vec<Game> = games
        .into_iter()
        .map(|game| Game {
            id: game.id,
            name: game.name,
            summary: game.summary,
            is_installed: Some(false),
            cover: covers_map.get(&game.id).cloned(),
            artworks: artworks_map.get(&game.id).cloned(),
            store_id: games_stores_map.get(&game.id).cloned(),
            release_date: game.release_date,
        })
        .collect();

    Ok(games)
}

async fn insert_games(
    db_state: State<'_, DatabaseState>,
    games: Vec<IgdbGame>,
) -> Result<(), sqlx::Error> {
    for game in games {
        let id =
            GameRepository::insert_game(&db_state.pool, game.name, game.summary, game.release_date)
                .await?;

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

        GenreRepository::insert_game_genres(
            &db_state.pool,
            id,
            game.genres.iter().map(|genre| genre.name.clone()).collect(),
        )
        .await?;

        if let Some(store_id) = game.store_id {
            GameStoreRepository::insert_game_store(&db_state.pool, id, store_id).await?;
        }
    }

    Ok(())
}

#[tauri::command]
pub async fn get_game(
    steam_client: State<'_, SteamClient>,
    db_state: State<'_, DatabaseState>,
    game_id: i64,
) -> Result<Game, String> {
    let game = GameRepository::get_game_by_id(&db_state.pool, game_id)
        .await
        .map_err(|e| e.to_string())?;
    let cover = CoverRepository::get_game_cover(&db_state.pool, game_id)
        .await
        .map_err(|e| e.to_string())?;
    let artworks = ArtworkRepository::get_game_artworks(&db_state.pool, game_id)
        .await
        .map_err(|e| e.to_string())?;

    let game_store = GameStoreRepository::get_game_store(&db_state.pool, game_id)
        .await
        .map_err(|e| e.to_string())?;

    let is_installed = steam_client.is_steam_game_install(game_store.store_id.clone());

    Ok(Game {
        id: game.id,
        release_date: game.release_date,
        name: game.name,
        is_installed: Some(is_installed),
        summary: game.summary,
        artworks: Some(
            artworks
                .into_iter()
                .map(|artwork| artwork.artwork_id)
                .collect(),
        ),
        cover: Some(cover.cover_id),
        store_id: Some(game_store.store_id),
    })
}

#[tauri::command]
pub fn install_game(
    steam_client: State<'_, SteamClient>,
    app: AppHandle,
    steam_game_id: String,
) -> Result<bool, String> {
    steam_client.install_game(app, steam_game_id)
}

#[tauri::command]
pub fn uninstall_game(
    steam_client: State<'_, SteamClient>,
    app: AppHandle,
    steam_game_id: String,
) -> Result<bool, String> {
    steam_client.uninstall_game(app, steam_game_id)
}
