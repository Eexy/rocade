use sqlx::{Pool, Sqlite};

#[derive(sqlx::FromRow, Debug)]
pub struct CoverRow {
    pub id: i64,
    pub game_id: i64,
    pub cover_id: String,
}

pub struct CoverRepository {}

impl CoverRepository {
    pub async fn delete_covers(pool: &Pool<Sqlite>) -> Result<(), sqlx::Error> {
        let mut conn = pool.acquire().await?;

        sqlx::query!("delete from covers")
            .execute(&mut *conn)
            .await?;

        Ok(())
    }

    pub async fn get_covers(pool: &Pool<Sqlite>) -> Result<Vec<CoverRow>, sqlx::Error> {
        let mut conn = pool.acquire().await?;

        let covers = sqlx::query_as!(CoverRow, "select * from covers")
            .fetch_all(&mut *conn)
            .await?;

        Ok(covers)
    }
}
