use crate::{
    db::{
        artwork::ArtworkRepository, cover::CoverRepository, game::GameRepository,
        game_store::GameStoreRepository, genre::GenreRepository, studio::CompanyRepository,
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
    developers: Option<Vec<String>>,
}

#[tauri::command]
pub async fn get_games(db_state: State<'_, DatabaseState>) -> Result<Vec<Game>, String> {
    let games = GameRepository::get_games(&db_state.pool)
        .await
        .map_err(|e| e.to_string())?;

    let parsed_games = games
        .into_iter()
        .map(|game| Game {
            id: game.id,
            release_date: game.release_date,
            name: game.name,
            developers: game
                .studios
                .as_ref()
                .and_then(|s| serde_json::from_str::<Vec<String>>(s).ok()),
            genres: game
                .genres
                .as_ref()
                .and_then(|s| serde_json::from_str::<Vec<String>>(s).ok()),
            is_installed: None,
            summary: game.summary,
            artworks: game
                .artworks
                .as_ref()
                .and_then(|s| serde_json::from_str::<Vec<String>>(s).ok()),
            cover: game.covers.as_ref().and_then(|s| {
                serde_json::from_str::<Vec<String>>(s)
                    .ok()
                    .and_then(|mut v| v.pop())
            }),
            store_id: game.store_id,
        })
        .collect();

    Ok(parsed_games)
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

        CompanyRepository::insert_companies(&db_state.pool, id, game.developers).await?;

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

    let mut is_installed = false;

    if let Some(store_id) = game.store_id.clone() {
        is_installed = steam_client.is_steam_game_install(store_id);
    }

    Ok(Game {
        id: game.id,
        release_date: game.release_date,
        name: game.name,
        developers: game
            .studios
            .as_ref()
            .and_then(|s| serde_json::from_str::<Vec<String>>(s).ok()),
        genres: game
            .genres
            .as_ref()
            .and_then(|s| serde_json::from_str::<Vec<String>>(s).ok()),
        is_installed: Some(is_installed),
        summary: game.summary,
        artworks: game
            .artworks
            .as_ref()
            .and_then(|s| serde_json::from_str::<Vec<String>>(s).ok()),
        cover: game.covers.as_ref().and_then(|s| {
            serde_json::from_str::<Vec<String>>(s)
                .ok()
                .and_then(|mut v| v.pop())
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
