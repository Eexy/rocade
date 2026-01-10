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

        let games = sqlx::query_as!(GameRow, "select * from games order by name")
            .fetch_all(&mut *conn)
            .await?;

        Ok(games)
    }

    pub async fn insert_game(
        pool: &Pool<Sqlite>,
        name: String,
        summary: Option<String>,
    ) -> Result<i64, sqlx::Error> {
        let mut conn = pool.acquire().await?;

        let id = sqlx::query!(
            r#"insert into games (name, summary) values ( ?1, ?2)"#,
            name,
            summary
        )
        .execute(&mut *conn)
        .await?
        .last_insert_rowid();

        Ok(id)
    }

    pub async fn get_game_by_id(pool: &Pool<Sqlite>, game_id: i64) -> Result<GameRow, sqlx::Error> {
        let mut conn = pool.acquire().await?;

        let game = sqlx::query_as!(GameRow, r#"select * from games where id = ?"#, game_id)
            .fetch_one(&mut *conn)
            .await?;

        Ok(game)
    }
}
