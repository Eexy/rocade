use sqlx::{Pool, Sqlite};

#[derive(sqlx::FromRow, Debug)]
pub struct GameRow {
    pub id: i64,
    pub name: String,
    pub summary: Option<String>,
}

pub struct GameRepository {}

impl GameRepository {
    pub async fn delete_games(pool: &Pool<Sqlite>) -> Result<(), sqlx::Error> {
        let mut conn = pool.acquire().await?;

        sqlx::query!("delete from games")
            .execute(&mut *conn)
            .await?;

        Ok(())
    }

    pub async fn get_games(pool: &Pool<Sqlite>) -> Result<Vec<GameRow>, sqlx::Error> {
        let mut conn = pool.acquire().await?;

        let games = sqlx::query_as!(GameRow, "select * from games")
            .fetch_all(&mut *conn)
            .await?;

        Ok(games)
    }
}
