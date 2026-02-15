# Rocade - Code Analysis

## Critical Issues

### 1. `INNER JOIN` silently drops games missing any relation

**File:** `src-tauri/src/db/game.rs:36-42`

The `BASE_QUERY` uses `INNER JOIN` for **all** relations (genres, developers, artworks, covers, games_store). If a game is missing even one genre, one cover, or one developer, it completely disappears from results.

```sql
inner join developed_by on games.id = developed_by.game_id
inner join companies on developed_by.studio_id = companies.id
inner join belongs_to on games.id = belongs_to.game_id
inner join genres on belongs_to.genre_id = genres.id
inner join artworks on artworks.game_id = games.id
inner join covers on covers.game_id = games.id
inner join games_store on games_store.game_id = games.id
```

**Example of failure:** IGDB returns a game with `artworks: None` (the field is `Option<Vec<IgdbImage>>`). No artworks are inserted. The game exists in the `games` table but the `INNER JOIN artworks` causes it to silently vanish from all queries. The user's Steam library shows fewer games than expected with no error.

**Hint:** Replace every `inner join` with `left join` in `BASE_QUERY` for all optional relations (`artworks`, `covers`, `developed_by`/`companies`, `belongs_to`/`genres`). Only `games_store` could stay `inner join` since every game should have a store entry. With `left join`, `json_group_array` will produce `[null]` for missing relations — update `map_game_row` to filter out null entries when parsing the JSON arrays (e.g. `.filter(|s| s != "null")`).

---

### 2. `refresh_games` wipes the entire database on every sync

**File:** `src-tauri/src/commands/game.rs:98-104`

Every time a user refreshes their library, `clean()` deletes all data from every table, then re-fetches and re-inserts everything from scratch. If the IGDB API fails mid-sync (rate limit, network error, outage), the user loses their entire library with no way to recover.

```rust
prepare_db(db_state.clone())
    .await
    .map_err(|e| RocadeError::Database(e.to_string()))?;

insert_games(game_repository, igdb_games)
    .await
    .map_err(|e| RocadeError::Database(e.to_string()))?;
```

**Example of failure:** User has 200 games. They click refresh. `clean()` deletes all 200. The IGDB API returned only 50 games due to the 500 limit issue (see #5). The user now has 50 games. Even if the API worked perfectly, if `insert_games` fails mid-way on game 100, the user loses the remaining 100 with no way to recover without a full resync.

**Hint:** Use an upsert strategy instead of delete-all + reinsert. In `insert_complete_game`, use `INSERT ... ON CONFLICT DO UPDATE` on the games table (conflict on a unique IGDB id or store_id). This way existing games are updated and new ones are added without deleting anything. Remove `prepare_db`/`clean()` entirely from the refresh flow. If you still want a "full resync" option, wrap `clean()` + `insert_games` in a single SQLite transaction so it's atomic — if insert fails, the transaction rolls back and the old data is preserved.

---

### 3. No database migrations at startup

**File:** `src-tauri/src/db.rs`

`DatabaseState::new()` creates the SQLite connection pool but never runs migrations. There are 12 migration files in `src-tauri/migrations/` that are never applied automatically. A fresh install will have an empty database with no tables.

```rust
// db.rs - DatabaseState::new() ends with:
let pool = SqlitePool::connect_with(connection).await?;
Ok(Self { pool })
// No sqlx::migrate!().run(&pool).await? anywhere
```

**Example of failure:** A user installs Rocade for the first time. The app creates `rocade.db` but no tables exist. Every query fails with "no such table: games". The app is unusable.

**Hint:** Add `sqlx::migrate!().run(&pool).await?;` right after `SqlitePool::connect_with(connection).await?` in `DatabaseState::new()`. The `sqlx::migrate!()` macro embeds the `src-tauri/migrations/` directory at compile time. SQLx tracks which migrations have already run via a `_sqlx_migrations` table, so it's safe to call on every startup — only new migrations will be applied.

---

### 4. `IgdbGameInfo` has non-optional fields that IGDB doesn't guarantee

**File:** `src-tauri/src/igdb.rs:32-42`

Several fields are declared as required (non-`Option`) even though IGDB doesn't guarantee they exist on every game:

```rust
pub struct IgdbGameInfo {
    cover: IgdbImage,                                  // not Option - many games have no cover
    genres: Vec<IgdbGenre>,                            // not Option - some games have no genres
    involved_companies: Vec<IgdbInvolvedCompany>,      // not Option - some games have no companies
    first_release_date: i64,                           // not Option - unreleased games have no date
}
```

**Example of failure:** Your Steam library contains an early access game with no release date. `serde_json::from_str` fails to deserialize it because `first_release_date` is missing from the JSON. In the `get_games` batch call, this causes the **entire** deserialization to fail - not just that one game. All 500 games are lost because of one incomplete entry.

**Hint:** Make all unreliable fields `Option` in `IgdbGameInfo`: `cover: Option<IgdbImage>`, `genres: Option<Vec<IgdbGenre>>`, `involved_companies: Option<Vec<IgdbInvolvedCompany>>`, `first_release_date: Option<i64>`. Then add `#[serde(default)]` on the `Vec` fields so missing arrays deserialize as empty vecs. Propagate the `Option` through `IgdbGame` (which already has `cover: IgdbImage` non-optional) and update `insert_complete_game` to conditionally insert covers/genres/developers only when present. The `IgdbGame.cover` field should become `Option<IgdbImage>` and the cover insert in `insert_complete_game` should be wrapped in `if let Some(cover) = game.cover { ... }`.

---

### 5. IGDB API has a 500 result limit - no pagination

**File:** `src-tauri/src/igdb.rs:198-201`

The IGDB `external_games` query sets `limit` to `game_ids.len()`, but IGDB's API caps responses at 500 results.

```rust
let query = format!(
    "fields *;  where external_game_source = 1 & uid = ({}); limit {};",
    steam_urls.join(","),
    game_ids.len() // Could be 1000+
);
```

**Example of failure:** A user with 800 Steam games triggers a refresh. The query asks for `limit 800` but IGDB caps at 500. Only 500 games are returned. 300 games silently vanish from the library.

**Hint:** In `get_steam_games`, chunk the `game_ids` into batches of 500 using `.chunks(500)` and send one IGDB query per chunk, then concatenate the results. Do the same in `get_games_infos` since the `/v4/games` endpoint has the same 500 limit. Example pattern:
```rust
let mut all_results = Vec::new();
for chunk in game_ids.chunks(500) {
    let query = format!("fields *; where ...; limit {};", chunk.len());
    let mut batch = self.request_and_parse(URL, &query).await?;
    all_results.append(&mut batch);
}
```

---

### 6. Steam API called over HTTP (not HTTPS)

**File:** `src-tauri/src/steam.rs:46`

The Steam API is called using plaintext HTTP. The API key is included as a URL query parameter, meaning it's transmitted in cleartext over the network.

```rust
let url = format!(
    "http://api.steampowered.com/IPlayerService/GetOwnedGames/v0001/?key={}&steamid={}...",
    self.key, self.profile_id
);
```

**Example of failure:** Anyone on the same network (coffee shop wifi, shared LAN) can intercept the request and read the Steam API key. The key can then be used to query the user's Steam data or abuse the API under their rate limits.

**Hint:** Change `"http://api.steampowered.com/..."` to `"https://api.steampowered.com/..."` in `SteamApiClient::get_games()`. Steam's API supports HTTPS. This is a one-character fix (`http` → `https`).

---

## Error Handling Issues

### 7. `unwrap_or_default()` silently produces empty config values

**File:** `src-tauri/src/lib.rs:37-40`

Missing `.env` variables now produce empty strings instead of panicking. This is an improvement over the old `panic!`, but the failure is now silent and deferred - the app starts fine but API calls fail later with cryptic errors.

```rust
let rocade_config = RocadeConfig {
    steam_api_key: env::var("STEAM_API_KEY").unwrap_or_default(),      // "" if missing
    steam_profile_id: env::var("STEAM_PROFILE_ID").unwrap_or_default(), // "" if missing
    twitch_client_id: env::var("TWITCH_CLIENT_ID").unwrap_or_default(), // "" if missing
    twitch_client_secret: env::var("TWITCH_CLIENT_SECRET").unwrap_or_default(), // "" if missing
};
```

**Example of failure:** A new user runs the app without a `.env` file. The app starts fine. They see the UI. They trigger a refresh. The Steam API call sends `key=&steamid=` and returns a 403 error. The IGDB client gets an empty client_id, sends a token request, and gets an authentication error. The user sees a vague error string with no indication that configuration is missing.

**Hint:** Validate the config values right after loading them in `lib.rs`. Check that none of the four required values are empty and return a clear error early. Example:
```rust
let steam_api_key = env::var("STEAM_API_KEY")
    .map_err(|_| "STEAM_API_KEY is not set in .env")?;
```
If you prefer a softer approach (app still launches), store them as `Option<String>` in `RocadeConfig` and check at the point of use, returning a `RocadeError::Config("STEAM_API_KEY is missing")` from the command layer so the frontend can display a meaningful message.

---

### 8. Multiple `expect()` calls that panic in setup

**Files:** `src-tauri/src/db.rs:15,17`, `src-tauri/src/lib.rs:47`, `src-tauri/src/igdb.rs:78,86`

Several `.expect()` calls in the app initialization will crash the entire application with an unhelpful panic message if they fail:

```rust
// db.rs
let app_dir = app_handle.path().app_data_dir()
    .expect("unable to get app directory");              // panic
fs::create_dir_all(&app_dir)
    .expect("unable to create app directory");           // panic

// lib.rs
let db_state = db::DatabaseState::new(handle).await
    .expect("unable to init local db");                  // panic

// igdb.rs
HeaderValue::from_str(twitch_client.get_client_id().as_str())
    .expect("unable to set igdb client id");             // panic
```

**Example of failure:** On a restricted Linux system, the app data directory is not writable. `fs::create_dir_all` fails. The user sees `thread 'main' panicked at 'unable to create app directory'` and the app closes. There is no dialog, no suggestion, no recovery.

**Hint:** In `db.rs`, change `DatabaseState::new` to propagate errors with `?` instead of `expect()`:
```rust
let app_dir = app_handle.path().app_data_dir()
    .map_err(|e| sqlx::Error::Configuration(e.into()))?;
fs::create_dir_all(&app_dir)
    .map_err(|e| sqlx::Error::Configuration(e.to_string().into()))?;
```
In `lib.rs`, the `setup` closure can return `Err(Box<dyn std::error::Error>)` — Tauri will display the error in a dialog instead of panicking. For `igdb.rs`, `IgdbApiClient::new` should return `Result<Self, String>` instead of using `expect()`, and the caller in `lib.rs` should handle the error.

---

### 9. `RocadeError` variants still wrap `String` instead of source errors

**File:** `src-tauri/src/commands/game.rs:15-23`

The custom error type is a good start, but every variant wraps `String`. This means the original error type, cause chain, and backtrace are all lost at the point of conversion.

```rust
#[derive(Debug, Serialize, Error)]
pub enum RocadeError {
    #[error("database error: {0}")]
    Database(String),        // loses sqlx::Error type info
    #[error("steam error: {0}")]
    Steam(String),           // loses reqwest::Error type info
    #[error("igddb error: {0}")]  // also: typo "igddb" should be "igdb"
    Igdb(String),
}
```

Additionally, the underlying API modules (`steam.rs`, `igdb.rs`, `twitch.rs`) still return `Result<_, String>` directly, so the error type only exists at the command layer. This means errors are converted to String twice: once in the API module, and once wrapping into `RocadeError`.

**Learning opportunity:** Look into `#[from]` attribute with `thiserror` and the `#[source]` attribute. With proper `From` impls, the `?` operator can convert errors automatically without manual `.map_err()` calls.

**Hint:** First, fix the typo `"igddb"` → `"igdb"`. Then create a proper error enum that wraps source errors. Since Tauri commands need `Serialize`, you can keep `String` for serialization but store the source error for debugging. A practical approach: change the API modules (`steam.rs`, `igdb.rs`, `twitch.rs`) to return their own error types instead of `Result<_, String>`, then add `#[from]` in `RocadeError`:
```rust
pub enum RocadeError {
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),
    #[error("steam error: {0}")]
    Steam(#[from] reqwest::Error),
    #[error("igdb error: {0}")]
    Igdb(String),
}
```
Note: Since `RocadeError` needs `Serialize` for Tauri, and `sqlx::Error`/`reqwest::Error` don't implement `Serialize`, you may need to implement `Serialize` manually or keep the `String` approach but add `impl From<sqlx::Error> for RocadeError` to avoid the `.map_err()` boilerplate.

---

### 10. No error handling or loading states on the frontend

**Files:** `src/stores/game.store.ts`, `src/pages/games/[id].vue`

All `invoke` calls are `await`ed with no `try/catch`, no loading indicator, and no error feedback.

```typescript
// game.store.ts - no try/catch
onMounted(async () => {
    let res: GameInfo[] = await getGames()
    if (!res.length) {
        await refreshGames()  // can throw silently
        res = await getGames();
    }
    games.value = res
})
```

**Example of failure:** The IGDB API is down. `refreshGames()` throws. The promise rejects. `games.value` is never set. The sidebar is empty with no indication of what went wrong.

**Hint:** Add `loading` and `error` refs to the game store and wrap invoke calls in try/catch:
```typescript
const loading = ref(false);
const error = ref<string | null>(null);

async function init() {
    loading.value = true;
    error.value = null;
    try {
        let res = await getGames();
        if (!res.length) {
            await refreshGames();
            res = await getGames();
        }
        games.value = res;
    } catch (e) {
        error.value = String(e);
    } finally {
        loading.value = false;
    }
}
```
Expose `loading` and `error` from the store and use them in the sidebar template to show a spinner or error message. Do the same in `[id].vue` for the `getGameById` call.

---

### 11. Publishers are fetched from IGDB but never stored in the database

**File:** `src-tauri/src/db/game.rs:177-193`

The `insert_complete_game` method inserts developers into the `developed_by` junction table, but `game.publishers` is completely ignored. The publisher data is fetched from IGDB, transferred through the entire pipeline, and then silently dropped at the database insert step.

```rust
// Only developers are inserted, publishers are never mentioned:
for developer in game.developers {
    let company_id = sqlx::query_scalar::<_, i64>(
        "INSERT INTO companies (igdb_id, name) VALUES (?, ?) ..."
    )
    // ...
}
// game.publishers is never used
```

**Example of failure:** You add a "Publisher" field to the game detail page. It's always empty because the data was never persisted.

**Hint:** Add a `published_by` junction table (similar to `developed_by`) via a new migration:
```sql
CREATE TABLE IF NOT EXISTS published_by (
    game_id INTEGER NOT NULL REFERENCES games(id),
    company_id INTEGER NOT NULL REFERENCES companies(id),
    PRIMARY KEY (game_id, company_id)
);
```
Then in `insert_complete_game`, add a loop for publishers right after the developers loop:
```rust
for publisher in game.publishers {
    let company_id = sqlx::query_scalar::<_, i64>(
        "INSERT INTO companies (igdb_id, name) VALUES (?, ?)
         ON CONFLICT(igdb_id) DO UPDATE SET igdb_id = igdb_id RETURNING id"
    ).bind(publisher.id).bind(&publisher.name).fetch_one(&mut *tx).await?;

    sqlx::query("INSERT INTO published_by (game_id, company_id) VALUES (?, ?)")
        .bind(id).bind(company_id).execute(&mut *tx).await?;
}
```
Add a `LEFT JOIN` for `published_by` in `BASE_QUERY` and a `publishers` field to the `Game` struct.

---

## Rust Idiom Issues

### 12. `trigrams` and `similarity` take `String` by value instead of `&str`

**File:** `src-tauri/src/commands/game.rs:61,73`

These functions take ownership of `String` values, forcing unnecessary allocations at every call site. They only need to read the string.

```rust
pub fn trigrams(s: String) -> HashSet<String> { ... }
pub fn similarity(a: String, b: String) -> f64 { ... }

// Called like this, forcing clones:
similarity(
    name.clone().to_ascii_lowercase(),      // clone + allocate
    game.name.clone().to_ascii_lowercase(), // clone + allocate
)
```

**Learning point:** In Rust, if a function only needs to *read* a string, it should take `&str`. This lets callers pass `&String`, `&str`, or string slices without copying. The `clone()` + `to_ascii_lowercase()` chain is doing two allocations when one would suffice.

**Hint:** Change the signatures to take `&str` and remove the clones at call sites:
```rust
pub fn trigrams(s: &str) -> HashSet<String> {
    let s_with_spaces = format!("  {} ", s);
    // ... rest unchanged
}

pub fn similarity(a: &str, b: &str) -> f64 {
    let tri_a = trigrams(a);
    let tri_b = trigrams(b);
    tri_a.intersection(&tri_b).count() as f64 / tri_a.len() as f64
}
```
Then at the call site in `get_games`, compute the lowercase once and pass references:
```rust
let name_lower = name.to_ascii_lowercase();
games = games.into_iter().filter(|game| {
    let game_lower = game.name.to_ascii_lowercase();
    game_lower.contains(&name_lower) || similarity(&name_lower, &game_lower) > 0.4
}).collect();
```

---

### 13. `similarity` still uses explicit `return`

**File:** `src-tauri/src/commands/game.rs:77`

```rust
pub fn similarity(a: String, b: String) -> f64 {
    let tri_a = trigrams(a);
    let tri_b = trigrams(b);

    return tri_a.intersection(&tri_b).count() as f64 / tri_a.len() as f64;
}
```

Idiomatic Rust uses the last expression as the implicit return value. The `return` keyword is typically reserved for early returns in control flow.

**Hint:** Remove the `return` keyword — the last expression is already the return value:
```rust
pub fn similarity(a: &str, b: &str) -> f64 {
    let tri_a = trigrams(a);
    let tri_b = trigrams(b);
    tri_a.intersection(&tri_b).count() as f64 / tri_a.len() as f64
}
```

---

### 14. `SteamClient` struct and `new()` method are dead code

**File:** `src-tauri/src/steam.rs:62-67`

`SteamClient` is an empty struct where every method is an associated function (no `&self` parameter). The `new()` method is never called anywhere in the codebase.

```rust
pub struct SteamClient {}

impl SteamClient {
    pub fn new() -> Self { SteamClient {} }  // never called
    fn get_steam_dir() -> Result<PathBuf, String> { ... }
    pub fn install_game(app_handle: AppHandle, steam_game_id: String) -> Result<bool, String> { ... }
    pub fn is_steam_game_installed(game_id: String) -> bool { ... }
}

// Called as:
SteamClient::is_steam_game_installed(store_id);  // no instance needed
```

The struct serves purely as a namespace. In Rust, you can just use free functions in a module - that's what modules are for.

**Hint:** Remove the `SteamClient` struct and `new()` entirely. Convert the associated functions into free module-level functions in `steam.rs`:
```rust
// steam.rs - remove `pub struct SteamClient {}` and `impl SteamClient { ... }`
// Instead, just have:
fn get_steam_dir() -> Result<PathBuf, String> { ... }
pub fn install_game(app_handle: AppHandle, steam_game_id: String) -> Result<bool, String> { ... }
pub fn is_steam_game_installed(game_id: String) -> bool { ... }
pub fn uninstall_game(app_handle: AppHandle, steam_game_id: String) -> Result<bool, String> { ... }
```
Update call sites from `SteamClient::is_steam_game_installed(...)` to `steam::is_steam_game_installed(...)` (or import the function directly).

---

### 15. `env::home_dir()` is deprecated since Rust 1.29

**File:** `src-tauri/src/steam.rs:72`

`std::env::home_dir()` has been deprecated because it behaves incorrectly on Windows.

```rust
let mut user_dir = match env::home_dir() { ... };
```

Consider the `dirs` crate (`dirs::home_dir()`) or Tauri's path resolver.

**Hint:** Add the `dirs` crate to `Cargo.toml` (`dirs = "6"`) and replace the call:
```rust
// Before:
let mut user_dir = match env::home_dir() { ... };
// After:
let mut user_dir = dirs::home_dir()
    .ok_or_else(|| "unable to get user home directory".to_string())?;
```
Alternatively, since the functions `install_game` and `uninstall_game` already receive a Tauri `AppHandle`, you could use Tauri's path resolver (`app_handle.path().home_dir()`) and thread the `AppHandle` into `get_steam_dir` to avoid adding a new dependency.

---

### 16. Unused `window` import

**File:** `src-tauri/src/commands/game.rs:12`

```rust
use tauri::{async_runtime::Mutex, window, AppHandle, State};
//                                 ^^^^^^ unused
```

This adds noise and will trigger a compiler warning.

**Hint:** Remove `window` from the import in `src-tauri/src/commands/game.rs:12`:
```rust
use tauri::{async_runtime::Mutex, AppHandle, State};
```

---

### 17. `futures` crate in Cargo.toml appears unused

**File:** `src-tauri/Cargo.toml:26`

```toml
futures = "0.3.31"
```

No `use futures::` import appears in any Rust source file. This adds unnecessary compile time and dependency weight.

**Hint:** Remove the line `futures = "0.3.31"` from `src-tauri/Cargo.toml` and run `cargo check` to confirm nothing breaks. If the build succeeds, it was indeed unused.

---

### 18. `TwitchApiClient` clones access token excessively

**File:** `src-tauri/src/twitch.rs:44-50`

```rust
self.access_token = Some(parsed.access_token.clone()); // clone 1
Ok(parsed.access_token.clone())                         // clone 2 - parsed is about to drop anyway

pub fn get_access_token(&self) -> Option<String> {
    self.access_token.clone()                           // clone 3 - every time token is checked
}

pub fn get_client_id(&self) -> String {
    self.client_id.clone()                              // also clones every call
}
```

**Learning point:** The second clone in `refresh_access_token` is unnecessary - `parsed.access_token` is about to be dropped, so you can move it into `self.access_token` then clone from the stored value. For `get_access_token` and `get_client_id`, consider returning `&str` references instead of cloned `String`s.

**Hint:** Refactor `refresh_access_token` to move instead of clone, and return a reference from getters:
```rust
pub async fn refresh_access_token(&mut self) -> Result<String, String> {
    // ... fetch and parse ...
    let token = parsed.access_token;           // move, no clone
    self.access_token = Some(token.clone());    // one clone to store
    Ok(token)                                   // move into Ok
}

pub fn get_access_token(&self) -> Option<&str> {
    self.access_token.as_deref()
}

pub fn get_client_id(&self) -> &str {
    &self.client_id
}
```
The callers in `igdb.rs` that use `get_client_id()` and `get_access_token()` already work with `&str` — `HeaderValue::from_str()` and `bearer_auth()` both accept `&str`.

---

### 19. API secrets passed as URL query parameters

**Files:** `src-tauri/src/steam.rs:46`, `src-tauri/src/twitch.rs:32`

Both the Steam API key and Twitch client secret are embedded directly in URL query strings:

```rust
// steam.rs
format!("http://...?key={}&steamid={}", self.key, self.profile_id)

// twitch.rs
format!("https://id.twitch.tv/oauth2/token?client_id={}&client_secret={}", ...)
```

If these URLs are logged (by reqwest in debug mode, by a proxy, in error messages, or in stack traces), the secrets are exposed.

**Learning point:** Use `.query()` or `.form()` methods on the request builder instead of string formatting URLs. For POST requests like Twitch, the body/form approach is standard for OAuth.

**Hint:** For Steam (`steam.rs`), use reqwest's `.query()` builder:
```rust
let res = self.client
    .get("https://api.steampowered.com/IPlayerService/GetOwnedGames/v0001/")
    .query(&[("key", &self.key), ("steamid", &self.profile_id),
             ("include_appinfo", &"1".to_string()), ("format", &"json".to_string())])
    .send().await.map_err(|e| e.to_string())?;
```
For Twitch (`twitch.rs`), use `.form()` since it's a POST with `client_credentials`:
```rust
let res = self.client
    .post("https://id.twitch.tv/oauth2/token")
    .form(&[("client_id", &self.client_id), ("client_secret", &self.client_secret),
            ("grant_type", &"client_credentials".to_string())])
    .send().await.map_err(|e| e.to_string())?;
```
This way, secrets never appear in the URL string and won't be logged.

---

## Frontend Issues

### 20. `watchEffect` with async callback in game detail page

**File:** `src/pages/games/[id].vue:98-105`

`watchEffect` does not properly handle async callbacks. The returned Promise is ignored, meaning errors are silently swallowed and cleanup/disposal doesn't wait for the async work.

```typescript
watchEffect(async () => {
    game.value = await getGameById(id.value);
    if (game.value && game.value.release_date) {
        releaseDate.value = format(new Date(game.value.release_date * 1000), "MMMM dd, yyyy")
    }
})
```

**Example of failure:** User navigates rapidly between games. Multiple `getGameById` calls fire concurrently. Responses return out of order. The UI displays data for a different game than the one selected (race condition).

**Hint:** Replace `watchEffect(async () => ...)` with a `watch` on `id` and guard against stale responses:
```typescript
watch(id, async (newId) => {
    const result = await getGameById(newId);
    // Guard: only update if the route hasn't changed while we were fetching
    if (id.value === newId) {
        game.value = result;
        if (result?.release_date) {
            releaseDate.value = format(new Date(result.release_date * 1000), "MMMM dd, yyyy");
        }
    }
}, { immediate: true });
```
This prevents race conditions by checking that the current `id` still matches the one we fetched for.

---

### 21. `refreshGames` is called but not imported in the game store

**File:** `src/stores/game.store.ts:1,16`

```typescript
import { getGames } from "@/commands/game.command";  // only getGames imported
// ...
await refreshGames()  // line 16 - not imported!
```

`refreshGames` is exported from `game.command.ts` but never imported in the store. This would cause a `ReferenceError` at runtime when the store tries to call it.

**Hint:** Add `refreshGames` to the import in `src/stores/game.store.ts`:
```typescript
import { getGames, refreshGames } from "@/commands/game.command";
```

---

### 22. `IgdbImgSize` type does not exist

**File:** `src/api/igdb.ts:1`

```typescript
import { IgdbImgSize } from "@/types/igdb";  // IgdbImgSize doesn't exist
```

The `types/igdb.ts` file exports `ImageSize` and `IgdbImage`, but not `IgdbImgSize`. This should either be `IgdbImage` (which is `t_${ImageSize}`) or `ImageSize` depending on whether the caller is expected to include the `t_` prefix.

**Hint:** The callers (`igdb.ts`, `GameSidebarItem.vue`, `[id].vue`) all pass values like `'t_cover_small'` and `'t_1080p'` — these match the `IgdbImage` type (`t_${ImageSize}`). Fix the import in `src/api/igdb.ts`:
```typescript
import { IgdbImage } from "@/types/igdb";

export function getIgdbImageUrl(id: string, size: IgdbImage): string {
    return `https://images.igdb.com/igdb/image/upload/${size}/${id}.jpg`;
}
```

---

### 23. `onMounted` inside a Pinia store is an anti-pattern

**File:** `src/stores/game.store.ts:12`

```typescript
export const useGameStore = defineStore('game', () => {
    onMounted(async () => {    // component lifecycle hook inside a store
        let res = await getGames()
        // ...
    })
})
```

`onMounted` is a **component** lifecycle hook. It works here only because the store happens to be initialized inside a component's `setup()`. If the store is ever accessed outside a component context (in a router guard, in another store, in a utility function), `onMounted` will silently do nothing.

**Learning point:** Pinia stores should use explicit initialization methods or actions. A common pattern is an `init()` action that the root component calls, or using `$onAction` / `$subscribe` for side effects.

**Hint:** Replace `onMounted` with an explicit `init` function exposed from the store:
```typescript
// game.store.ts
async function init() {
    let res = await getGames();
    if (!res.length) {
        await refreshGames();
        res = await getGames();
    }
    games.value = res;
}

return { games, filteredGames, search, init };
```
Then call `init()` from the component that uses the store (e.g. `AppSidebar.vue`):
```typescript
const store = useGameStore();
onMounted(() => store.init());
```
This makes the store independent of component lifecycle and safe to use from anywhere.

---

### 24. Empty `<style scoped></style>` tags in Vue components

**Files:** `src/pages/games.vue`, `src/pages/games/[id].vue`, `src/components/app-sidebar/AppSidebar.vue`, `src/components/app-sidebar/game-sidebar-item/GameSidebarItem.vue`, `src/App.vue`

Every component has an empty scoped style block. This adds noise and no value.

**Hint:** Remove the `<style scoped></style>` tags from all five files: `games.vue`, `[id].vue`, `AppSidebar.vue`, `GameSidebarItem.vue`, and `App.vue`. They can always be added back when actual styles are needed. If using a linter/formatter that adds them automatically, configure it to omit empty blocks.

---

## Architecture & Design Issues

### 25. Filtering done in Rust memory instead of SQL

**File:** `src-tauri/src/commands/game.rs:35-56`

The `get_games` command fetches **all** games from the database with all joins, then filters them in Rust using trigrams. Every search keystroke loads the entire games table into memory.

```rust
let mut games = game_repository.get_games().await?;  // loads ALL games with all joins
if let Some(name) = query.and_then(|q| q.name) {
    games = games.into_iter().filter(|game| { ... }).collect(); // filters in memory
}
```

**Example of failure:** A user with 2000+ Steam games types in the search box. Every keystroke loads all 2000 games with their genres, developers, covers, and artworks from SQLite, deserializes them, then filters. This causes noticeable UI lag.

**Hint:** Two approaches, from simplest to best:

1. **Quick fix — SQL `LIKE` filter:** Add a `WHERE games.name LIKE ?` clause in `GameRepository` when a name query is provided. This pushes the basic substring search to SQLite and avoids loading all games. Keep the trigram similarity as a fallback for fuzzy matches only.

2. **Better — SQLite FTS5:** Create a virtual table with FTS5 for full-text search:
```sql
CREATE VIRTUAL TABLE games_fts USING fts5(name, content=games, content_rowid=id);
```
Populate it with triggers on insert/update/delete. Then search with `SELECT * FROM games_fts WHERE games_fts MATCH ?`. This gives fast prefix and substring searches without loading everything into memory.

Also add a debounce (300ms) on the frontend `search` watcher in `game.store.ts` to avoid firing a query on every keystroke.

---

### 26. `similarity()` function is asymmetric

**File:** `src-tauri/src/commands/game.rs:73-78`

The similarity function divides by the length of `tri_a` (the search term), not by the union or maximum of both sets. This means `similarity("ab", "abcdefghij")` gives a very different result than `similarity("abcdefghij", "ab")`.

```rust
pub fn similarity(a: String, b: String) -> f64 {
    let tri_a = trigrams(a);
    let tri_b = trigrams(b);
    return tri_a.intersection(&tri_b).count() as f64 / tri_a.len() as f64;
    //                                                    ^^^^^^^^^^
    //     always divides by first argument's trigram count
}
```

**Example of failure:** Searching for "cs" (short term) gives high similarity to "Counter-Strike" because most of "cs"'s trigrams are found. But this also means short search terms match almost everything, since they have few trigrams and most of them will appear somewhere. A 2-letter search will have very few trigrams and the threshold of 0.4 will be easy to pass.

**Learning point:** Standard trigram similarity (as used by PostgreSQL's `pg_trgm`) divides by the **union** of both sets: `|A ∩ B| / |A ∪ B|`. This gives a symmetric, more meaningful similarity score.

**Hint:** Change the denominator to use the union size (Jaccard index):
```rust
pub fn similarity(a: &str, b: &str) -> f64 {
    let tri_a = trigrams(a);
    let tri_b = trigrams(b);
    let intersection = tri_a.intersection(&tri_b).count() as f64;
    let union = tri_a.union(&tri_b).count() as f64;
    if union == 0.0 { return 0.0; }
    intersection / union
}
```
You may need to lower the threshold from `0.4` to something like `0.2` or `0.3` since Jaccard similarity gives lower scores than the current asymmetric formula. Test with a few game names to calibrate.

---

### 27. Steam path is Linux-only (no cross-platform support)

**File:** `src-tauri/src/steam.rs:77-80`

The Steam directory is hardcoded to the Linux path:

```rust
user_dir.push(r".local");
user_dir.push("share");
user_dir.push("Steam");
user_dir.push("steamapps");
```

This means `is_steam_game_installed`, `install_game`, and `uninstall_game` only work on Linux.

**Hint:** Use conditional compilation or a runtime OS check to build the Steam path per platform:
```rust
fn get_steam_dir() -> Result<PathBuf, String> {
    let home = dirs::home_dir()
        .ok_or_else(|| "unable to get user home directory".to_string())?;

    #[cfg(target_os = "linux")]
    let steam_dir = home.join(".local/share/Steam/steamapps");

    #[cfg(target_os = "windows")]
    let steam_dir = PathBuf::from(r"C:\Program Files (x86)\Steam\steamapps");

    #[cfg(target_os = "macos")]
    let steam_dir = home.join("Library/Application Support/Steam/steamapps");

    Ok(steam_dir)
}
```
For Windows, you may also want to check the registry (`HKCU\Software\Valve\Steam\SteamPath`) for non-default install locations.

---

### 28. `prepare_db` and `insert_games` take `State` wrapper instead of inner types

**File:** `src-tauri/src/commands/game.rs:109-122`

These private helper functions accept `State<'_, T>` (Tauri's injection wrapper) instead of `&T`:

```rust
async fn prepare_db(db_state: State<'_, DatabaseState>) -> Result<(), sqlx::Error> { ... }
async fn insert_games(game_repository: State<'_, GameRepository>, ...) -> Result<(), sqlx::Error> { ... }
```

This unnecessarily couples internal logic to Tauri's dependency injection. These functions should take `&DatabaseState` and `&GameRepository` directly, making them testable and reusable without Tauri.

**Hint:** Change the helper function signatures to accept references directly. `State<'_, T>` implements `Deref<Target = T>`, so you just need to dereference at the call site:
```rust
async fn prepare_db(db_state: &DatabaseState) -> Result<(), sqlx::Error> {
    db_state.clean().await
}

async fn insert_games(game_repository: &GameRepository, games: Vec<IgdbGame>) -> Result<(), sqlx::Error> {
    for game in games { game_repository.insert_complete_game(game).await?; }
    Ok(())
}
```
Then in `refresh_games`, call them as `prepare_db(&db_state)` and `insert_games(&game_repository, igdb_games)`. The `State` wrapper auto-derefs to `&T`.

---

### 29. `is_steam_game_installed` reads and parses file but could use simpler check

**File:** `src-tauri/src/steam.rs:110-153`

The function checks if a game is installed by:
1. Checking if the manifest file exists
2. Reading the entire file
3. Parsing `BytesToDownload` and `BytesDownloaded`
4. Comparing them

The bytes comparison is clever (it detects partial downloads), but the function takes `game_id: String` by value when `&str` would suffice, and the manual line parsing could use a more robust approach.

Also, if the manifest file exists but is empty or malformed (missing both fields), `bytes_to_download` and `bytes_downloaded` remain `None`, and `matches!` returns `false`. This means a fully installed game whose manifest lacks these fields would be reported as not installed.

**Hint:** Change the function signature to take `&str` instead of `String`:
```rust
pub fn is_steam_game_installed(game_id: &str) -> bool { ... }
```
For the missing-fields case, treat "manifest exists but fields are absent" as installed (a game that's fully installed may not have download progress fields). Add a fallback:
```rust
match (bytes_to_download, bytes_downloaded) {
    (Some(to_dl), Some(downloaded)) => to_dl == downloaded,
    _ => true, // manifest exists, no download tracking = assume installed
}
```
Update the call site in `commands/game.rs` from `SteamClient::is_steam_game_installed(store_id)` to pass `&store_id`.

---

### 30. No SQLite foreign key enforcement

**File:** `src-tauri/src/db.rs:21-24`

SQLite requires `PRAGMA foreign_keys = ON` **per connection** to enforce foreign key constraints. This is never set:

```rust
let connection = SqliteConnectOptions::new()
    .filename(&db_path)
    .create_if_missing(true)
    .journal_mode(sqlx::sqlite::SqliteJournalMode::Wal);
// No .pragma("foreign_keys", "ON")
```

This means the database accepts orphaned references (e.g., a `belongs_to` row pointing to a deleted game or genre) without error. Data integrity depends entirely on application code being correct.

**Hint:** Add the foreign keys pragma to the connection options in `db.rs`:
```rust
let connection = SqliteConnectOptions::new()
    .filename(&db_path)
    .create_if_missing(true)
    .journal_mode(sqlx::sqlite::SqliteJournalMode::Wal)
    .pragma("foreign_keys", "ON");
```
SQLx's `SqliteConnectOptions::pragma()` sets it for every connection in the pool. After enabling this, also add `ON DELETE CASCADE` to your foreign key constraints in a new migration so that deleting a game automatically cleans up its junction table rows (covers, artworks, belongs_to, etc.), which will fix the `clean()` method's delete ordering issue.

---

### 31. Inconsistent naming: `studios` alias in SQL while table is `companies`

**File:** `src-tauri/src/db/game.rs:32`

```sql
json_group_array(distinct companies.name) as studios,
```

The table was renamed from `studios` to `companies` (migration 11), but the SQL alias still says `studios`. The Rust mapping code also uses `"studios"` as the column name (`row.get("studios")`). While technically functional, this is confusing - reading the code suggests a `studios` table that doesn't exist.

**Hint:** Rename the SQL alias in `BASE_QUERY` from `studios` to `developers` (since the query joins through `developed_by`):
```sql
json_group_array(distinct companies.name) as developers,
```
Then update `map_game_row` to match:
```rust
let developers_json: Option<String> = row.get("developers");
```
This aligns the SQL alias, the Rust variable name, and the `Game` struct field (`developers`).

---

### 32. Migration history contains schema churn

**Files:** `src-tauri/migrations/`

The migrations contain back-and-forth changes:
- Migration 5 adds `release_date` as `text`
- Migration 6 drops it and re-adds as `integer`
- Migration 7 creates `games_genres`
- Migration 12 renames it to `belongs_to`
- Migration 9 creates `studios`
- Migration 11 renames it to `companies`

Before a release, these should be squashed into a clean initial schema. Running 12 migrations on first install (including drops and renames) is slower and harder to audit than a single clean migration.

**Hint:** Since the app hasn't been released yet (no users have existing databases to migrate), delete all 12 migration files in `src-tauri/migrations/` and create a single `0001_initial_schema.sql` that defines the final schema in one clean migration. Include all tables (`games`, `companies`, `genres`, `covers`, `artworks`, `belongs_to`, `developed_by`, `games_store`) with their final column types and constraints. This is safe because no production database depends on the migration history yet.

---

## Summary by Priority

| Priority | Issue | Impact |
|----------|-------|--------|
| Critical | INNER JOIN drops games (#1) | Games silently missing from library |
| Critical | No migration runner (#3) | App unusable on fresh install |
| Critical | Non-optional IGDB fields (#4) | Entire batch deserialization fails |
| Critical | `refreshGames` not imported (#21) | Runtime crash on first use |
| Critical | `IgdbImgSize` type missing (#22) | TypeScript compilation error |
| High | Destructive refresh (#2) | Data loss on API failure |
| High | IGDB 500 limit (#5) | Games silently missing |
| High | HTTP for Steam API (#6) | API key transmitted in cleartext |
| High | No frontend error handling (#10) | Silent failures, blank UI |
| High | Publishers never stored (#11) | Data silently dropped |
| Medium | Silent empty config (#7) | Cryptic errors when .env missing |
| Medium | `expect()` panics (#8) | App crash with no recovery |
| Medium | Error type improvement (#9) | Poor error diagnostics |
| Medium | watchEffect async race (#20) | Wrong game displayed |
| Medium | Filtering in memory (#25) | Performance on large libraries |
| Medium | Asymmetric similarity (#26) | Inconsistent search results |
| Medium | `onMounted` in store (#23) | Fragile initialization |
| Medium | Secrets in URL params (#19) | Credential leakage risk |
| Medium | No FK enforcement (#30) | Data integrity risk |
| Low | `String` by value (#12) | Unnecessary allocations |
| Low | Explicit return (#13) | Non-idiomatic Rust |
| Low | Dead `SteamClient` code (#14) | Code noise |
| Low | Deprecated `home_dir` (#15) | Future compatibility |
| Low | Unused imports (#16) | Compiler warnings |
| Low | Unused `futures` crate (#17) | Unnecessary build time |
| Low | Excessive cloning (#18) | Minor performance |
| Low | Empty style tags (#24) | Code noise |
| Low | Steam Linux-only (#27) | No cross-platform |
| Low | `State` in helpers (#28) | Testability |
| Low | `studios` alias (#31) | Confusing naming |
| Low | Migration churn (#32) | Maintenance burden |
