//! Local Steam client utilities.
//!
//! Interacts with the locally installed Steam client to install, uninstall,
//! and check the installation state of games by reading ACF manifest files
//! and opening `steam://` protocol URLs via the OS.

use std::{fs, path::PathBuf};

use tauri::AppHandle;
use tauri_plugin_opener::OpenerExt;

use crate::service::steam::SteamError;

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
    fn get_game_manifest_file_path(&self, steam_game_id: &str) -> Result<PathBuf, String> {
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
    pub fn install_game(app_handle: AppHandle, steam_game_id: &str) -> Result<bool, SteamError> {
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
    pub fn is_steam_game_installed(&self, game_id: &str) -> bool {
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
