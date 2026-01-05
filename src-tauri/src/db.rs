use std::fs;

use sqlx::{sqlite::SqliteConnectOptions, Pool, Sqlite, SqlitePool};
use tauri::{AppHandle, Manager};

pub struct DatabaseState {
    pub pool: Pool<Sqlite>,
}

impl DatabaseState {
    pub async fn new(app_handle: &AppHandle) -> Result<DatabaseState, sqlx::Error> {
        let app_dir = app_handle
            .path()
            .app_data_dir()
            .expect("unable to get app directory");

        fs::create_dir_all(&app_dir).expect("unable to create app directory");

        let db_path = app_dir.join("rocade.db");
        dbg!(&db_path);
        let db_url = format!("sqlite:{}", db_path.display());
        dbg!(&db_url);

        let connexion = SqliteConnectOptions::new()
            .filename(&db_path)
            .create_if_missing(true)
            .journal_mode(sqlx::sqlite::SqliteJournalMode::Wal);

        let pool = SqlitePool::connect_with(connexion).await?;

        Ok(Self { pool: pool })
    }
}
