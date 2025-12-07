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
