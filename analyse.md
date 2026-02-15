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

---

### 15. `env::home_dir()` is deprecated since Rust 1.29

**File:** `src-tauri/src/steam.rs:72`

`std::env::home_dir()` has been deprecated because it behaves incorrectly on Windows.

```rust
let mut user_dir = match env::home_dir() { ... };
```

Consider the `dirs` crate (`dirs::home_dir()`) or Tauri's path resolver.

---

### 16. Unused `window` import

**File:** `src-tauri/src/commands/game.rs:12`

```rust
use tauri::{async_runtime::Mutex, window, AppHandle, State};
//                                 ^^^^^^ unused
```

This adds noise and will trigger a compiler warning.

---

### 17. `futures` crate in Cargo.toml appears unused

**File:** `src-tauri/Cargo.toml:26`

```toml
futures = "0.3.31"
```

No `use futures::` import appears in any Rust source file. This adds unnecessary compile time and dependency weight.

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

---

### 21. `refreshGames` is called but not imported in the game store

**File:** `src/stores/game.store.ts:1,16`

```typescript
import { getGames } from "@/commands/game.command";  // only getGames imported
// ...
await refreshGames()  // line 16 - not imported!
```

`refreshGames` is exported from `game.command.ts` but never imported in the store. This would cause a `ReferenceError` at runtime when the store tries to call it.

---

### 22. `IgdbImgSize` type does not exist

**File:** `src/api/igdb.ts:1`

```typescript
import { IgdbImgSize } from "@/types/igdb";  // IgdbImgSize doesn't exist
```

The `types/igdb.ts` file exports `ImageSize` and `IgdbImage`, but not `IgdbImgSize`. This should either be `IgdbImage` (which is `t_${ImageSize}`) or `ImageSize` depending on whether the caller is expected to include the `t_` prefix.

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

---

### 24. Empty `<style scoped></style>` tags in Vue components

**Files:** `src/pages/games.vue`, `src/pages/games/[id].vue`, `src/components/app-sidebar/AppSidebar.vue`, `src/components/app-sidebar/game-sidebar-item/GameSidebarItem.vue`, `src/App.vue`

Every component has an empty scoped style block. This adds noise and no value.

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

---

### 28. `prepare_db` and `insert_games` take `State` wrapper instead of inner types

**File:** `src-tauri/src/commands/game.rs:109-122`

These private helper functions accept `State<'_, T>` (Tauri's injection wrapper) instead of `&T`:

```rust
async fn prepare_db(db_state: State<'_, DatabaseState>) -> Result<(), sqlx::Error> { ... }
async fn insert_games(game_repository: State<'_, GameRepository>, ...) -> Result<(), sqlx::Error> { ... }
```

This unnecessarily couples internal logic to Tauri's dependency injection. These functions should take `&DatabaseState` and `&GameRepository` directly, making them testable and reusable without Tauri.

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

---

### 31. Inconsistent naming: `studios` alias in SQL while table is `companies`

**File:** `src-tauri/src/db/game.rs:32`

```sql
json_group_array(distinct companies.name) as studios,
```

The table was renamed from `studios` to `companies` (migration 11), but the SQL alias still says `studios`. The Rust mapping code also uses `"studios"` as the column name (`row.get("studios")`). While technically functional, this is confusing - reading the code suggests a `studios` table that doesn't exist.

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
