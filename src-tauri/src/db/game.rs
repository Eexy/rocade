//! Database access layer for game records.
//!
//! Provides [`Game`], the serializable game entity returned to the frontend,
//! and [`GameRepository`], which handles all SQL queries and inserts against
//! the SQLite database.

use serde::Serialize;
use sqlx::{sqlite::SqliteRow, Pool, Row, Sqlite};

use crate::igdb::IgdbGame;

/// A fully resolved game record, ready to be serialized and sent to the
/// frontend.
///
/// Aggregated columns such as `genres`, `developers`, and `artworks` are
/// collected from their respective join tables. `is_installed` is not stored
/// in the database — it is set at query time by checking the local Steam
/// library.
#[derive(Serialize)]
pub struct Game {
    /// Internal database ID.
    pub id: i64,
    pub name: String,
    pub summary: Option<String>,
    /// Steam App ID, sourced from the `games_store` table.
    pub store_id: Option<String>,
    /// IGDB `image_id` of the game's cover art.
    pub cover: Option<String>,
    /// Whether the game is fully installed in the local Steam library.
    /// Always `None` when retrieved from the database — callers must set it.
    pub is_installed: Option<bool>,
    /// List of IGDB `image_id` values for the game's artwork images.
    pub artworks: Option<Vec<String>>,
    /// Unix timestamp of the game's first release.
    pub release_date: Option<i64>,
    pub genres: Option<Vec<String>>,
    pub developers: Option<Vec<String>>,
}

/// Data-access object for game-related database operations.
pub struct GameRepository {
    pool: Pool<Sqlite>,
}

impl GameRepository {
    /// Base SELECT that joins all related tables (genres, companies, artworks,
    /// covers, store IDs) into a single row per game using `json_group_array`.
    const BASE_QUERY: &'static str = "
select
    games.id as id,
    games.name as name,
    games_store.store_id as store_id,
    summary, release_date,
    json_group_array(distinct genres.name) as genres,
    json_group_array(distinct companies.name) as studios,
    json_group_array(distinct artworks.artwork_id) as artworks,
    json_group_array(distinct covers.cover_id) as covers
from games
left join developed_by on games.id = developed_by.game_id
left join companies on developed_by.studio_id = companies.id
left join belongs_to on games.id = belongs_to.game_id
left join genres on belongs_to.genre_id = genres.id
left join artworks on artworks.game_id = games.id
left join covers on covers.game_id = games.id
left join games_store on games_store.game_id = games.id
";

    /// GROUP BY / ORDER BY clause appended to every query built by
    /// [`Self::build_query_string`]. Results are ordered alphabetically by
    /// game name.
    const GROUP_ORDER: &'static str = "
group by games.id, games.name, games_store.store_id, games.summary, games.release_date
order by games.name
";

    /// Creates a new `GameRepository` backed by the given connection pool.
    pub fn new(pool: Pool<Sqlite>) -> Self {
        Self { pool }
    }

    /// Returns all games in the database ordered alphabetically by name.
    pub async fn get_games(&self) -> Result<Vec<Game>, sqlx::Error> {
        let query = Self::build_query_string(None);
        let games = sqlx::query(&query)
            .map(Self::map_game_row)
            .fetch_all(&self.pool)
            .await?;

        Ok(games)
    }

    /// Returns a single game by its database ID.
    ///
    /// # Errors
    ///
    /// Returns [`sqlx::Error::RowNotFound`] if no game with the given ID
    /// exists.
    pub async fn get_game_by_id(&self, game_id: i64) -> Result<Game, sqlx::Error> {
        let query = Self::build_query_string(Some(game_id));
        let game = sqlx::query(&query)
            .bind(game_id)
            .map(Self::map_game_row)
            .fetch_one(&self.pool)
            .await?;

        Ok(game)
    }

    /// Builds the full SQL query string, optionally appending a `WHERE`
    /// clause to filter by a specific game ID.
    fn build_query_string(game_id: Option<i64>) -> String {
        let where_clause = if game_id.is_some() {
            " where games.id =  ?"
        } else {
            ""
        };

        format!("{}{}{}", Self::BASE_QUERY, where_clause, Self::GROUP_ORDER)
    }

    /// Maps a raw SQLite row returned by [`BASE_QUERY`](Self::BASE_QUERY) into
    /// a [`Game`].
    ///
    /// The `genres`, `studios`, `artworks`, and `covers` columns are stored as
    /// JSON arrays and decoded via [`Self::parse_json_array`]. The cover is
    /// taken as the last element of the covers array. `is_installed` is always
    /// initialized to `None` and must be set by the caller.
    fn map_game_row(row: SqliteRow) -> Game {
        let genres_json: Option<String> = row.get("genres");
        let studios_json: Option<String> = row.get("studios");
        let artworks_json: Option<String> = row.get("artworks");
        let covers_json: Option<String> = row.get("covers");

        Game {
            id: row.get("id"),
            release_date: row.get("release_date"),
            name: row.get("name"),
            developers: Self::parse_json_array(studios_json),
            genres: Self::parse_json_array(genres_json),
            is_installed: None,
            summary: row.get("summary"),
            artworks: Self::parse_json_array(artworks_json),
            cover: Self::parse_json_array(covers_json).and_then(|mut v: Vec<String>| v.pop()),
            store_id: row.get("store_id"),
        }
    }

    /// Deserializes a JSON array string produced by `json_group_array` into a
    /// `Vec<String>`, filtering out any SQL `NULL` entries that appear as
    /// `null` in the JSON.
    ///
    /// Returns `None` if the input is `None` or the JSON cannot be parsed.
    fn parse_json_array(json: Option<String>) -> Option<Vec<String>> {
        json.and_then(|s| {
            serde_json::from_str::<Vec<Option<String>>>(&s)
                .ok()
                .map(|v| v.into_iter().flatten().collect())
        })
    }

    /// Returns the Steam store ID for the given game.
    ///
    /// # Errors
    ///
    /// Returns [`sqlx::Error::RowNotFound`] if the game has no associated
    /// store entry.
    pub async fn get_game_store_id(&self, game_id: i64) -> Result<String, sqlx::Error> {
        let store_id: String =
            sqlx::query_scalar("select store_id from games_store where game_id = $1")
                .bind(game_id)
                .fetch_one(&self.pool)
                .await?;

        Ok(store_id)
    }

    /// Inserts a game and all its related data in a single transaction.
    ///
    /// The following records are created:
    /// - The core game row (`games` table).
    /// - Its Steam store ID (`games_store`).
    /// - Its cover image, if present (`covers`).
    /// - Each artwork image (`artworks`).
    /// - Each genre, upserted by name to avoid duplicates (`genres`), with a
    ///   `belongs_to` link.
    /// - Each developer company, upserted by IGDB ID (`companies`), with a
    ///   `developed_by` link.
    ///
    /// Returns the newly created game's database ID.
    pub async fn insert_complete_game(&self, game: IgdbGame) -> Result<i64, sqlx::Error> {
        let mut tx = self.pool.begin().await?;
        let id = sqlx::query_scalar::<_, i64>(
            r#"insert into games (name, summary, release_date) values ( ?, ?, ?) returning id"#,
        )
        .bind(&game.name)
        .bind(&game.summary)
        .bind(&game.release_date)
        .fetch_one(&mut *tx)
        .await?;

        // Insert store
        sqlx::query("INSERT INTO games_store (game_id, store_id) VALUES (?, ?)")
            .bind(id)
            .bind(&game.store_id)
            .execute(&mut *tx)
            .await?;

        if let Some(cover_id) = game.cover.as_ref().map(|cover| &cover.image_id) {
            sqlx::query("INSERT INTO covers (game_id, cover_id) VALUES (?, ?)")
                .bind(id)
                .bind(cover_id)
                .execute(&mut *tx)
                .await?;
        }

        // Insert artworks
        if let Some(artworks) = game.artworks {
            for artwork in artworks {
                sqlx::query("INSERT INTO artworks (game_id, artwork_id) VALUES (?, ?)")
                    .bind(id)
                    .bind(&artwork.image_id)
                    .execute(&mut *tx)
                    .await?;
            }
        }

        // Insert genres
        for genre in game.genres.iter().flatten() {
            // Insert genre if it doesn't exist (ON CONFLICT DO UPDATE NAME)
            let genre_id = sqlx::query_scalar::<_, i64>("INSERT INTO genres (name) VALUES (?) ON CONFLICT(name) DO update set name = name returning id")
                .bind(&genre.name)
                .fetch_one(&mut *tx)
                .await?;

            sqlx::query("INSERT INTO belongs_to (game_id, genre_id) VALUES (?, ?)")
                .bind(id)
                .bind(genre_id)
                .execute(&mut *tx)
                .await?;
        }

        // Insert developers
        for developer in game.developers.iter().flatten() {
            let company_id = sqlx::query_scalar::<_, i64>(
                "INSERT INTO companies (igdb_id, name) VALUES (?, ?) 
             ON CONFLICT(igdb_id) DO UPDATE SET igdb_id = igdb_id 
             RETURNING id",
            )
            .bind(developer.id)
            .bind(&developer.name)
            .fetch_one(&mut *tx)
            .await?;

            sqlx::query("INSERT INTO developed_by (game_id, studio_id) VALUES (?, ?)")
                .bind(id)
                .bind(company_id)
                .execute(&mut *tx)
                .await?;
        }

        tx.commit().await?;

        Ok(id)
    }
}
