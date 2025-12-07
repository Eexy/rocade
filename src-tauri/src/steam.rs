#[derive(Debug)]
pub struct SteamState {
    key: String,
}

impl SteamState {
    pub fn new(key: String) -> Self {
        return SteamState { key };
    }

    pub fn get_key(&self) -> &String {
        &self.key
    }
}

pub struct SteamApiClient {
    key: String,
}

impl SteamApiClient {
    pub fn new(key: String) -> Self {
        SteamApiClient { key }
    }
}
