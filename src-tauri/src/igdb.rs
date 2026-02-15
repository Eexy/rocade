use std::collections::HashMap;

use crate::twitch::{TwitchApiClient, TwitchError};
use serde::{Deserialize, Serialize};
use tauri::http::{HeaderMap, HeaderValue, StatusCode};
use tauri_plugin_http::reqwest::{self, Client, Response};

#[derive(Serialize, Deserialize, Debug)]
pub struct IgdbGenre {
    pub name: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct IgdbImage {
    pub image_id: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct IgdbCompany {
    pub id: i64,
    pub name: String,
    published: Option<Vec<u64>>,
    developed: Option<Vec<u64>>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct IgdbInvolvedCompany {
    company: IgdbCompany,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct IgdbGameInfo {
    id: u64,
    name: String,
    cover: Option<IgdbImage>,
    genres: Option<Vec<IgdbGenre>>,
    storyline: Option<String>,
    involved_companies: Option<Vec<IgdbInvolvedCompany>>,
    summary: Option<String>,
    artworks: Option<Vec<IgdbImage>>,
    first_release_date: Option<i64>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct IgdbGame {
    id: u64,
    pub name: String,
    pub store_id: Option<String>,
    storyline: Option<String>,
    pub summary: Option<String>,
    pub genres: Option<Vec<IgdbGenre>>,
    pub cover: Option<IgdbImage>,
    pub artworks: Option<Vec<IgdbImage>>,
    pub publishers: Option<Vec<IgdbCompany>>,
    pub developers: Option<Vec<IgdbCompany>>,
    pub release_date: Option<i64>,
}

#[derive(Debug, thiserror::Error)]
pub enum IgdbError {
    #[error("http request failed: {0}")]
    Request(#[from] reqwest::Error),

    #[error("twitch failed: {0}")]
    Twitch(#[from] TwitchError),

    #[error("unable to parse igdb data: {0}")]
    InvalidData(#[from] serde_json::Error),

    #[error("unable to find game: {0}")]
    NoData(String),
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

        IgdbApiClient {
            twitch_client,
            client: tauri_plugin_http::reqwest::Client::builder()
                .default_headers(headers)
                .build()
                .expect("unable to build igdb client"),
        }
    }

    pub async fn get_game(&mut self, steam_game_id: u64) -> Result<IgdbGame, IgdbError> {
        let steam_game = self.get_steam_game(steam_game_id).await?;

        let store_id = steam_game_id.to_string();

        let game_info = self.get_game_info(steam_game.game).await?;

        let (publishers, developers) =
            self.extract_game_companies(game_info.involved_companies, game_info.id);

        let game = IgdbGame {
            name: game_info.name,
            store_id: Some(store_id),
            summary: game_info.summary,
            storyline: game_info.storyline,
            genres: game_info.genres,
            cover: game_info.cover,
            publishers,
            developers,
            artworks: game_info.artworks,
            id: game_info.id,
            release_date: game_info.first_release_date,
        };

        Ok(game)
    }

    fn extract_game_companies(
        &self,
        companies: Option<Vec<IgdbInvolvedCompany>>,
        game_id: u64,
    ) -> (Option<Vec<IgdbCompany>>, Option<Vec<IgdbCompany>>) {
        let mut publishers: Vec<IgdbCompany> = Vec::new();
        let mut developers: Vec<IgdbCompany> = Vec::new();

        if companies.is_none() {
            return (None, None);
        }

        for involved in companies.iter().flatten() {
            if let Some(published) = &involved.company.published {
                if published.contains(&game_id) {
                    publishers.push(involved.company.clone());
                }
            }

            if let Some(developed) = &involved.company.developed {
                if developed.contains(&game_id) {
                    developers.push(involved.company.clone());
                }
            }
        }

        (Some(publishers), Some(developers))
    }

    pub async fn get_games(
        &mut self,
        steam_games_ids: Vec<u64>,
    ) -> Result<Vec<IgdbGame>, IgdbError> {
        let steam_games = self.get_steam_games(steam_games_ids).await?;

        let mut steam_ids_map = HashMap::new();

        for game in &steam_games {
            steam_ids_map.insert(game.game, game.uid.clone());
        }

        let games_infos = self
            .get_games_infos(steam_games.iter().map(|game| game.game).collect())
            .await?;

        let parsed: Vec<_> = games_infos
            .into_iter()
            .map(|game| {
                let store_id = steam_ids_map.get(&game.id).cloned();

                let (publishers, developers) =
                    self.extract_game_companies(game.involved_companies, game.id);

                IgdbGame {
                    name: game.name,
                    store_id,
                    summary: game.summary,
                    storyline: game.storyline,
                    genres: game.genres,
                    cover: game.cover,
                    developers,
                    publishers,
                    artworks: game.artworks,
                    id: game.id,
                    release_date: game.first_release_date,
                }
            })
            .collect();

        Ok(parsed)
    }

    async fn get_steam_games(
        &mut self,
        game_ids: Vec<u64>,
    ) -> Result<Vec<IgdbAlternativeGame>, IgdbError> {
        const URL: &str = "https://api.igdb.com/v4/external_games";
        let steam_urls: Vec<_> = game_ids.iter().map(|id| format!(r#""{}""#, &id)).collect();

        let query = format!(
            "fields *;  where external_game_source = 1 & uid = ({}); limit {};",
            steam_urls.join(","),
            game_ids.len()
        );

        let res = self.request_with_retry(URL, &query).await?;

        let body = res.text().await?;

        let parsed = serde_json::from_str::<Vec<IgdbAlternativeGame>>(&body)?;

        Ok(parsed)
    }

    async fn get_steam_game(&mut self, game_id: u64) -> Result<IgdbAlternativeGame, IgdbError> {
        const URL: &str = "https://api.igdb.com/v4/external_games";
        let query = format!(
            "fields *;  where external_game_source = 1 & url = \"https://store.steampowered.com/app/{}\"; limit 1;",
            game_id
        );
        let res = self.request_with_retry(URL, &query).await?;

        let body = res.text().await?;

        let mut parsed = serde_json::from_str::<Vec<IgdbAlternativeGame>>(&body)?;

        parsed
            .pop()
            .ok_or(IgdbError::NoData("Unable to find game".to_string()))
    }

    async fn get_game_info(&mut self, igdb_game_id: u64) -> Result<IgdbGameInfo, IgdbError> {
        const URL: &str = "https://api.igdb.com/v4/games";
        let query = format!(
            "fields *, genres.name, artworks.image_id, involved_companies.company.*, cover.image_id; where id = {}; limit 1;",
            igdb_game_id
        );
        let res = self.request_with_retry(URL, &query).await?;

        let body = res.text().await?;

        let mut parsed = serde_json::from_str::<Vec<IgdbGameInfo>>(&body)?;

        parsed
            .pop()
            .ok_or(IgdbError::NoData("Unable to find game".to_string()))
    }

    async fn get_games_infos(
        &mut self,
        igdb_game_ids: Vec<u64>,
    ) -> Result<Vec<IgdbGameInfo>, IgdbError> {
        const URL: &str = "https://api.igdb.com/v4/games";
        let ids: Vec<_> = igdb_game_ids.iter().map(|id| id.to_string()).collect();
        let query = format!(
            r#"fields *, genres.name, artworks.image_id, cover.image_id, involved_companies.company.*; where id = ({}); limit {};"#,
            ids.join(","),
            igdb_game_ids.len()
        );

        let res = self.request_with_retry(URL, &query).await?;

        let body = res.text().await?;

        let parsed = serde_json::from_str::<Vec<IgdbGameInfo>>(&body)?;

        Ok(parsed)
    }

    async fn get_twitch_access_token(&mut self) -> Result<String, IgdbError> {
        if let Some(token) = self.twitch_client.get_access_token() {
            return Ok(token);
        }

        Ok(self.twitch_client.refresh_access_token().await?)
    }

    async fn request_with_retry(&mut self, url: &str, query: &str) -> Result<Response, IgdbError> {
        let token = self.get_twitch_access_token().await?;

        let response = self
            .client
            .post(url)
            .bearer_auth(&token)
            .body(query.to_string())
            .send()
            .await?;

        if response.status() == StatusCode::UNAUTHORIZED {
            let new_token = self.twitch_client.refresh_access_token().await?;
            Ok(self
                .client
                .post(url)
                .bearer_auth(&new_token)
                .body(query.to_string())
                .send()
                .await?)
        } else {
            Ok(response)
        }
    }
}
