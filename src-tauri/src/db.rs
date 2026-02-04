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

        let connexion = SqliteConnectOptions::new()
            .filename(&db_path)
            .create_if_missing(true)
            .journal_mode(sqlx::sqlite::SqliteJournalMode::Wal);

        let pool = SqlitePool::connect_with(connexion).await?;

        Ok(Self { pool: pool })
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
            from games_genres;

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

pub mod artwork;
pub mod cover;
pub mod game;
pub mod game_store;
pub mod genre;
pub mod studio;
