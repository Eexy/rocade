use std::ops::Deref;

use serde::Deserialize;
use tauri_plugin_http::reqwest::{self, Client};

#[derive(Debug)]
pub struct TwitchApiClient {
    client_id: String,
    client_secret: String,
    access_token: Option<String>,
    client: Client,
}

#[derive(Deserialize)]
pub struct TwitchAuthResponse {
    access_token: String,
}

#[derive(Debug, thiserror::Error)]
pub enum TwitchError {
    #[error("http request failed: {0}")]
    Request(#[from] reqwest::Error),

    #[error("unable to parse data: {0}")]
    InvalidData(#[from] serde_json::Error),
}

impl TwitchApiClient {
    pub fn new(client_id: String, client_secret: String) -> Self {
        TwitchApiClient {
            client_id,
            client_secret,
            access_token: None,
            client: tauri_plugin_http::reqwest::Client::new(),
        }
    }

    pub fn get_client_id(&self) -> &str {
        self.client_id.deref()
    }

    pub async fn refresh_access_token(&mut self) -> Result<&str, TwitchError> {
        let url = "https://id.twitch.tv/oauth2/token";
        let res = self
            .client
            .post(url)
            .form(&[
                ("client_id", &self.client_id),
                ("client_secret", &self.client_secret),
                ("grant_type", &"client_credentials".to_string()),
            ])
            .send()
            .await?;

        let body = res.text().await?;

        let parsed: TwitchAuthResponse = serde_json::from_str(&body)?;

        self.access_token = Some(parsed.access_token);

        Ok(self.access_token.as_deref().unwrap())
    }

    pub fn get_access_token(&self) -> Option<&str> {
        self.access_token.as_deref()
    }
}
