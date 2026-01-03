use crate::twitch::TwitchApiClient;
use serde::{Deserialize, Serialize};
use tauri::http::{HeaderMap, HeaderValue, StatusCode};
use tauri_plugin_http::reqwest::{Client, Response};

#[derive(Serialize, Deserialize, Debug)]
pub struct IgdbGenre {
    name: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct IgdbCover {
    image_id: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct IgdbGameInfo {
    name: String,
    cover: IgdbCover,
    genres: Vec<IgdbGenre>,
    storyline: Option<String>,
    summary: Option<String>,
    artworks: Vec<IgdbCover>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct IgdbGame {
    name: String,
    storyline: Option<String>,
    summary: Option<String>,
    genres: Vec<IgdbGenre>,
    cover: IgdbCover,
    artworks: Vec<IgdbCover>,
}

#[derive(Debug)]
pub struct IgdbApiClient {
    twitch_client: TwitchApiClient,
    client: Client,
}

#[derive(Deserialize)]
pub struct IgdbAlternativeGame {
    game: i64,
}

impl IgdbApiClient {
    pub fn new(twitch_client: TwitchApiClient) -> Self {
        let mut headers = HeaderMap::new();

        headers.insert(
            "CLIENT-ID",
            HeaderValue::from_str(twitch_client.get_client_id().as_str())
                .expect("unable to set igdb client id"),
        );

        return IgdbApiClient {
            twitch_client,
            client: tauri_plugin_http::reqwest::Client::builder()
                .default_headers(headers)
                .build()
                .expect("unable to build igdb client"),
        };
    }

    pub async fn get_game(&mut self, steam_game_id: i64) -> Result<IgdbGame, String> {
        let steam_game = self
            .get_steam_game(steam_game_id)
            .await
            .map_err(|e| e.to_string())?;

        let game_info = self
            .get_game_info(steam_game.game)
            .await
            .map_err(|e| e.to_string())?;

        let game = IgdbGame {
            name: game_info.name,
            summary: game_info.summary,
            storyline: game_info.storyline,
            genres: game_info.genres,
            cover: game_info.cover,
            artworks: game_info.artworks,
        };

        Ok(game)
    }

    async fn get_steam_game(&mut self, game_id: i64) -> Result<IgdbAlternativeGame, String> {
        const URL: &str = "https://api.igdb.com/v4/external_games";
        let query = format!(
            "fields *;  where external_game_source = 1 & url = \"https://store.steampowered.com/app/{}\"; limit 1;",
            game_id
        );
        let res = self
            .request_with_retry(URL, &query)
            .await
            .map_err(|e| e.to_string())?;

        let body = res.text().await.map_err(|e| e.to_string())?;

        let mut parsed =
            serde_json::from_str::<Vec<IgdbAlternativeGame>>(&body).map_err(|e| e.to_string())?;

        parsed.pop().ok_or("Unable to find game".to_string())
    }

    async fn get_game_info(&mut self, igdb_game_id: i64) -> Result<IgdbGameInfo, String> {
        const URL: &str = "https://api.igdb.com/v4/games";
        let query = format!(
            "fields *, genres.name, artworks.image_id, cover.image_id; where id = {}; limit 1;",
            igdb_game_id
        );
        let res = self
            .request_with_retry(URL, &query)
            .await
            .map_err(|e| e.to_string())?;

        let body = res.text().await.map_err(|e| e.to_string())?;

        let mut parsed =
            serde_json::from_str::<Vec<IgdbGameInfo>>(&body).map_err(|e| e.to_string())?;

        parsed.pop().ok_or("Unable to find game".to_string())
    }

    async fn get_twitch_access_token(&mut self) -> Result<String, String> {
        match self.twitch_client.get_access_token() {
            Some(token) => Ok(token),
            None => self.twitch_client.refresh_access_token().await,
        }
    }

    async fn request_with_retry(&mut self, url: &str, query: &str) -> Result<Response, String> {
        let token = self
            .get_twitch_access_token()
            .await
            .map_err(|e| e.to_string())?;

        let response = self
            .client
            .post(url)
            .bearer_auth(&token)
            .body(query.to_string())
            .send()
            .await
            .map_err(|e| e.to_string())?;

        if response.status() == StatusCode::UNAUTHORIZED {
            let new_token = self.twitch_client.refresh_access_token().await?;
            self.client
                .post(url)
                .bearer_auth(&new_token)
                .body(query.to_string())
                .send()
                .await
                .map_err(|e| e.to_string())
        } else {
            Ok(response)
        }
    }
}
