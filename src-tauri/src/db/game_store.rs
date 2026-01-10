use sqlx::{pool, Pool, Sqlite};

#[derive(sqlx::FromRow, Debug)]
pub struct GameStoreRow {
    pub id: i64,
    pub game_id: i64,
    pub store_id: String,
}

pub struct GameStoreRepository {}

impl GameStoreRepository {
    pub async fn delete_games_store(pool: &Pool<Sqlite>) -> Result<(), sqlx::Error> {
        let mut conn = pool.acquire().await?;

        sqlx::query!("delete from games_store")
            .execute(&mut *conn)
            .await?;

        Ok(())
    }

    pub async fn get_games_store(pool: &Pool<Sqlite>) -> Result<Vec<GameStoreRow>, sqlx::Error> {
        let mut conn = pool.acquire().await?;

        let games = sqlx::query_as!(GameStoreRow, "select * from games_store")
            .fetch_all(&mut *conn)
            .await?;

        Ok(games)
    }

    pub async fn insert_game_store(
        pool: &Pool<Sqlite>,
        game_id: i64,
        store_id: String,
    ) -> Result<i64, sqlx::Error> {
        let mut conn = pool.acquire().await?;

        let id = sqlx::query!(
            r#"insert into games_store (game_id, store_id) values ( ?1, ?2)"#,
            game_id,
            store_id
        )
        .execute(&mut *conn)
        .await?
        .last_insert_rowid();

        Ok(id)
    }

    pub async fn get_game_store(
        pool: &Pool<Sqlite>,
        game_id: i64,
    ) -> Result<GameStoreRow, sqlx::Error> {
        let mut conn = pool.acquire().await?;

        let game_store = sqlx::query_as!(
            GameStoreRow,
            r#"select * from games_store where game_id = ?"#,
            game_id
        )
        .fetch_one(&mut *conn)
        .await?;

        Ok(game_store)
    }
}
