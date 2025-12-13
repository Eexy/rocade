use serde::Deserialize;
use tauri_plugin_http::reqwest::Client;

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

impl TwitchApiClient {
    pub fn new(client_id: String, client_secret: String) -> Self {
        return TwitchApiClient {
            client_id,
            client_secret,
            access_token: None,
            client: tauri_plugin_http::reqwest::Client::new(),
        };
    }

    pub async fn auth(&mut self) -> Result<(), String> {
        let url = format!("https://id.twitch.tv/oauth2/token?client_id={}&client_secret={}&grant_type=client_credentials", self.client_id, self.client_secret);
        let res = self
            .client
            .post(url)
            .send()
            .await
            .map_err(|e| e.to_string())?;

        let body = res.text().await.map_err(|e| e.to_string())?;

        let parsed: TwitchAuthResponse = serde_json::from_str(&body).map_err(|e| e.to_string())?;

        self.access_token = Some(parsed.access_token);

        Ok(())
    }

    pub fn get_access_token(&self) -> Option<String> {
        self.access_token.clone()
    }
}
