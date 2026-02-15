use serde::Serialize;
use sqlx::{sqlite::SqliteRow, Pool, Row, Sqlite};

use crate::igdb::IgdbGame;

#[derive(Serialize)]
pub struct Game {
    pub id: i64,
    pub name: String,
    pub summary: Option<String>,
    pub store_id: Option<String>,
    pub cover: Option<String>,
    pub is_installed: Option<bool>,
    pub artworks: Option<Vec<String>>,
    pub release_date: Option<i64>,
    pub genres: Option<Vec<String>>,
    pub developers: Option<Vec<String>>,
}

pub struct GameRepository {
    pool: Pool<Sqlite>,
}

impl GameRepository {
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

    const GROUP_ORDER: &'static str = "
group by games.id, games.name, games_store.store_id, games.summary, games.release_date
order by games.name
";

    pub fn new(pool: Pool<Sqlite>) -> Self {
        Self { pool }
    }

    pub async fn get_games(&self) -> Result<Vec<Game>, sqlx::Error> {
        let query = Self::build_query_string(None);
        let games = sqlx::query(&query)
            .map(Self::map_game_row)
            .fetch_all(&self.pool)
            .await?;

        Ok(games)
    }

    pub async fn get_game_by_id(&self, game_id: i64) -> Result<Game, sqlx::Error> {
        let query = Self::build_query_string(Some(game_id));
        let game = sqlx::query(&query)
            .bind(game_id)
            .map(Self::map_game_row)
            .fetch_one(&self.pool)
            .await?;

        Ok(game)
    }

    fn build_query_string(game_id: Option<i64>) -> String {
        let where_clause = if game_id.is_some() {
            " where games.id =  ?"
        } else {
            ""
        };

        format!("{}{}{}", Self::BASE_QUERY, where_clause, Self::GROUP_ORDER)
    }

    fn map_game_row(row: SqliteRow) -> Game {
        let genres_json: Option<String> = row.get("genres");
        let studios_json: Option<String> = row.get("studios");
        let artworks_json: Option<String> = row.get("artworks");
        let covers_json: Option<String> = row.get("covers");

        Game {
            id: row.get("id"),
            release_date: row.get("release_date"),
            name: row.get("name"),
            developers: studios_json
                .as_ref()
                .and_then(|s| serde_json::from_str::<Vec<String>>(s).ok()),
            genres: genres_json
                .as_ref()
                .and_then(|s| serde_json::from_str::<Vec<String>>(s).ok()),
            is_installed: None,
            summary: row.get("summary"),
            artworks: artworks_json
                .as_ref()
                .and_then(|s| serde_json::from_str::<Vec<String>>(s).ok()),
            cover: covers_json
                .as_ref()
                .and_then(|s| serde_json::from_str::<Vec<String>>(s).ok())
                .and_then(|mut v| v.pop()),
            store_id: row.get("store_id"),
        }
    }

    pub async fn get_game_store_id(&self, game_id: i64) -> Result<String, sqlx::Error> {
        let store_id: String =
            sqlx::query_scalar("select store_id from games_store where game_id = $1")
                .bind(game_id)
                .fetch_one(&self.pool)
                .await?;

        Ok(store_id)
    }

    /// Insert a game with all its informations : covers, genres...
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
