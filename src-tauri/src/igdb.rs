//! IGDB (Internet Game Database) API client.
//!
//! Provides types and an async client for querying game metadata from the
//! [IGDB API](https://api-docs.igdb.com/). Authentication is handled via a
//! Twitch OAuth token managed by [`TwitchApiClient`].

use std::collections::HashMap;

use crate::twitch::{TwitchApiClient, TwitchError};
use serde::{Deserialize, Serialize};
use tauri::http::{HeaderMap, HeaderValue, StatusCode};
use tauri_plugin_http::reqwest::{self, Client, Response};

/// A game genre as returned by the IGDB API.
#[derive(Serialize, Deserialize, Debug)]
pub struct IgdbGenre {
    pub name: String,
}

/// An image asset (cover art or artwork) as returned by the IGDB API.
///
/// The `image_id` can be used to build an image URL via the
/// [IGDB Images endpoint](https://api-docs.igdb.com/#images).
#[derive(Serialize, Deserialize, Debug)]
pub struct IgdbImage {
    pub image_id: String,
}

/// A game company (publisher or developer) as returned by the IGDB API.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct IgdbCompany {
    pub id: i64,
    pub name: String,
    /// IDs of games this company has published.
    published: Option<Vec<u64>>,
    /// IDs of games this company has developed.
    developed: Option<Vec<u64>>,
}

/// An IGDB `involved_company` entry, linking a game to a company with a role.
#[derive(Serialize, Deserialize, Debug)]
pub struct IgdbInvolvedCompany {
    company: IgdbCompany,
}

/// Raw game data as returned directly by the IGDB `/games` endpoint.
///
/// This is an intermediate representation. Use [`IgdbGame`] for the
/// processed, caller-facing version.
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
    /// Unix timestamp of the game's first release.
    first_release_date: Option<i64>,
}

/// Processed game metadata ready for use by the rest of the application.
///
/// Constructed from [`IgdbGameInfo`] after resolving companies and
/// mapping the Steam store ID.
#[derive(Serialize, Deserialize, Debug)]
pub struct IgdbGame {
    id: u64,
    pub name: String,
    /// Steam store ID associated with this game entry, if available.
    pub store_id: Option<String>,
    storyline: Option<String>,
    pub summary: Option<String>,
    pub genres: Option<Vec<IgdbGenre>>,
    pub cover: Option<IgdbImage>,
    pub artworks: Option<Vec<IgdbImage>>,
    pub publishers: Option<Vec<IgdbCompany>>,
    pub developers: Option<Vec<IgdbCompany>>,
    /// Unix timestamp of the game's first release.
    pub release_date: Option<i64>,
}

/// Errors that can occur while using the IGDB API client.
#[derive(Debug, thiserror::Error)]
pub enum IgdbError {
    /// An HTTP request to the IGDB API failed.
    #[error("http request failed: {0}")]
    Request(#[from] reqwest::Error),

    /// Fetching or refreshing the Twitch access token failed.
    #[error("twitch failed: {0}")]
    Twitch(#[from] TwitchError),

    /// The response body could not be deserialized into the expected type.
    #[error("unable to parse igdb data: {0}")]
    InvalidData(#[from] serde_json::Error),

    /// No matching game was found for the given identifier.
    #[error("unable to find game: {0}")]
    NoData(String),
}

/// Async client for the IGDB API.
///
/// Uses a [`TwitchApiClient`] to obtain and refresh Bearer tokens, which are
/// required by the IGDB API for authentication.
#[derive(Debug)]
pub struct IgdbApiClient {
    twitch_client: TwitchApiClient,
    client: Client,
}

/// An IGDB external-game record that maps an IGDB game ID to a Steam UID.
#[derive(Deserialize, Debug)]
pub struct IgdbAlternativeGame {
    /// IGDB internal game ID.
    #[serde(rename = "game")]
    id: u64,
    /// External store identifier (Steam App ID when `external_game_source = 1`).
    uid: String,
}

impl IgdbApiClient {
    /// Creates a new IGDB API client.
    ///
    /// Builds the underlying HTTP client with the Twitch `CLIENT-ID` header
    /// pre-configured. Bearer tokens are fetched lazily on each request via
    /// the provided `twitch_client`.
    pub fn new(twitch_client: TwitchApiClient) -> Self {
        let mut headers = HeaderMap::new();

        headers.insert(
            "CLIENT-ID",
            HeaderValue::from_str(twitch_client.get_client_id())
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

    /// Fetches IGDB metadata for a single game identified by its Steam App ID.
    ///
    /// Resolves the Steam ID to an IGDB game ID via the external-games
    /// endpoint, then retrieves the full game record including cover art,
    /// genres, artworks, and company roles.
    ///
    /// # Errors
    ///
    /// Returns [`IgdbError::NoData`] if no IGDB entry is linked to the given
    /// Steam App ID.
    pub async fn get_game(&mut self, steam_game_id: u64) -> Result<IgdbGame, IgdbError> {
        let steam_game = self.get_steam_game(steam_game_id).await?;

        let store_id = steam_game_id.to_string();

        let game_info = self.get_game_info(steam_game.id).await?;

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

    /// Splits a list of involved companies into publishers and developers for a given game.
    ///
    /// - A company is a **developer** if `game_id` appears in its `developed` list.
    /// - A company is a **publisher** if `game_id` appears in its `published` list.
    ///
    /// A company can appear in both lists. Returns `(None, None)` when
    /// `companies` is `None`.
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

    /// Fetches IGDB metadata for multiple games identified by their Steam App IDs.
    ///
    /// Resolves all Steam IDs to IGDB game IDs in a single batch request, then
    /// retrieves full game records for all of them in a second batch request.
    /// Games that have no corresponding IGDB entry are silently omitted from
    /// the result.
    pub async fn get_games(
        &mut self,
        steam_games_ids: Vec<u64>,
    ) -> Result<Vec<IgdbGame>, IgdbError> {
        let steam_games = self.get_steam_games(steam_games_ids).await?;

        let mut steam_ids_map = HashMap::new();

        for game in &steam_games {
            steam_ids_map.insert(game.id, game.uid.clone());
        }

        let mut all_games_infos = Vec::new();
        for chunck in steam_games.chunks(500) {
            let games_infos = self
                .get_games_infos(chunck.iter().map(|game| game.id).collect())
                .await?;
            all_games_infos.extend(games_infos);
        }

        let parsed: Vec<_> = all_games_infos
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

    /// Resolves a batch of Steam App IDs to IGDB external-game records.
    ///
    /// Queries the IGDB `/external_games` endpoint filtering by
    /// `external_game_source = 1` (Steam) and the provided UIDs.
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

    /// Resolves a single Steam App ID to its IGDB external-game record.
    ///
    /// Queries the IGDB `/external_games` endpoint by the Steam store URL.
    ///
    /// # Errors
    ///
    /// Returns [`IgdbError::NoData`] if no IGDB entry is linked to the given
    /// Steam App ID.
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

    /// Fetches the full game record from IGDB for a single IGDB game ID.
    ///
    /// Requests all standard fields plus nested `genres`, `artworks`,
    /// `cover`, and `involved_companies` in one query.
    ///
    /// # Errors
    ///
    /// Returns [`IgdbError::NoData`] if no game with the given ID exists.
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

    /// Fetches full game records from IGDB for a batch of IGDB game IDs.
    ///
    /// Requests all standard fields plus nested `genres`, `artworks`,
    /// `cover`, and `involved_companies` in a single query.
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

    /// Returns a valid Twitch access token, refreshing it if one is not cached or is expired.
    async fn get_twitch_access_token(&mut self) -> Result<String, IgdbError> {
        if let Some(token) = self.twitch_client.get_access_token() {
            return Ok(token.to_string());
        }

        Ok(self.twitch_client.refresh_access_token().await?.to_string())
    }

    /// Sends a POST request to an IGDB endpoint with an Apicalypse `query` body.
    ///
    /// If the first attempt returns `401 Unauthorized`, the Twitch token is
    /// refreshed and the request is retried once with the new token.
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
