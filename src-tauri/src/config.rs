pub struct RocadeConfig {
    pub steam_api_key: String,
    pub steam_profile_id: String,
    pub twitch_client_id: String,
    pub twitch_client_secret: String,
}

#[derive(Debug, thiserror::Error)]
pub enum RocadeConfigError {
    #[error("environment config error: {0}")]
    EnvError(String),

    #[error("database error: {0}")]
    DatabaseError(#[from] sqlx::Error),

    #[error("config error: {0}")]
    ConfigError(String),
}
