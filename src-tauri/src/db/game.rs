use sqlx::{Pool, Sqlite};

#[derive(sqlx::FromRow, Debug)]
pub struct GameRow {
    pub id: i64,
    pub name: String,
    pub summary: Option<String>,
    pub release_date: Option<i64>,
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
