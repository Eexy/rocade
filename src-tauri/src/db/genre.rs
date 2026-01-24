use sqlx::{FromRow, QueryBuilder};
use sqlx::{Pool, Sqlite};

#[derive(Debug, FromRow)]
pub struct GenreRow {
    id: i64,
    name: String,
}

#[derive(Debug, FromRow)]
pub struct GameGenreRow {
    id: i64,
    game_id: i64,
    genre_id: i64,
}

pub struct GenreRepository {}

impl GenreRepository {
    pub async fn delete_genres(pool: &Pool<Sqlite>) -> Result<(), sqlx::Error> {
        let mut conn = pool.acquire().await?;

        sqlx::query!("delete from games_genres")
            .execute(&mut *conn)
            .await?;

        sqlx::query!("delete from genres")
            .execute(&mut *conn)
            .await?;

        Ok(())
    }

    pub async fn insert_game_genres(
        pool: &Pool<Sqlite>,
        game_id: i64,
        genres: Vec<String>,
    ) -> Result<(), sqlx::Error> {
        let mut conn = pool.acquire().await?;

        let mut genres_query_builder: QueryBuilder<Sqlite> =
            QueryBuilder::new("insert into genres (name) ");
        genres_query_builder.push_values(&genres, |mut query_builder, genre_name| {
            query_builder.push_bind(genre_name);
        });

        genres_query_builder.push("on conflict(name) do nothing".to_string());

        let genre_insertion_query = genres_query_builder.build();
        genre_insertion_query.execute(&mut *conn).await?;

        let inserted_genres = GenreRepository::get_genres(pool, genres).await?;

        let mut game_genres_query_builder: QueryBuilder<Sqlite> =
            QueryBuilder::new("insert into games_genres(game_id, genre_id) ");

        game_genres_query_builder.push_values(&inserted_genres, |mut query_builder, genre| {
            query_builder.push_bind(game_id).push_bind(genre.id);
        });

        let game_genre_insertion_query = game_genres_query_builder.build();
        game_genre_insertion_query.execute(&mut *conn).await?;

        Ok(())
    }

    pub async fn get_genres(
        pool: &Pool<Sqlite>,
        genres: Vec<String>,
    ) -> Result<Vec<GenreRow>, sqlx::Error> {
        let mut conn = pool.acquire().await?;

        if genres.is_empty() {
            let result = sqlx::query_as!(GenreRow, r#"select * from genres"#)
                .fetch_all(&mut *conn)
                .await?;

            return Ok(result);
        }

        let placeholders = vec!["?"; genres.len()].join(", ");
        let query_str = format!("select * from genres where name in ({})", placeholders);

        let mut query = sqlx::query_as::<_, GenreRow>(&query_str);

        for genre in &genres {
            query = query.bind(genre);
        }

        let result = query.fetch_all(&mut *conn).await?;

        Ok(result)
    }

    pub async fn get_game_genre(
        pool: &Pool<Sqlite>,
        game_id: i64,
    ) -> Result<Vec<String>, sqlx::Error> {
        let mut conn = pool.acquire().await?;

        let game_genres = sqlx::query_as!(
            GameGenreRow,
            "select * from games_genres where game_id = ?",
            game_id
        )
        .fetch_all(&mut *conn)
        .await?;

        let genre_ids: Vec<_> = game_genres.iter().map(|genre| genre.genre_id).collect();

        if genre_ids.is_empty() {
            return Ok(vec![]);
        }

        let placeholders = vec!["?"; genre_ids.len()].join(", ");
        let query_str = format!("select * from genres where id in ({})", placeholders);

        let mut query = sqlx::query_as::<_, GenreRow>(&query_str);

        for genre_id in &genre_ids {
            query = query.bind(genre_id);
        }

        let result = query.fetch_all(&mut *conn).await?;

        Ok(result.into_iter().map(|genre| genre.name).collect())
    }
}
