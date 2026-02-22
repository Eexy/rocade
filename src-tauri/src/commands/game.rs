//! Tauri commands for game management.
//!
//! Exposes the application's game-related operations to the frontend:
//! listing and searching games, refreshing the library from Steam and IGDB,
//! retrieving a single game with its install status, and triggering
//! Steam install/uninstall actions.

use std::collections::{HashMap, HashSet};

use crate::{
    assets::{AssetError, AssetManager},
    client::steam::{SteamClient, SteamClientError},
    db::{
        game::{Game, GameRepository},
        DatabaseState,
    },
    igdb::{IgdbApiClient, IgdbError, IgdbGame},
    service::steam::{SteamApiClient, SteamError},
};
use serde::{Deserialize, Serialize};
use tauri::{async_runtime::Mutex, AppHandle, State};
use thiserror::Error;

/// Top-level error type returned by all Tauri commands in this module.
///
/// Serialized as a plain string message so the frontend receives a
/// human-readable error rather than a structured object.
#[derive(Debug, Error)]
pub enum RocadeError {
    /// A database operation failed.
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),
    /// A Steam API operation failed.
    #[error("steam api error: {0}")]
    Steam(#[from] SteamError),
    /// A Steam client operation failed.
    #[error("steam local client error: {0}")]
    SteamLocalClient(#[from] SteamClientError),
    /// An IGDB API operation failed.
    #[error("igdb error: {0}")]
    Igdb(#[from] IgdbError),
    /// An asset management operation failed.
    #[error("asset error: {0}")]
    Asset(#[from] AssetError),
}

impl Serialize for RocadeError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

/// Optional filter parameters accepted by [`get_games`].
#[derive(Deserialize, Debug)]
pub struct GameQuery {
    /// When set, only games whose name matches this string are returned.
    name: Option<String>,
}

/// Returns all games in the local database, optionally filtered by name.
///
/// Filtering applies a case-insensitive substring check first; if that
/// fails, a trigram similarity score above `0.4` is used as a fallback
/// for fuzzy matching.
#[tauri::command]
pub async fn get_games(
    game_repository: State<'_, GameRepository>,
    query: Option<GameQuery>,
) -> Result<Vec<Game>, RocadeError> {
    let mut games = game_repository.get_games().await?;

    if let Some(name) = query.and_then(|q| q.name) {
        games = games
            .into_iter()
            .filter(|game| {
                let game_name_lower = game.name.to_ascii_lowercase();

                if game_name_lower.contains(&name.to_ascii_lowercase()) {
                    return true;
                }

                similarity(&name.to_ascii_lowercase(), &game.name.to_ascii_lowercase()) > 0.4
            })
            .collect();
    }

    Ok(games)
}

/// Computes the set of trigrams for a string.
///
/// The input is padded with two leading spaces and one trailing space before
/// extracting all overlapping three-character windows. Used by [`similarity`]
/// for fuzzy name matching.
pub fn trigrams(s: &str) -> HashSet<String> {
    let s_with_spaces = format!("  {} ", s);
    let chars: Vec<char> = s_with_spaces.chars().collect();
    let mut hashset: HashSet<String> = HashSet::new();

    for window in chars.windows(3) {
        hashset.insert(window.iter().collect());
    }

    hashset
}

/// Returns a trigram-based similarity score between two strings in `[0.0, 1.0]`.
///
/// Computed as `|trigrams(a) ∩ trigrams(b)| / |trigrams(a)|`. A score of
/// `1.0` means `a`'s trigrams are a subset of `b`'s; `0.0` means no overlap.
pub fn similarity(a: &str, b: &str) -> f64 {
    let tri_a = trigrams(a);
    let tri_b = trigrams(b);

    tri_a.intersection(&tri_b).count() as f64 / tri_a.len() as f64
}

/// Refreshes the local game library from Steam and IGDB.
///
/// Fetches the user's owned games from Steam, enriches each entry with
/// metadata from IGDB (cover art, genres, companies, etc.), wipes the
/// existing database records, downloads all game images locally, and inserts
/// the updated set with local image paths.
#[tauri::command]
pub async fn refresh_games(
    steam_client: State<'_, SteamApiClient>,
    igdb_client: State<'_, Mutex<IgdbApiClient>>,
    asset_manager: State<'_, AssetManager>,
    db_state: State<'_, DatabaseState>,
    game_repository: State<'_, GameRepository>,
) -> Result<(), RocadeError> {
    // 1. Fetch games from Steam
    let games_res = steam_client.get_games().await?;

    // 2. Fetch IGDB metadata
    let mut locked_client = igdb_client.lock().await;
    let igdb_games = locked_client
        .get_games(games_res.iter().map(|game| game.appid).collect())
        .await?;

    // 3. Clear database and assets
    prepare_db(&db_state, &asset_manager).await?;

    // 4. Collect image IDs from all games
    let mut cover_ids = Vec::new();
    let mut artwork_ids = Vec::new();

    for game in &igdb_games {
        if let Some(cover) = &game.cover {
            cover_ids.push(cover.image_id.clone());
        }
        if let Some(artworks) = &game.artworks {
            for artwork in artworks {
                artwork_ids.push(artwork.image_id.clone());
            }
        }
    }

    // 5. Download images in parallel
    let cover_paths = asset_manager.download_batch_covers(cover_ids).await?;
    let artwork_paths = asset_manager.download_batch_artworks(artwork_ids).await?;

    // 6. Build maps: image_id -> local_path
    let cover_map: HashMap<String, String> = cover_paths.into_iter().collect();
    let artwork_map: HashMap<String, String> = artwork_paths.into_iter().collect();

    // 7. Insert games and update image paths
    insert_games_with_images(&game_repository, igdb_games, cover_map, artwork_map).await?;

    Ok(())
}

/// Clears all existing game records from the database and cached assets.
async fn prepare_db(
    db_state: &DatabaseState,
    asset_manager: &AssetManager,
) -> Result<(), RocadeError> {
    db_state.clean().await?;
    asset_manager.clear_all().await?;
    Ok(())
}

/// Inserts a batch of IGDB games into the database and updates their image paths.
async fn insert_games_with_images(
    game_repository: &GameRepository,
    games: Vec<IgdbGame>,
    cover_map: HashMap<String, String>,
    artwork_map: HashMap<String, String>,
) -> Result<(), sqlx::Error> {
    for game in games {
        // Get cover and artwork info before move
        let cover_id = game.cover.as_ref().map(|c| c.image_id.clone());
        let artwork_ids: Vec<String> = game
            .artworks
            .as_ref()
            .map(|artworks| artworks.iter().map(|a| a.image_id.clone()).collect())
            .unwrap_or_default();

        // Insert game
        let game_id = game_repository.insert_complete_game(game).await?;

        // Update cover path if downloaded
        if let Some(cover_id) = cover_id {
            if let Some(local_path) = cover_map.get(&cover_id) {
                game_repository
                    .update_cover_path(game_id, &cover_id, local_path)
                    .await?;
            }
        }

        // Update artwork paths
        let downloaded_artworks: Vec<(String, String)> = artwork_ids
            .into_iter()
            .filter_map(|id| {
                artwork_map.get(&id).map(|path| (id.clone(), path.clone()))
            })
            .collect();

        if !downloaded_artworks.is_empty() {
            game_repository
                .update_artwork_paths(game_id, downloaded_artworks)
                .await?;
        }
    }

    Ok(())
}

/// Returns a single game by its database ID, with its current install status.
///
/// Looks up the game's Steam store ID and checks the local Steam library to
/// determine whether the game is fully installed, then sets `is_installed`
/// on the returned record.
#[tauri::command]
pub async fn get_game(
    game_repository: State<'_, GameRepository>,
    steam_client: State<'_, SteamClient>,
    game_id: i64,
) -> Result<Game, RocadeError> {
    let mut game = game_repository.get_game_by_id(game_id).await?;

    let mut is_installed = false;

    if let Some(store_id) = game.store_id.clone() {
        is_installed = steam_client.is_steam_game_installed(&store_id);
    }

    game.is_installed = Some(is_installed);

    Ok(game)
}

/// Triggers installation of a game via the Steam client.
///
/// Resolves the game's Steam store ID from the database and opens the
/// `steam://install/<id>` URL. Returns `true` if the URL was dispatched
/// successfully; the actual download is handled asynchronously by Steam.
#[tauri::command]
pub async fn install_game(
    game_repository: State<'_, GameRepository>,
    app: AppHandle,
    game_id: i64,
) -> Result<bool, RocadeError> {
    let store_id = game_repository.get_game_store_id(game_id).await?;

    SteamClient::install_game(app, &store_id)?;

    Ok(true)
}

/// Triggers uninstallation of a game via the Steam client.
///
/// Resolves the game's Steam store ID from the database and opens the
/// `steam://uninstall/<id>` URL. Returns `true` if the URL was dispatched
/// successfully.
#[tauri::command]
pub async fn uninstall_game(
    game_repository: State<'_, GameRepository>,
    app: AppHandle,
    game_id: i64,
) -> Result<bool, RocadeError> {
    let store_id = game_repository.get_game_store_id(game_id).await?;

    SteamClient::uninstall_game(app, store_id)?;
    Ok(true)
}
