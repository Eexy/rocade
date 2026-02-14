# Rocade - Code Analysis

## Critical Issues

### 1. Transaction bug in `insert_complete_game` - game insert runs outside transaction

**File:** `src-tauri/src/db/game.rs:153`

The first `INSERT INTO games` query uses `pool` directly instead of `&mut *tx`. This means the game row is inserted outside the transaction, while covers, genres, artworks, and developers are inside the transaction. If any of those fail and the transaction rolls back, you end up with an orphan game row in the database with no associated data.

```rust
// Current (broken): uses pool, not the transaction
let id = sqlx::query_scalar::<_, i64>(
    r#"insert into games (name, summary, release_date) values ( ?, ?, ?) returning id"#,
)
.bind(&game.name)
.bind(&game.summary)
.bind(&game.release_date)
.fetch_one(pool)       // <-- BUG: should be &mut *tx
.await?;
```

**Example of failure:** If inserting a genre fails (e.g. database constraint), the transaction rolls back. But the game row is already committed because it bypassed the transaction. You now have a game with no genres, no cover, no developers. The next `refresh_games` call will `clean()` and delete it, but any read in between will return broken data.

**Fix:** Change `.fetch_one(pool)` to `.fetch_one(&mut *tx)`.

---

### 2. Trigram function panics on non-ASCII game names

**File:** `src-tauri/src/commands/game.rs:53`

The `trigrams()` function uses byte-based string slicing (`s_with_spaces[i..i + 3]`). This will **panic at runtime** if any game name contains multi-byte UTF-8 characters (accented letters, Japanese, Chinese, Korean, emojis, etc.).

```rust
// This panics on "Pokémon" because 'é' is 2 bytes
for i in 0..s_with_spaces.len() - 2 {
    hashset.insert(s_with_spaces[i..i + 3].to_string()); // PANIC on byte boundary
}
```

**Example of failure:** A user searches for "pokemon" while their library contains "Pokémon Legends: Arceus". The function tries to slice into the middle of the `é` character (2 bytes in UTF-8) and the app **crashes**.

**Fix:** Use `.chars()` and iterate over character windows instead of byte indices:

```rust
pub fn trigrams(s: &str) -> HashSet<String> {
    let s_with_spaces = format!("  {} ", s);
    let chars: Vec<char> = s_with_spaces.chars().collect();
    let mut hashset = HashSet::new();
    for window in chars.windows(3) {
        hashset.insert(window.iter().collect());
    }
    hashset
}
```

---

### 3. `INNER JOIN` silently drops games missing any relation

**File:** `src-tauri/src/db/game.rs:36-42`

The `get_games` and `get_game_by_id` queries use `INNER JOIN` for **all** relations (genres, developers, artworks, covers, games_store). If a game is missing even one genre, one cover, or one developer, it completely disappears from results.

```sql
-- If a game has no artwork, this INNER JOIN removes it from results entirely
inner join artworks on artworks.game_id = games.id
inner join covers on covers.game_id = games.id
```

**Example of failure:** IGDB returns a game with `artworks: None` (the field is `Option<Vec<IgdbImage>>`). No artworks are inserted. The game exists in the `games` table but the `INNER JOIN artworks` causes it to silently vanish from all queries. The user's Steam library shows fewer games than expected with no error.

**Fix:** Use `LEFT JOIN` for optional relations:

```sql
LEFT JOIN artworks ON artworks.game_id = games.id
LEFT JOIN covers ON covers.game_id = games.id
```

---

### 4. `refresh_games` wipes the entire database on every sync

**File:** `src-tauri/src/commands/game.rs:80-86`

Every time a user refreshes their library, the `clean()` method deletes all data from every table, then re-fetches and re-inserts everything from scratch. If the IGDB API fails mid-sync (rate limit, network error, outage), the user loses their entire library with no way to recover.

```rust
prepare_db(db_state.clone()).await.map_err(|e| e.to_string())?; // deletes everything
insert_games(db_state.clone(), igdb_games).await.map_err(|e| e.to_string())?; // might fail
```

**Example of failure:** User has 200 games in their library. They click refresh. `clean()` deletes all 200 games. The IGDB API returns a 429 rate limit error after 50 games. The user now has 50 games instead of 200, and no way to get the other 150 back without a successful full resync.

**Fix:** Use an upsert strategy (`INSERT ... ON CONFLICT DO UPDATE`) instead of delete + re-insert. Or at least wrap the entire clean + insert in a single database transaction so it's atomic.

---

### 5. No database migrations at startup

**File:** `src-tauri/src/db.rs`

`DatabaseState::new()` creates the SQLite connection pool but never runs migrations. There is no `sqlx::migrate!().run(&pool).await` call. The 12 migration files in `src-tauri/migrations/` are not applied automatically, meaning a fresh install would have an empty database with no tables.

**Example of failure:** A user installs Rocade for the first time. The app creates `rocade.db` but no tables exist. Every query fails with "no such table: games". The app is unusable until migrations are run manually via CLI.

**Fix:** Add migration execution after pool creation:

```rust
sqlx::migrate!().run(&pool).await?;
```

---

## Error Handling Issues

### 6. `panic!` for missing environment variables instead of graceful error

**File:** `src-tauri/src/lib.rs:60-62, 71-73`

Missing `.env` variables cause a hard `panic!`, crashing the entire application with no user-facing error message. The user sees a raw crash with no explanation.

```rust
_ => {
    panic!("Unable to load steam config. Missing STEAM_KEY or STEAM_PROFILE_ID in dotenv file")
}
```

**Example of failure:** A new user clones the project, runs `cargo tauri dev` without creating a `.env` file. The app crashes on startup with a panic. The error message mentions "STEAM_KEY" but the actual variable name in the code is "STEAM_API_KEY" - so even the panic message is misleading.

**Fix:** Return a proper `Err` from the `setup` closure, or display a dialog box explaining what configuration is missing.

---

### 7. No custom error type - all errors mapped to `String`

**Files:** All Rust files use `.map_err(|e| e.to_string())`

Every error in the application is converted to a plain `String` with `.map_err(|e| e.to_string())`. This loses error context (type, cause chain, backtrace) and makes it impossible to programmatically handle different error kinds on the frontend.

```rust
// This is repeated ~30 times across the codebase
.map_err(|e| e.to_string())?;
```

**Example of failure:** The frontend receives `"error returned from database: (code: 1555) UNIQUE constraint failed"` as a raw string. It cannot distinguish between a network error, a database error, or an API error to show the user an appropriate message or retry strategy.

**Fix:** Create an `AppError` enum implementing `std::error::Error` and `serde::Serialize`:

```rust
#[derive(Debug, thiserror::Error, Serialize)]
pub enum AppError {
    #[error("Database error: {0}")]
    Database(String),
    #[error("Steam API error: {0}")]
    SteamApi(String),
    #[error("IGDB API error: {0}")]
    IgdbApi(String),
}
```

---

### 8. `dbg!()` macros left in production code

**File:** `src-tauri/src/db/game.rs:156, 191, 212` and `src-tauri/src/commands/game.rs:101`

Multiple `dbg!()` calls are left in the code. These write to stderr in production, leak internal state (database IDs), and slow down inserts in loops.

```rust
dbg!(&id);         // line 156 - after every game insert
dbg!(&genre_id);   // line 191 - after every genre insert
dbg!(&company_id); // line 212 - after every company insert
```

**Example of failure:** A user with 500 games triggers a refresh. This produces 500 game IDs + thousands of genre/company IDs written to stderr. On systems where stderr is logged (journald, systemd), this creates log noise and wastes disk space.

**Fix:** Remove all `dbg!()` calls, or replace with proper `tracing`/`log` crate logging at debug level.

---

## Rust Syntax & Idiom Issues

### 9. Explicit `return` at end of functions

**Files:** `src-tauri/src/igdb.rs:81`, `src-tauri/src/twitch.rs:19`, `src-tauri/src/commands/game.rs:57,64`

Rust idiom is to use the last expression as the implicit return value. Explicit `return` at the end of a function is considered non-idiomatic.

```rust
// Non-idiomatic
pub fn new(twitch_client: TwitchApiClient) -> Self {
    return IgdbApiClient { ... };
}

// Idiomatic
pub fn new(twitch_client: TwitchApiClient) -> Self {
    IgdbApiClient { ... }
}
```

Also in `trigrams()` and `similarity()`:
```rust
return hashset;        // should be just: hashset
return tri_a.intersection(&tri_b).count() as f64 / tri_a.len() as f64;
```

---

### 10. `trigrams` and `similarity` take `String` by value instead of `&str`

**File:** `src-tauri/src/commands/game.rs:49,60`

These functions take ownership of `String` values, forcing unnecessary clones at every call site. They only need to read the string.

```rust
// Current: forces caller to clone
pub fn trigrams(s: String) -> HashSet<String> { ... }
pub fn similarity(a: String, b: String) -> f64 { ... }

// In get_games, this forces cloning on every iteration:
similarity(
    name.clone().to_ascii_lowercase(),     // clone + allocate
    game.name.clone().to_ascii_lowercase(), // clone + allocate
)
```

**Fix:**

```rust
pub fn trigrams(s: &str) -> HashSet<String> { ... }
pub fn similarity(a: &str, b: &str) -> f64 { ... }
```

---

### 11. `Ok(Self { pool: pool })` - redundant field name

**File:** `src-tauri/src/db.rs:28`

When a variable has the same name as the struct field, Rust allows shorthand initialization.

```rust
// Current
Ok(Self { pool: pool })

// Idiomatic
Ok(Self { pool })
```

---

### 12. `use core::panic` import is unused

**File:** `src-tauri/src/lib.rs:1`

`use core::panic;` is imported but never used. `panic!` is a built-in macro that doesn't need this import.

---

### 13. `env::home_dir()` is deprecated since Rust 1.29

**File:** `src-tauri/src/steam.rs:72`

`std::env::home_dir()` has been deprecated because it behaves incorrectly on Windows. It can return the wrong directory.

```rust
let mut user_dir = match env::home_dir() { ... };
```

**Fix:** Use the `dirs` crate or `tauri`'s path resolver:

```rust
let home = dirs::home_dir().ok_or("unable to get home dir")?;
```

---

### 14. `SteamClient` is an empty struct used only as a method namespace

**File:** `src-tauri/src/steam.rs:62`

`SteamClient` has zero fields but all methods take `&self`. It's essentially used as a namespace.

```rust
pub struct SteamClient {}

impl SteamClient {
    pub fn new() -> Self { SteamClient {} }
    pub fn is_steam_game_install(&self, game_id: String) -> bool { ... }
}
```

This is not wrong, but it means every method receives a useless `&self` parameter. Either add state to justify the struct (e.g., a cached steam directory path), or use free functions in a `steam` module.

---

### 15. Loading ALL environment variables into Tauri managed state

**File:** `src-tauri/src/lib.rs:35-41`

The code iterates over every environment variable on the system and puts them all in a `HashMap<String, String>` managed by Tauri. This exposes sensitive system variables (like `PATH`, `HOME`, `SSH_AUTH_SOCK`, etc.) to any code that accesses this state.

```rust
let mut config = HashMap::new();
for (key, val) in env::vars() {
    config.insert(key, val);
}
app.manage::<HashMap<String, String>>(config);
```

**Example of failure:** If you later add a Tauri command that accidentally exposes this state to the frontend, all system environment variables become accessible to the WebView (including potential secrets).

**Fix:** Only load the specific variables you need:

```rust
struct AppConfig {
    steam_api_key: String,
    steam_profile_id: String,
    twitch_client_id: String,
    twitch_client_secret: String,
}
```

---

## Code Duplication

### 16. SQL query + row mapping duplicated between `get_games` and `get_game_by_id`

**File:** `src-tauri/src/db/game.rs:23-79` vs `81-139`

These two functions share identical SQL (minus a `WHERE` clause) and identical row mapping code (~30 lines each). Any schema change requires updating both in sync.

**Example of failure:** You add a new column `rating` to the `games` table. You update the SQL in `get_games` but forget to update `get_game_by_id`. Now the game detail page is missing the rating while the list shows it.

**Fix:** Extract the row mapping into a shared function and build the query dynamically or use a base query.

---

### 17. `install_game` and `uninstall_game` duplicate store_id lookup

**File:** `src-tauri/src/commands/game.rs:128-174`

Both commands contain an identical `sqlx::query_scalar` to fetch `store_id` from `games_store`. This is 8 lines of duplicated code.

**Fix:** Extract into a shared function:

```rust
async fn get_store_id(pool: &Pool<Sqlite>, game_id: i64) -> Result<String, String> { ... }
```

---

### 18. Publisher/developer extraction logic duplicated in IGDB client

**File:** `src-tauri/src/igdb.rs:103-118` vs `158-173`

The `get_game` and `get_games` methods both contain identical loops extracting publishers and developers from `involved_companies`. This logic is repeated verbatim.

**Fix:** Extract into a method:

```rust
fn extract_companies(involved: &[IgdbInvolvedCompany], game_id: u64) -> (Vec<IgdbCompany>, Vec<IgdbCompany>) { ... }
```

---

### 19. IGDB image base URL hardcoded in multiple Vue components

**Files:** `src/pages/games/[id].vue:111` and `src/components/app-sidebar/game-sidebar-item/GameSidebarItem.vue:21`

The IGDB image URL pattern is written as raw string literals in two separate components:

```typescript
// [id].vue
`https://images.igdb.com/igdb/image/upload/t_1080p/${game.value.artworks[0]}.jpg`

// GameSidebarItem.vue
`https://images.igdb.com/igdb/image/upload/t_cover_big/${props.game.cover}.jpg`
```

**Example of failure:** IGDB changes their CDN domain. You update one component but forget the other. Half the images break.

**Fix:** Create a utility function:

```typescript
export function igdbImageUrl(imageId: string, size: string = 't_1080p'): string {
    return `https://images.igdb.com/igdb/image/upload/${size}/${imageId}.jpg`
}
```

---

## Frontend Issues

### 20. `String` (uppercase) instead of `string` in TypeScript type

**File:** `src/types/game.ts:3`

```typescript
export type GameInfo = {
    id: number,
    name: String,  // <-- should be lowercase 'string'
}
```

`String` is the JavaScript wrapper object type. `string` is the primitive type. They behave differently:

```typescript
const a: String = new String("hello") // wrapper object
const b: string = "hello"             // primitive

a === b // false! Different types
typeof a // "object"
typeof b // "string"
```

**Example of failure:** A function expecting `string` receives a `String` wrapper. Strict equality checks fail unexpectedly. TypeScript may also not warn about certain invalid operations.

**Fix:** Use lowercase `string`.

---

### 21. `watchEffect` with async callback in game detail page

**File:** `src/pages/games/[id].vue:95-102`

`watchEffect` does not properly handle async callbacks. The returned Promise is ignored, meaning errors are silently swallowed and cleanup/disposal doesn't wait for the async work to complete.

```typescript
watchEffect(async () => {
    game.value = await getGameById(id.value);
    // ...
})
```

**Example of failure:** User navigates rapidly between games. Multiple `getGameById` calls fire concurrently. The responses may return out of order, causing the UI to display data for a different game than the one selected (race condition).

**Fix:** Use `watch` with explicit source and an `AbortController` or a guard:

```typescript
watch(id, async (newId) => {
    game.value = await getGameById(newId);
    // ...
}, { immediate: true })
```

---

### 22. Missing `:key` on `v-for` loops for genres and developers

**File:** `src/pages/games/[id].vue:16,29`

```html
<span v-for="studio in game.developers" ...>{{ studio }}</span>
<span v-for="genre in game.genres" ...>{{ genre }}</span>
```

Vue's `v-for` without `:key` uses a "in-place patch" strategy. This can cause rendering bugs when the list changes (wrong item updated, stale DOM state, broken transitions).

**Example of failure:** If genres are reordered (e.g., after a data update), Vue may reuse DOM nodes incorrectly, causing visual glitches or keeping stale attribute bindings from a previous item.

**Fix:**

```html
<span v-for="(studio, index) in game.developers" :key="index">
```

---

### 23. No error handling or loading states on the frontend

**Files:** `src/pages/games.vue`, `src/pages/games/[id].vue`, `src/stores/game.store.ts`

All `invoke` calls are `await`ed with no `try/catch`, no loading indicator, and no error feedback to the user. If any backend call fails, the UI silently stays empty or in a broken state.

```typescript
// games.vue - no try/catch, no loading state
onMounted(async () => {
    let res: GameInfo[] = await getGames()
    if (!res.length) {
        await refreshGames() // can throw silently
        res = await getGames();
    }
    games.value = res
})
```

**Example of failure:** The IGDB API is down. `refreshGames()` throws an error. The promise rejects. `games.value` is never set. The sidebar is empty with no indication of what went wrong. The user thinks the app is broken.

**Fix:** Add try/catch blocks with error state refs and display error messages in the UI. Add loading states while fetching.

---

### 24. `games.vue` is a layout page that also handles data fetching

**File:** `src/pages/games.vue`

This page component acts as both:
1. A **layout** (sidebar + main content area)
2. A **data fetcher** (loads games on mount, triggers refresh)

This violates single responsibility and makes it harder to reuse the layout or test the data logic independently.

**Fix:** Extract data fetching into the Pinia store's initialization, or create a composable. Keep the page component focused on layout.

---

## Architecture & Design Issues

### 25. Filtering done in Rust memory instead of SQL

**File:** `src-tauri/src/commands/game.rs:24-44`

The `get_games` command fetches **all** games from the database, then filters them in Rust using trigrams. This loads the entire games table (with all joins) into memory on every search keystroke.

```rust
let mut games = GameRepository::get_games(&db_state.pool).await?; // loads ALL games
if let Some(name) = query.and_then(|q| q.name) {
    games = games.into_iter().filter(|game| { ... }).collect(); // filters in memory
}
```

**Example of failure:** A user with 2000+ Steam games types in the search box. Every keystroke loads all 2000 games with their genres, developers, covers, and artworks from SQLite, deserializes them, then filters. This causes noticeable UI lag.

**Fix:** SQLite supports the `LIKE` operator and you could use `FTS5` for full-text search. At minimum, push the `LIKE` filter to SQL and only use trigrams as a fallback.

---

### 26. IGDB API has a 500 result limit - no pagination

**File:** `src-tauri/src/igdb.rs:202`

The IGDB external_games query sets `limit` to `game_ids.len()`, but IGDB's API has a maximum limit of 500 per request.

```rust
let query = format!(
    "fields *;  where external_game_source = 1 & uid = ({}); limit {};",
    steam_urls.join(","),
    game_ids.len() // Could be 1000+
);
```

**Example of failure:** A user with 800 Steam games triggers a refresh. The query asks for `limit 800` but IGDB caps at 500. Only 500 games are returned. 300 games silently vanish from the library.

**Fix:** Paginate requests in batches of 500:

```rust
for chunk in game_ids.chunks(500) {
    // query each chunk separately and aggregate results
}
```

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

**Example of failure:** A macOS or Windows user compiles and runs the app. `is_steam_game_install` always returns `false` because the path doesn't exist. The install/uninstall buttons show incorrect state.

**Fix:** Detect the OS and use the appropriate path:
- Linux: `~/.local/share/Steam/steamapps`
- macOS: `~/Library/Application Support/Steam/steamapps`
- Windows: `C:\Program Files (x86)\Steam\steamapps`

---

### 28. `TwitchApiClient` clones access token excessively

**File:** `src-tauri/src/twitch.rs:44-50`

The access token is cloned 3 times in `refresh_access_token`: once to set the field, once to return, and once in `get_access_token`. Since tokens are short strings this isn't a performance issue, but it indicates the API could be cleaner.

```rust
self.access_token = Some(parsed.access_token.clone()); // clone 1
Ok(parsed.access_token.clone()) // clone 2 (could just use parsed.access_token)
```

**Fix:** Store the token, then return a reference or clone from the stored value:

```rust
self.access_token = Some(parsed.access_token);
Ok(self.access_token.clone().unwrap())
```

---

### 29. Inconsistent naming conventions

**Files:** Multiple

- `connexion` (French) in `db.rs:21` - rest of codebase is English
- `is_steam_game_install` in `steam.rs:118` - should be `is_steam_game_installed` (adjective, not verb)
- `store_id` in the database refers to a Steam app ID but the name is generic
- `studios` alias in SQL queries while the table is now `companies`

These inconsistencies make the code harder to navigate and understand at a glance.

---

### 30. `GameRepository` is a stateless empty struct

**File:** `src-tauri/src/db/game.rs:20-21`

```rust
pub struct GameRepository {}
```

All methods are `pub async fn method(pool: &Pool<Sqlite>, ...)` - they take the pool as an argument rather than from `self`. The struct exists purely as a namespace for static methods.

**Fix:** Either give it state (hold a reference to the pool) or just use free functions in the `game` module:

```rust
// Option A: Stateful
pub struct GameRepository<'a> {
    pool: &'a Pool<Sqlite>,
}

// Option B: Just functions
pub async fn get_games(pool: &Pool<Sqlite>) -> Result<Vec<Game>, sqlx::Error> { ... }
```

---

## Minor / Style Issues

### 31. Inconsistent whitespace and formatting

**Files:** `src-tauri/src/lib.rs`

Mixed indentation depth, double blank lines, and inconsistent spacing around parentheses:

```rust
             dotenvy::dotenv().ok();   // extra indentation (line 33)

            let mut config = HashMap::new();  // different indentation

tauri::async_runtime::block_on(  async {  // space before async
```

**Fix:** Run `cargo fmt` consistently.

---

### 32. Empty `<style scoped></style>` tags in Vue components

**Files:** `src/pages/games.vue:35`, `src/pages/games/[id].vue:134`, `src/components/app-sidebar/AppSidebar.vue:33`, `src/components/app-sidebar/game-sidebar-item/GameSidebarItem.vue:26`

Every component has an empty scoped style block. This adds noise and no value.

---

### 33. `futures` crate in Cargo.toml appears unused

**File:** `src-tauri/Cargo.toml:26`

```toml
futures = "0.3.31"
```

No `use futures::` import appears in any Rust source file. This adds unnecessary compile time.

---

### 34. Migration history contains schema churn

**Files:** `src-tauri/migrations/`

The migrations contain back-and-forth changes:
- Migration 5 adds `release_date` as `text`
- Migration 6 drops it and re-adds as `integer`
- Migration 7 creates `games_genres`
- Migration 12 renames it to `belongs_to`
- Migration 9 creates `studios`
- Migration 11 renames it to `companies`

While this is fine during active development, before a release these should be squashed into a clean initial schema. Running 12 migrations on first install (including drops and renames) is slower and harder to audit than a single clean migration.

---

## Summary by Priority

| Priority | Issue | Impact |
|----------|-------|--------|
| Critical | Transaction bug (#1) | Orphaned data in database |
| Critical | Trigram panic on UTF-8 (#2) | App crash on search |
| Critical | INNER JOIN drops games (#3) | Games silently missing |
| Critical | No migration runner (#5) | App unusable on fresh install |
| High | Destructive refresh (#4) | Data loss on API failure |
| High | IGDB 500 limit (#26) | Games silently missing |
| High | No error handling frontend (#23) | Silent failures, blank UI |
| High | All env vars in state (#15) | Security risk |
| Medium | `panic!` on missing config (#6) | Crash without explanation |
| Medium | No error types (#7) | Poor error diagnostics |
| Medium | watchEffect async race (#21) | Wrong game displayed |
| Medium | Filtering in memory (#25) | Performance on large libraries |
| Medium | Duplicated code (#16-19) | Maintenance burden |
| Low | Idiomatic Rust (#9-14) | Code readability |
| Low | TypeScript `String` (#20) | Type safety |
| Low | Style issues (#31-34) | Code cleanliness |
