use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use tauri_plugin_http::reqwest::Client;

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

    pub fn get_steam_dir(&self) -> Result<PathBuf, String> {
        use std::env;

        let mut user_dir = match env::home_dir() {
            Some(path) => path,
            None => return Err("unable to get user home directory".to_string()),
        };

        user_dir.push(r".local");
        user_dir.push("share");
        user_dir.push("Steam");
        user_dir.push("steamapps");

        user_dir.try_exists().map_err(|e| e.to_string())?;

        return Ok(user_dir);
    }
}
