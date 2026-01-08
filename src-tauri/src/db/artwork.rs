use sqlx::{Pool, Sqlite};

#[derive(sqlx::FromRow, Debug)]
pub struct ArtworkRow {
    pub id: i64,
    pub game_id: i64,
    pub artwork_id: String,
}

pub struct ArtworkRepository {}

impl ArtworkRepository {
    pub async fn delete_artworks(pool: &Pool<Sqlite>) -> Result<(), sqlx::Error> {
        let mut conn = pool.acquire().await?;

        sqlx::query!("delete from artworks")
            .execute(&mut *conn)
            .await?;

        Ok(())
    }

    pub async fn get_artworks(pool: &Pool<Sqlite>) -> Result<Vec<ArtworkRow>, sqlx::Error> {
        let mut conn = pool.acquire().await?;

        let artworks = sqlx::query_as!(ArtworkRow, "select * from artworks")
            .fetch_all(&mut *conn)
            .await?;

        Ok(artworks)
    }
}
