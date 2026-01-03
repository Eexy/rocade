use serde::{Deserialize, Serialize};
use tauri_plugin_http::reqwest::Client;

#[derive(Debug)]
pub struct SteamState {
    key: String,
    profile_id: String,
}

impl SteamState {
    pub fn new(key: String, profile_id: String) -> Self {
        return SteamState { key, profile_id };
    }
}

#[derive(Debug)]
pub struct SteamApiClient {
    key: String,
    profile_id: String,
    client: Client,
}

#[derive(Serialize, Deserialize)]
pub struct Game {
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
    games: Vec<Game>,
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

    pub async fn get_games(&self) -> Result<Vec<Game>, String> {
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
