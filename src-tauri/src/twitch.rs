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

    pub fn get_client_id(&self) -> String {
        self.client_id.clone()
    }

    pub async fn refresh_access_token(&mut self) -> Result<String, TwitchError> {
        let url = format!("https://id.twitch.tv/oauth2/token?client_id={}&client_secret={}&grant_type=client_credentials", self.client_id, self.client_secret);
        let res = self.client.post(url).send().await?;

        let body = res.text().await?;

        let parsed: TwitchAuthResponse = serde_json::from_str(&body)?;

        self.access_token = Some(parsed.access_token.clone());

        Ok(parsed.access_token.clone())
    }

    pub fn get_access_token(&self) -> Option<String> {
        self.access_token.clone()
    }
}
