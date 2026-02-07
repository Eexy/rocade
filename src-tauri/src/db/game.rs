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
    pub async fn get_games(pool: &Pool<Sqlite>) -> Result<Vec<GameInfo>, sqlx::Error> {
        let games = sqlx::query_as!(
            GameInfo,
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
    "
        )
        .fetch_all(pool)
        .await?;

        Ok(games)
    }

    pub async fn get_game_by_id(
        pool: &Pool<Sqlite>,
        game_id: i64,
    ) -> Result<GameInfo, sqlx::Error> {
        let game = sqlx::query_as!(
            GameInfo,
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
where games.id = ?
group by games.id, games.name, games_store.store_id, games.summary, games.release_date
order by games.name
    ",
            game_id
        )
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
