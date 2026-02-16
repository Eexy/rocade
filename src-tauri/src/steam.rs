//! Steam API client and local Steam client utilities.
//!
//! Provides two independent entry points:
//! - [`SteamApiClient`] — queries the Steam Web API for a user's owned games.
//! - [`SteamClient`] — interacts with the locally installed Steam client to
//!   install, uninstall, and check the installation state of games.

use std::{fs, path::PathBuf};

use serde::{Deserialize, Serialize};
use tauri::AppHandle;
use tauri_plugin_http::reqwest::{self, Client};
use tauri_plugin_opener::OpenerExt;

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

/// Interface to the locally installed Steam client.
///
/// Operates on the Steam library directory to check installation state and
/// triggers install/uninstall actions by opening `steam://` protocol URLs via
/// the OS.
pub struct SteamClient {
    /// Path to the Steam library `steamapps` directory.
    path: PathBuf,
}

impl SteamClient {
    /// Creates a new `SteamClient` pointing at the given Steam library path.
    ///
    /// `path` should be the `steamapps` directory inside the Steam library
    /// (e.g. `~/.steam/steam/steamapps`).
    pub fn new(path: PathBuf) -> Self {
        SteamClient { path }
    }

    /// Returns the expected path to the ACF manifest file for a given game.
    ///
    /// Steam stores per-game metadata in `appmanifest_<id>.acf` files inside
    /// the library's `steamapps` directory.
    fn get_game_manifest_file_path(&self, steam_game_id: String) -> Result<PathBuf, String> {
        let mut steam_dir = self.path.clone();
        steam_dir.push(format!("appmanifest_{}", steam_game_id));
        steam_dir.set_extension("acf");
        Ok(steam_dir)
    }

    /// Triggers installation of a Steam game via the `steam://install` protocol.
    ///
    /// Opens the URL in the OS default handler, which hands control to the
    /// running Steam client. Returns `true` if the URL was opened successfully;
    /// the actual download happens asynchronously inside Steam.
    pub fn install_game(app_handle: AppHandle, steam_game_id: String) -> Result<bool, SteamError> {
        app_handle
            .opener()
            .open_url(format!("steam://install/{}", steam_game_id), None::<&str>)?;

        Ok(true)
    }

    /// Triggers uninstallation of a Steam game via the `steam://uninstall` protocol.
    ///
    /// Opens the URL in the OS default handler, which hands control to the
    /// running Steam client. Returns `true` if the URL was opened successfully.
    pub fn uninstall_game(
        app_handle: AppHandle,
        steam_game_id: String,
    ) -> Result<bool, SteamError> {
        app_handle
            .opener()
            .open_url(format!("steam://uninstall/{}", steam_game_id), None::<&str>)?;

        Ok(true)
    }

    /// Returns `true` if the game is fully installed in this Steam library.
    ///
    /// Checks for the presence of the game's ACF manifest file and then reads
    /// its `BytesToDownload` and `BytesDownloaded` fields. A game is considered
    /// installed only when both values are present and equal, meaning no pending
    /// download remains.
    pub fn is_steam_game_installed(&self, game_id: String) -> bool {
        let manifest_file = match self.get_game_manifest_file_path(game_id) {
            Ok(file) => file,
            Err(_) => return false,
        };

        let exist = match manifest_file.try_exists() {
            Ok(exist) => exist,
            Err(_) => false,
        };

        if !exist {
            return false;
        }

        let content = match fs::read_to_string(manifest_file) {
            Ok(contents) => contents,
            Err(_) => return false,
        };

        let mut bytes_to_download: Option<i64> = None;
        let mut bytes_downloaded: Option<i64> = None;

        for line in content.lines() {
            let mut parts = line.trim().split_whitespace();
            if let (Some(property), Some(value)) = (parts.next(), parts.next()) {
                match property {
                    "\"BytesToDownload\"" => {
                        bytes_to_download = value.trim_matches('"').parse().ok();
                    }
                    "\"BytesDownloaded\"" => {
                        bytes_downloaded = value.trim_matches('"').parse().ok();
                    }
                    _ => continue,
                }

                if bytes_to_download.is_some() && bytes_downloaded.is_some() {
                    break;
                }
            }
        }

        matches!((bytes_to_download, bytes_downloaded), (Some(to_dl), Some(downloaded)) if to_dl == downloaded)
    }
}
