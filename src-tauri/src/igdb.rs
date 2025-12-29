use std::future::Future;

use crate::{igdb, steam, twitch::TwitchApiClient};
use serde::{Deserialize, Serialize};
use tauri::http::{HeaderMap, HeaderValue, StatusCode};
use tauri_plugin_http::reqwest::{Client, Response};

#[derive(Serialize, Deserialize, Debug)]
pub struct IgdbGameInfo {
    name: String,
    cover: u64,
    genres: Vec<u64>,
    storyline: Option<String>,
    summary: Option<String>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct IgdbGenre {
    name: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct IgdbCover {
    image_id: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct IgdbGame {
    name: String,
    storyline: Option<String>,
    summary: Option<String>,
    genres: Vec<IgdbGenre>,
    covers: Vec<IgdbCover>,
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

        let game_genres = self
            .get_game_genres(game_info.genres)
            .await
            .map_err(|e| e.to_string())?;

        let game_cover = self
            .get_game_cover(game_info.cover)
            .await
            .map_err(|e| e.to_string())?;

        let game = IgdbGame {
            name: game_info.name,
            summary: game_info.summary,
            storyline: game_info.storyline,
            genres: game_genres,
            covers: game_cover,
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
            .request_with_retry(|client, token| {
                let value = query.clone();
                async move {
                    client
                        .post(URL)
                        .bearer_auth(token)
                        .body(value)
                        .send()
                        .await
                        .map_err(|e| e.to_string())
                }
            })
            .await
            .map_err(|e| e.to_string())?;

        let body = res.text().await.map_err(|e| e.to_string())?;

        let mut parsed =
            serde_json::from_str::<Vec<IgdbAlternativeGame>>(&body).map_err(|e| e.to_string())?;

        match parsed.pop() {
            Some(game) => Ok(game),
            None => Err("Unable to find game".to_string()),
        }
    }

    async fn search_game(&mut self, game_name: String) -> Result<IgdbGameInfo, String> {
        const URL: &str = "https://api.igdb.com/v4/games";
        let query = format!(
            "fields *; search \"{}\"; where game.platforms = 6; limit 1;",
            game_name
        );
        let res = self
            .request_with_retry(|client, token| {
                let value = query.clone();
                async move {
                    client
                        .post(URL)
                        .bearer_auth(token)
                        .body(value)
                        .send()
                        .await
                        .map_err(|e| e.to_string())
                }
            })
            .await
            .map_err(|e| e.to_string())?;

        let body = res.text().await.map_err(|e| e.to_string())?;

        let mut parsed =
            serde_json::from_str::<Vec<IgdbGameInfo>>(&body).map_err(|e| e.to_string())?;

        match parsed.pop() {
            Some(game) => Ok(game),
            None => Err("Unable to find game".to_string()),
        }
    }

    async fn get_game_info(&mut self, igdb_game_id: i64) -> Result<IgdbGameInfo, String> {
        const URL: &str = "https://api.igdb.com/v4/games";
        let query = format!("fields *; where id = {}; limit 1;", igdb_game_id);
        let res = self
            .request_with_retry(|client, token| {
                let value = query.clone();
                async move {
                    client
                        .post(URL)
                        .bearer_auth(token)
                        .body(value)
                        .send()
                        .await
                        .map_err(|e| e.to_string())
                }
            })
            .await
            .map_err(|e| e.to_string())?;

        let body = res.text().await.map_err(|e| e.to_string())?;

        let mut parsed =
            serde_json::from_str::<Vec<IgdbGameInfo>>(&body).map_err(|e| e.to_string())?;

        match parsed.pop() {
            Some(game) => Ok(game),
            None => Err("Unable to find game".to_string()),
        }
    }

    async fn get_game_genres(&mut self, genres: Vec<u64>) -> Result<Vec<IgdbGenre>, String> {
        const URL: &str = "https://api.igdb.com/v4/genres";
        let query = format!(
            "fields *; where id = ({});",
            genres
                .iter()
                .map(|genre| genre.to_string())
                .collect::<Vec<_>>()
                .join(",")
        );
        let res = self
            .request_with_retry(|client, token| {
                let value = query.clone();
                async move {
                    client
                        .post(URL)
                        .bearer_auth(token)
                        .body(value)
                        .send()
                        .await
                        .map_err(|e| e.to_string())
                }
            })
            .await
            .map_err(|e| e.to_string())?;

        let body = res.text().await.map_err(|e| e.to_string())?;

        let parsed = serde_json::from_str::<Vec<IgdbGenre>>(&body).map_err(|e| e.to_string())?;

        Ok(parsed)
    }

    async fn get_game_cover(&mut self, cover: u64) -> Result<Vec<IgdbCover>, String> {
        const URL: &str = "https://api.igdb.com/v4/covers";
        let query = format!("fields *; where id = ({});", cover);
        let res = self
            .request_with_retry(|client, token| {
                let value = query.clone();
                async move {
                    client
                        .post(URL)
                        .bearer_auth(token)
                        .body(value)
                        .send()
                        .await
                        .map_err(|e| e.to_string())
                }
            })
            .await
            .map_err(|e| e.to_string())?;

        let body = res.text().await.map_err(|e| e.to_string())?;

        let parsed = serde_json::from_str::<Vec<IgdbCover>>(&body).map_err(|e| e.to_string())?;

        Ok(parsed)
    }

    async fn get_twitch_access_token(&mut self) -> Result<String, String> {
        match self.twitch_client.get_access_token() {
            Some(token) => Ok(token),
            None => self.twitch_client.refresh_access_token().await,
        }
    }

    async fn request_with_retry<F, Fut>(&mut self, request_fn: F) -> Result<Response, String>
    where
        F: Fn(Client, String) -> Fut,
        Fut: Future<Output = Result<Response, String>>,
    {
        let token = self
            .get_twitch_access_token()
            .await
            .map_err(|e| e.to_string())?;

        let response = request_fn(self.client.clone(), token).await?;

        if response.status() == StatusCode::UNAUTHORIZED {
            let new_token = self.twitch_client.refresh_access_token().await?;
            request_fn(self.client.clone(), new_token).await
        } else {
            Ok(response)
        }
    }
}
