//! Tauri commands for game management.
//!
//! Exposes the application's game-related operations to the frontend:
//! listing and searching games, refreshing the library from Steam and IGDB,
//! retrieving a single game with its install status, and triggering
//! Steam install/uninstall actions.

use std::collections::HashSet;

use crate::{
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
/// existing database records, and inserts the updated set.
#[tauri::command]
pub async fn refresh_games(
    steam_client: State<'_, SteamApiClient>,
    igdb_client: State<'_, Mutex<IgdbApiClient>>,
    db_state: State<'_, DatabaseState>,
    game_repository: State<'_, GameRepository>,
) -> Result<(), RocadeError> {
    let games_res = steam_client.get_games().await?;

    let mut locked_client = igdb_client.lock().await;

    let igdb_games = locked_client
        .get_games(games_res.iter().map(|game| game.appid).collect())
        .await?;

    prepare_db(db_state.clone()).await?;

    insert_games(game_repository, igdb_games).await?;

    Ok(())
}

/// Clears all existing game records from the database in preparation for a
/// fresh import.
async fn prepare_db(db_state: State<'_, DatabaseState>) -> Result<(), sqlx::Error> {
    db_state.clean().await
}

/// Inserts a batch of IGDB games into the database.
async fn insert_games(
    game_repository: State<'_, GameRepository>,
    games: Vec<IgdbGame>,
) -> Result<(), sqlx::Error> {
    for game in games {
        let _id = game_repository.insert_complete_game(game).await?;
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
