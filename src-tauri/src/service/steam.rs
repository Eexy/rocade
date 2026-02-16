//! Steam Web API client.
//!
//! Provides an async client for querying the Steam Web API to retrieve a
//! user's owned games, along with the shared [`SteamError`] type used across
//! the Steam integration.

use serde::{Deserialize, Serialize};
use tauri_plugin_http::reqwest::{self, Client};

/// Errors that can occur when using Steam API or client operations.
#[derive(Debug, thiserror::Error)]
pub enum SteamError {
    /// An HTTP request to the Steam Web API failed.
    #[error("http request failed: {0}")]
    Request(#[from] reqwest::Error),

    /// The Steam Web API returned an unexpected or malformed response.
    #[error("invalid response: {0}")]
    InvalidResponse(String),

    /// The response body could not be deserialized into the expected type.
    #[error("unable to parse steam data: {0}")]
    InvalidData(#[from] serde_json::Error),

    /// Opening a `steam://` URL via the OS opener failed.
    #[error("unable to communicate with steam client: {0}")]
    ClientError(#[from] tauri_plugin_opener::Error),

    /// The Steam client configuration (e.g. library path) is invalid.
    #[error("invalid steam client config")]
    ClientConfigError(String),
}

/// A game entry as returned by the Steam `GetOwnedGames` endpoint.
#[derive(Serialize, Deserialize)]
pub struct SteamGame {
    /// Steam App ID.
    pub appid: u64,
    name: String,
    /// Playtime in minutes over the last two weeks, if any.
    playtime_2weeks: Option<u64>,
    /// Total playtime in minutes.
    playtime_forever: Option<u64>,
    img_icon_url: Option<String>,
    img_logo_url: Option<String>,
}

/// The inner payload of the `GetOwnedGames` response.
#[derive(Serialize, Deserialize)]
pub struct GameList {
    game_count: u64,
    games: Vec<SteamGame>,
}

/// Top-level wrapper for the `GetOwnedGames` JSON response.
#[derive(Serialize, Deserialize)]
pub struct GameListResponse {
    response: GameList,
}

/// Async client for the Steam Web API.
///
/// Requires a Steam Web API `key` and the target user's 64-bit `profile_id`
/// (SteamID64).
#[derive(Debug)]
pub struct SteamApiClient {
    key: String,
    profile_id: String,
    client: Client,
}

impl SteamApiClient {
    /// Creates a new Steam Web API client.
    ///
    /// # Arguments
    ///
    /// * `key` — Steam Web API key.
    /// * `profile_id` — SteamID64 of the target user profile.
    pub fn new(key: String, profile_id: String) -> Self {
        SteamApiClient {
            key,
            profile_id,
            client: tauri_plugin_http::reqwest::Client::new(),
        }
    }

    /// Fetches all games owned by the configured Steam profile.
    ///
    /// Calls the `IPlayerService/GetOwnedGames` endpoint with `include_appinfo`
    /// enabled so that each entry includes the game name and icon URLs.
    pub async fn get_games(&self) -> Result<Vec<SteamGame>, SteamError> {
        let url = format!("https://api.steampowered.com/IPlayerService/GetOwnedGames/v0001");
        let res = self
            .client
            .get(url)
            .query(&[
                ("key", &self.key),
                ("steamid", &self.profile_id),
                ("include_appinfo", &"1".to_string()),
                ("format", &"json".to_string()),
            ])
            .send()
            .await?;

        let body = res.text().await?;

        let parsed: GameListResponse = serde_json::from_str(&body)?;

        Ok(parsed.response.games)
    }
}
