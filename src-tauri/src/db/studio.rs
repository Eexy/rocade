use sqlx::{FromRow, QueryBuilder};
use sqlx::{Pool, Sqlite};

use crate::igdb::IgdbCompany;

#[derive(Debug, FromRow)]
pub struct CompanyRow {
    id: i64,
    igdb_id: i64,
    pub name: String,
}

pub struct CompanyRepository {}

impl CompanyRepository {
    pub async fn insert_companies(
        pool: &Pool<Sqlite>,
        game_id: i64,
        studios: Vec<IgdbCompany>,
    ) -> Result<(), sqlx::Error> {
        let mut conn = pool.acquire().await?;

        let mut studios_query_builder: QueryBuilder<Sqlite> =
            QueryBuilder::new("insert into companies (igdb_id, name) ");
        studios_query_builder.push_values(&studios, |mut query_builder, studio| {
            query_builder.push_bind(studio.id).push_bind(&studio.name);
        });

        studios_query_builder.push("on conflict(igdb_id) do nothing".to_string());

        let studios_insertion_query = studios_query_builder.build();
        studios_insertion_query.execute(&mut *conn).await?;

        let inserted_studios = CompanyRepository::get_companies(
            pool,
            studios.iter().map(|studio| studio.id).collect(),
        )
        .await?;

        let mut game_studios_query_builder: QueryBuilder<Sqlite> =
            QueryBuilder::new("insert into developed_by(game_id, studio_id) ");

        game_studios_query_builder.push_values(&inserted_studios, |mut query_builder, studio| {
            query_builder.push_bind(game_id).push_bind(studio.id);
        });

        let game_studios_insertion_query = game_studios_query_builder.build();
        game_studios_insertion_query.execute(&mut *conn).await?;

        Ok(())
    }

    pub async fn get_companies(
        pool: &Pool<Sqlite>,
        studio_ids: Vec<i64>,
    ) -> Result<Vec<CompanyRow>, sqlx::Error> {
        let mut conn = pool.acquire().await?;

        if studio_ids.is_empty() {
            let result = sqlx::query_as!(CompanyRow, r#"select * from companies"#)
                .fetch_all(&mut *conn)
                .await?;

            return Ok(result);
        }

        let placeholders = vec!["?"; studio_ids.len()].join(", ");
        let query_str = format!(
            "select * from companies where igdb_id in ({})",
            placeholders
        );

        let mut query = sqlx::query_as::<_, CompanyRow>(&query_str);

        for genre in &studio_ids {
            query = query.bind(genre);
        }

        let result = query.fetch_all(&mut *conn).await?;

        Ok(result)
    }
}
