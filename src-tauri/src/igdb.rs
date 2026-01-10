use std::collections::HashMap;

use crate::twitch::TwitchApiClient;
use serde::{Deserialize, Serialize};
use tauri::http::{HeaderMap, HeaderValue, StatusCode};
use tauri_plugin_http::reqwest::{Client, Response};

#[derive(Serialize, Deserialize, Debug)]
pub struct IgdbGenre {
    name: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct IgdbImage {
    pub image_id: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct IgdbGameInfo {
    id: u64,
    name: String,
    cover: IgdbImage,
    genres: Vec<IgdbGenre>,
    storyline: Option<String>,
    summary: Option<String>,
    artworks: Option<Vec<IgdbImage>>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct IgdbGame {
    id: u64,
    pub name: String,
    pub store_id: Option<String>,
    storyline: Option<String>,
    pub summary: Option<String>,
    genres: Vec<IgdbGenre>,
    pub cover: IgdbImage,
    pub artworks: Option<Vec<IgdbImage>>,
}

#[derive(Debug)]
pub struct IgdbApiClient {
    twitch_client: TwitchApiClient,
    client: Client,
}

#[derive(Deserialize, Debug)]
pub struct IgdbAlternativeGame {
    game: u64,
    uid: String,
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

    pub async fn get_game(&mut self, steam_game_id: u64) -> Result<IgdbGame, String> {
        let steam_game = self
            .get_steam_game(steam_game_id)
            .await
            .map_err(|e| e.to_string())?;

        let store_id = steam_game_id.to_string();

        let game_info = self
            .get_game_info(steam_game.game)
            .await
            .map_err(|e| e.to_string())?;

        let game = IgdbGame {
            name: game_info.name,
            store_id: Some(store_id),
            summary: game_info.summary,
            storyline: game_info.storyline,
            genres: game_info.genres,
            cover: game_info.cover,
            artworks: game_info.artworks,
            id: game_info.id,
        };

        Ok(game)
    }

    pub async fn get_games(&mut self, steam_games_ids: Vec<u64>) -> Result<Vec<IgdbGame>, String> {
        let steam_games = self
            .get_steam_games(steam_games_ids)
            .await
            .map_err(|e| e.to_string())?;

        let mut steam_ids_map = HashMap::new();

        for game in &steam_games {
            steam_ids_map.insert(game.game, game.uid.clone());
        }

        let games_infos = self
            .get_games_infos(steam_games.iter().map(|game| game.game).collect())
            .await?;

        let parsed: Result<Vec<_>, String> = games_infos
            .into_iter()
            .map(|game| {
                let store_id = steam_ids_map.get(&game.id).cloned();

                Ok(IgdbGame {
                    name: game.name,
                    store_id,
                    summary: game.summary,
                    storyline: game.storyline,
                    genres: game.genres,
                    cover: game.cover,
                    artworks: game.artworks,
                    id: game.id,
                })
            })
            .collect();

        parsed
    }

    async fn get_steam_games(
        &mut self,
        game_ids: Vec<u64>,
    ) -> Result<Vec<IgdbAlternativeGame>, String> {
        const URL: &str = "https://api.igdb.com/v4/external_games";
        let steam_urls: Vec<_> = game_ids
            .iter()
            .map(|id| format!(r#""https://store.steampowered.com/app/{}""#, &id))
            .collect();

        let query = format!(
            "fields *;  where external_game_source = 1 & url = ({}); limit {};",
            steam_urls.join(","),
            game_ids.len()
        );

        let res = self
            .request_with_retry(URL, &query)
            .await
            .map_err(|e| e.to_string())?;

        let body = res.text().await.map_err(|e| e.to_string())?;

        let parsed =
            serde_json::from_str::<Vec<IgdbAlternativeGame>>(&body).map_err(|e| e.to_string())?;

        Ok(parsed)
    }

    async fn get_steam_game(&mut self, game_id: u64) -> Result<IgdbAlternativeGame, String> {
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

    async fn get_game_info(&mut self, igdb_game_id: u64) -> Result<IgdbGameInfo, String> {
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

    async fn get_games_infos(
        &mut self,
        igdb_game_ids: Vec<u64>,
    ) -> Result<Vec<IgdbGameInfo>, String> {
        const URL: &str = "https://api.igdb.com/v4/games";
        let ids: Vec<_> = igdb_game_ids.iter().map(|id| id.to_string()).collect();
        let query = format!(
            r#"fields *, genres.name, artworks.image_id, cover.image_id; where id = ({}); limit {};"#,
            ids.join(","),
            igdb_game_ids.len()
        );

        let res = self
            .request_with_retry(URL, &query)
            .await
            .map_err(|e| e.to_string())?;

        let body = res.text().await.map_err(|e| e.to_string())?;

        let parsed = serde_json::from_str::<Vec<IgdbGameInfo>>(&body).map_err(|e| e.to_string())?;

        Ok(parsed)
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
