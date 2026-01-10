use sqlx::{Pool, QueryBuilder, Sqlite};

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

    pub async fn insert_artwork(
        pool: &Pool<Sqlite>,
        game_id: i64,
        artwork_id: String,
    ) -> Result<i64, sqlx::Error> {
        let mut conn = pool.acquire().await?;

        let id = sqlx::query!(
            r#"insert into artworks (game_id, artwork_id) values ( ?1, ?2)"#,
            game_id,
            artwork_id
        )
        .execute(&mut *conn)
        .await?
        .last_insert_rowid();

        Ok(id)
    }

    pub async fn bulk_insert_artworks(
        pool: &Pool<Sqlite>,
        game_id: i64,
        artworks_ids: Vec<String>,
    ) -> Result<(), sqlx::Error> {
        let mut conn = pool.acquire().await?;

        let mut artwork_query_builder: QueryBuilder<Sqlite> =
            QueryBuilder::new("insert into artworks (game_id, artwork_id) ");
        artwork_query_builder.push_values(artworks_ids, |mut query_builder, artwork_id| {
            query_builder.push_bind(game_id).push_bind(artwork_id);
        });

        let query = artwork_query_builder.build();
        query.execute(&mut *conn).await?;

        Ok(())
    }

    pub async fn get_game_artworks(
        pool: &Pool<Sqlite>,
        game_id: i64,
    ) -> Result<Vec<ArtworkRow>, sqlx::Error> {
        let mut conn = pool.acquire().await?;

        let artworks = sqlx::query_as!(
            ArtworkRow,
            r#"select * from artworks where game_id = ?"#,
            game_id
        )
        .fetch_all(&mut *conn)
        .await?;

        Ok(artworks)
    }
}
