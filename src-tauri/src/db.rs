use std::fs;

use sqlx::{Pool, Sqlite, SqlitePool};
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
        let db_url = format!("sqlite:{}", db_path.display());

        let pool = SqlitePool::connect(&db_url).await?;

        Ok(Self { pool: pool })
    }
}
