use std::future::Future;

use crate::twitch::TwitchApiClient;
use serde::{Deserialize, Serialize};
use tauri::http::{HeaderMap, HeaderValue, StatusCode};
use tauri_plugin_http::reqwest::{Client, Response};

#[derive(Debug)]
pub struct IgdbApiClient {
    twitch_client: TwitchApiClient,
    client: Client,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct IgdbGame {
    name: String,
    artworks: Vec<u64>,
    genres: Vec<u64>,
    storyline: Option<String>,
    summary: Option<String>,
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

    pub async fn get_games(&mut self) -> Result<(), String> {
        const URL: &str = "https://api.igdb.com/v4/games";
        let res = self
            .request_with_retry(|client, token| async move {
                client
                    .post(URL)
                    .bearer_auth(token)
                    .body("fields *;".to_string())
                    .send()
                    .await
                    .map_err(|e| e.to_string())
            })
            .await
            .map_err(|e| e.to_string())?;

        let _ = res.text().await.map_err(|e| e.to_string())?;

        Ok(())
    }

    pub async fn get_game(&mut self, game_name: String) -> Result<IgdbGame, String> {
        let game = self
            .get_game_info(game_name.clone())
            .await
            .map_err(|e| e.to_string());

        game
    }

    async fn get_game_info(&mut self, name: String) -> Result<IgdbGame, String> {
        const URL: &str = "https://api.igdb.com/v4/games";
        let query = format!("fields *; where name = \"{}\"; limit 1;", name);
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

        let mut parsed = serde_json::from_str::<Vec<IgdbGame>>(&body).map_err(|e| e.to_string())?;

        match parsed.pop() {
            Some(game) => Ok(game),
            None => Err("Unable to find game".to_string()),
        }
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
