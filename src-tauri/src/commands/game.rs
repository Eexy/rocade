use std::collections::HashMap;

use crate::{
    db::{
        artwork::ArtworkRepository, cover::CoverRepository, game::GameRepository,
        game_store::GameStoreRepository, genre::GenreRepository, studio::StudioRepository,
        DatabaseState,
    },
    igdb::{IgdbApiClient, IgdbGame},
    steam::{SteamApiClient, SteamClient},
};
use serde::Serialize;
use tauri::{async_runtime::Mutex, AppHandle, State};

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
    genres: Option<Vec<String>>,
    developers: Option<Vec<Option<String>>>,
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
    StudioRepository::delete_studios(&db_state.pool).await?;
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
            developers: None,
            genres: None,
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

        StudioRepository::insert_game_studios(&db_state.pool, id, game.developers).await?;

        if let Some(store_id) = game.store_id {
            GameStoreRepository::insert_game_store(&db_state.pool, id, store_id).await?;
        }
    }

    Ok(())
}

#[derive(Debug)]
pub struct GameInfo {
    id: i64,
    name: String,
    summary: Option<String>,
    artworks: Option<String>,
    covers: Option<String>,
    store_id: Option<String>,
    release_date: Option<i64>,
    genres: Option<String>,
    studios: Option<String>,
}

#[tauri::command]
pub async fn get_game(
    steam_client: State<'_, SteamClient>,
    db_state: State<'_, DatabaseState>,
    game_id: i64,
) -> Result<Game, String> {
    let game = sqlx::query_as!(GameInfo,
        "
select games.id as id, games.name as name, games_store.store_id as store_id, summary, release_date, group_concat(distinct genres.name) as genres, group_concat(distinct studios.name) as studios, group_concat(distinct artworks.artwork_id) as artworks, group_concat(distinct covers.cover_id) as covers
from games
inner join games_studios on games.id = games_studios.game_id
inner join studios on games_studios.studio_id = studios.id
inner join games_genres on games.id = games_genres.game_id
inner join genres on games_genres.genre_id = genres.id
inner join artworks on artworks.game_id = games.id
inner join covers on covers.game_id = games.id
inner join games_store on games_store.game_id = games.id
where games.id = ?
group by games.id, games.name, games_store.store_id, games.summary, games.release_date
    ", game_id).fetch_one(&db_state.pool).await.map_err(|e| e.to_string())?;

    let mut is_installed = false;

    if let Some(store_id) = game.store_id.clone() {
        is_installed = steam_client.is_steam_game_install(store_id);
    }

    Ok(Game {
        id: game.id,
        release_date: game.release_date,
        name: game.name,
        developers: game.studios.map(|val| {
            val.split(',')
                .map(|val| Some(val.to_string()))
                .collect::<Vec<_>>()
        }),
        genres: game.genres.map(|val| {
            val.split(',')
                .map(|val| val.to_string())
                .collect::<Vec<_>>()
        }),
        is_installed: Some(is_installed),
        summary: game.summary,
        artworks: game.artworks.map(|val| {
            val.split(',')
                .map(|val| val.to_string())
                .collect::<Vec<_>>()
        }),
        cover: game.covers.and_then(|val| {
            val.split(',')
                .map(|val| val.to_string())
                .collect::<Vec<_>>()
                .pop()
        }),
        store_id: game.store_id,
    })
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
