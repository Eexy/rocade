pub struct RocadeConfig {
    pub steam_api_key: String,
    pub steam_profile_id: String,
    pub twitch_client_id: String,
    pub twitch_client_secret: String,
}

impl RocadeConfig {
    pub fn new() -> Self {
        RocadeConfig {
            steam_api_key: String::from(""),
            steam_profile_id: String::from(""),
            twitch_client_id: String::from(""),
            twitch_client_secret: String::from(""),
        }
    }
}
