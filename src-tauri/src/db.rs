use std::{fs, path::PathBuf};

use sqlx::{sqlite::SqliteConnectOptions, Pool, Sqlite, SqlitePool};

use crate::config::RocadeConfigError;

pub struct DatabaseState {
    pub pool: Pool<Sqlite>,
}

impl DatabaseState {
    pub async fn new(app_dir: PathBuf) -> Result<DatabaseState, RocadeConfigError> {
        fs::create_dir_all(&app_dir);

        let db_path = app_dir.join("rocade.db");

        let connection = SqliteConnectOptions::new()
            .filename(&db_path)
            .create_if_missing(true)
            .journal_mode(sqlx::sqlite::SqliteJournalMode::Wal);

        let pool = SqlitePool::connect_with(connection).await?;

        Ok(Self { pool })
    }

    /// Empty all database
    pub async fn clean(&self) -> Result<(), sqlx::Error> {
        sqlx::query!(
            "
            delete
            from artworks;

            delete
            from covers;

            delete
            from belongs_to;

            delete
            from games_store;

            delete
            from developed_by;

            delete
            from games;

            delete
            from genres;

            delete
            from companies;
            "
        )
        .execute(&self.pool)
        .await?;

        Ok(())
    }
}

pub mod game;
