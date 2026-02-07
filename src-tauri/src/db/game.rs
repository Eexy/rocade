use serde::Serialize;
use sqlx::{sqlite::SqliteRow, Pool, Row, Sqlite};

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

pub struct GameRepository {}

impl GameRepository {
    pub async fn get_games(pool: &Pool<Sqlite>) -> Result<Vec<Game>, sqlx::Error> {
        let games = sqlx::query(
            "
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
inner join developed_by on games.id = developed_by.game_id
inner join companies on developed_by.studio_id = companies.id
inner join games_genres on games.id = games_genres.game_id
inner join genres on games_genres.genre_id = genres.id
inner join artworks on artworks.game_id = games.id
inner join covers on covers.game_id = games.id
inner join games_store on games_store.game_id = games.id
group by games.id, games.name, games_store.store_id, games.summary, games.release_date
order by games.name
    ",
        )
        .map(|row: SqliteRow| {
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
        })
        .fetch_all(pool)
        .await?;

        Ok(games)
    }

    pub async fn get_game_by_id(pool: &Pool<Sqlite>, game_id: i64) -> Result<Game, sqlx::Error> {
        let game = sqlx::query(
            "
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
inner join developed_by on games.id = developed_by.game_id
inner join companies on developed_by.studio_id = companies.id
inner join games_genres on games.id = games_genres.game_id
inner join genres on games_genres.genre_id = genres.id
inner join artworks on artworks.game_id = games.id
inner join covers on covers.game_id = games.id
inner join games_store on games_store.game_id = games.id
where games.id = ?1
group by games.id, games.name, games_store.store_id, games.summary, games.release_date
order by games.name
    ",
        )
        .bind(game_id)
        .map(|row: SqliteRow| {
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
        })
        .fetch_one(pool)
        .await?;

        Ok(game)
    }

    pub async fn insert_game(
        pool: &Pool<Sqlite>,
        name: String,
        summary: Option<String>,
        release_date: i64,
    ) -> Result<i64, sqlx::Error> {
        let mut conn = pool.acquire().await?;

        let id = sqlx::query!(
            r#"insert into games (name, summary, release_date) values ( ?1, ?2, ?3)"#,
            name,
            summary,
            release_date
        )
        .execute(&mut *conn)
        .await?
        .last_insert_rowid();

        Ok(id)
    }
}
