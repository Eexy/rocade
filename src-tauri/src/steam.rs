use std::{fs, path::PathBuf};

use serde::{Deserialize, Serialize};
use tauri::AppHandle;
use tauri_plugin_http::reqwest::Client;
use tauri_plugin_opener::OpenerExt;

#[derive(Debug)]
pub struct SteamApiClient {
    key: String,
    profile_id: String,
    client: Client,
}

#[derive(Serialize, Deserialize)]
pub struct SteamGame {
    pub appid: u64,
    name: String,
    playtime_2weeks: Option<u64>,
    playtime_forever: Option<u64>,
    img_icon_url: Option<String>,
    img_logo_url: Option<String>,
}

#[derive(Serialize, Deserialize)]
pub struct GameList {
    game_count: u64,
    games: Vec<SteamGame>,
}

#[derive(Serialize, Deserialize)]
pub struct GameListResponse {
    response: GameList,
}

impl SteamApiClient {
    pub fn new(key: String, profile_id: String) -> Self {
        SteamApiClient {
            key,
            profile_id,
            client: tauri_plugin_http::reqwest::Client::new(),
        }
    }

    pub async fn get_games(&self) -> Result<Vec<SteamGame>, String> {
        let url = format!("http://api.steampowered.com/IPlayerService/GetOwnedGames/v0001/?key={}&steamid={}&include_appinfo=1&format=json", self.key, self.profile_id);
        let res = self
            .client
            .get(url)
            .send()
            .await
            .map_err(|e| e.to_string())?;

        let body = res.text().await.map_err(|e| e.to_string())?;

        let parsed: GameListResponse = serde_json::from_str(&body).map_err(|e| e.to_string())?;

        Ok(parsed.response.games)
    }
}

pub struct SteamClient {}

impl SteamClient {
    pub fn new() -> Self {
        SteamClient {}
    }

    fn get_steam_dir(&self) -> Result<PathBuf, String> {
        use std::env;

        let mut user_dir = match env::home_dir() {
            Some(path) => path,
            None => return Err("unable to get user home directory".to_string()),
        };

        user_dir.push(r".local");
        user_dir.push("share");
        user_dir.push("Steam");
        user_dir.push("steamapps");

        Ok(user_dir)
    }

    fn get_game_manifest_file_path(&self, steam_game_id: String) -> Result<PathBuf, String> {
        let mut steam_dir = self.get_steam_dir()?;
        steam_dir.push(format!("appmanifest_{}", steam_game_id));
        steam_dir.set_extension("acf");
        Ok(steam_dir)
    }

    pub fn install_game(
        &self,
        app_handle: AppHandle,
        steam_game_id: String,
    ) -> Result<bool, String> {
        app_handle
            .opener()
            .open_url(format!("steam://install/{}", steam_game_id), None::<&str>)
            .map_err(|e| e.to_string())?;

        Ok(true)
    }

    pub fn uninstall_game(
        &self,
        app_handle: AppHandle,
        steam_game_id: String,
    ) -> Result<bool, String> {
        app_handle
            .opener()
            .open_url(format!("steam://uninstall/{}", steam_game_id), None::<&str>)
            .map_err(|e| e.to_string())?;

        Ok(true)
    }

    pub fn is_steam_game_install(&self, game_id: String) -> bool {
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
