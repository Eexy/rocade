use sqlx::{Pool, Sqlite};

#[derive(sqlx::FromRow, Debug)]
pub struct GameRow {
    pub id: i64,
    pub name: String,
    pub summary: Option<String>,
    pub release_date: Option<i64>,
}

#[derive(Debug)]
pub struct GameInfo {
    pub id: i64,
    pub name: String,
    pub summary: Option<String>,
    pub artworks: Option<String>,
    pub covers: Option<String>,
    pub store_id: Option<String>,
    pub release_date: Option<i64>,
    pub genres: Option<String>,
    pub studios: Option<String>,
}

pub struct GameRepository {}

impl GameRepository {
    pub async fn get_games(pool: &Pool<Sqlite>) -> Result<Vec<GameRow>, sqlx::Error> {
        let mut conn = pool.acquire().await?;

        let games = sqlx::query_as!(GameRow, "select * from games order by name")
            .fetch_all(&mut *conn)
            .await?;

        Ok(games)
    }

    pub async fn get_game_by_id(
        pool: &Pool<Sqlite>,
        game_id: i64,
    ) -> Result<GameInfo, sqlx::Error> {
        let game = sqlx::query_as!(GameInfo,
        "
select games.id as id, games.name as name, games_store.store_id as store_id, summary, release_date, group_concat(distinct genres.name) as genres, group_concat(distinct companies.name) as studios, group_concat(distinct artworks.artwork_id) as artworks, group_concat(distinct covers.cover_id) as covers
from games
inner join developed_by on games.id = developed_by.game_id
inner join companies on developed_by.studio_id = companies.id
inner join games_genres on games.id = games_genres.game_id
inner join genres on games_genres.genre_id = genres.id
inner join artworks on artworks.game_id = games.id
inner join covers on covers.game_id = games.id
inner join games_store on games_store.game_id = games.id
where games.id = ?
group by games.id, games.name, games_store.store_id, games.summary, games.release_date
    ", game_id).fetch_one(pool).await?;

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
