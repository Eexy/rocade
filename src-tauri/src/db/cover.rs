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

    pub async fn insert_cover(
        pool: &Pool<Sqlite>,
        game_id: i64,
        cover_id: String,
    ) -> Result<i64, sqlx::Error> {
        let mut conn = pool.acquire().await?;

        let id = sqlx::query!(
            r#"insert into covers (game_id, cover_id) values ( ?1, ?2)"#,
            game_id,
            cover_id
        )
        .execute(&mut *conn)
        .await?
        .last_insert_rowid();

        Ok(id)
    }
}
