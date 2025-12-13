use std::future::Future;

use crate::twitch::TwitchApiClient;
use tauri::http::{HeaderMap, HeaderValue, StatusCode};
use tauri_plugin_http::reqwest::{Client, Response};

pub struct IgdbApiClient {
    twitch_client: TwitchApiClient,
    client: Client,
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

        let body = res.text().await.map_err(|e| e.to_string())?;

        dbg!(body);

        Ok(())
    }

    async fn request_with_retry<F, Fut>(&mut self, request_fn: F) -> Result<Response, String>
    where
        F: Fn(Client, String) -> Fut,
        Fut: Future<Output = Result<Response, String>>,
    {
        let token = match self.twitch_client.get_access_token() {
            Some(t) => t,
            None => {
                let t = self
                    .twitch_client
                    .refresh_access_token()
                    .await
                    .map_err(|e| e.to_string())?;
                t
            }
        };

        let response = request_fn(self.client.clone(), token).await?;

        match response.status() {
            StatusCode::UNAUTHORIZED => {
                self.twitch_client.refresh_access_token().await?;

                match self.twitch_client.get_access_token() {
                    Some(new_token) => request_fn(self.client.clone(), new_token).await,
                    None => Err("unable to make request. missing auth".to_string()),
                }
            }
            _ => Ok(response),
        }
    }
}
