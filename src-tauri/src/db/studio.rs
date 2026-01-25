use sqlx::{FromRow, QueryBuilder};
use sqlx::{Pool, Sqlite};

use crate::igdb::IgdbCompany;

#[derive(Debug, FromRow)]
pub struct StudioRow {
    id: i64,
    igdb_id: i64,
    pub name: Option<String>,
}

#[derive(Debug, FromRow)]
pub struct GameStudioRow {
    id: i64,
    game_id: i64,
    studio_id: i64,
}

pub struct StudioRepository {}

impl StudioRepository {
    pub async fn delete_studios(pool: &Pool<Sqlite>) -> Result<(), sqlx::Error> {
        let mut conn = pool.acquire().await?;

        sqlx::query!("delete from games_studios")
            .execute(&mut *conn)
            .await?;

        sqlx::query!("delete from studios")
            .execute(&mut *conn)
            .await?;

        Ok(())
    }

    pub async fn insert_game_studios(
        pool: &Pool<Sqlite>,
        game_id: i64,
        studios: Vec<IgdbCompany>,
    ) -> Result<(), sqlx::Error> {
        let mut conn = pool.acquire().await?;

        let mut studios_query_builder: QueryBuilder<Sqlite> =
            QueryBuilder::new("insert into studios (igdb_id, name) ");
        studios_query_builder.push_values(&studios, |mut query_builder, studio| {
            query_builder.push_bind(studio.id).push_bind(&studio.name);
        });

        studios_query_builder.push("on conflict(igdb_id) do nothing".to_string());

        let studios_insertion_query = studios_query_builder.build();
        studios_insertion_query.execute(&mut *conn).await?;

        let inserted_studios =
            StudioRepository::get_studios(pool, studios.iter().map(|studio| studio.id).collect())
                .await?;

        let mut game_studios_query_builder: QueryBuilder<Sqlite> =
            QueryBuilder::new("insert into games_studios(game_id, studio_id) ");

        game_studios_query_builder.push_values(&inserted_studios, |mut query_builder, studio| {
            query_builder.push_bind(game_id).push_bind(studio.id);
        });

        let game_studios_insertion_query = game_studios_query_builder.build();
        game_studios_insertion_query.execute(&mut *conn).await?;

        Ok(())
    }

    pub async fn get_studios(
        pool: &Pool<Sqlite>,
        studio_ids: Vec<i64>,
    ) -> Result<Vec<StudioRow>, sqlx::Error> {
        let mut conn = pool.acquire().await?;

        if studio_ids.is_empty() {
            let result = sqlx::query_as!(StudioRow, r#"select * from studios"#)
                .fetch_all(&mut *conn)
                .await?;

            return Ok(result);
        }

        let placeholders = vec!["?"; studio_ids.len()].join(", ");
        let query_str = format!("select * from studios where igdb_id in ({})", placeholders);

        let mut query = sqlx::query_as::<_, StudioRow>(&query_str);

        for genre in &studio_ids {
            query = query.bind(genre);
        }

        let result = query.fetch_all(&mut *conn).await?;

        Ok(result)
    }

    pub async fn get_game_studios(
        pool: &Pool<Sqlite>,
        game_id: i64,
    ) -> Result<Vec<StudioRow>, sqlx::Error> {
        let mut conn = pool.acquire().await?;

        let game_studios = sqlx::query_as!(
            GameStudioRow,
            "select * from games_studios where game_id = ?",
            game_id
        )
        .fetch_all(&mut *conn)
        .await?;

        let studio_ids: Vec<_> = game_studios.iter().map(|studio| studio.studio_id).collect();

        if studio_ids.is_empty() {
            return Ok(vec![]);
        }

        let placeholders = vec!["?"; studio_ids.len()].join(", ");
        let query_str = format!("select * from studios where id in ({})", placeholders);

        let mut query = sqlx::query_as::<_, StudioRow>(&query_str);

        for studio_id in &studio_ids {
            query = query.bind(studio_id);
        }

        let result = query.fetch_all(&mut *conn).await?;

        Ok(result)
    }
}
