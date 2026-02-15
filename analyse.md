# Rocade - Code Analysis

## Critical Issues

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

## Error Handling Issues

### 8. `expect()` calls that panic in setup

**File:** `src-tauri/src/igdb.rs:93,101`

Two `.expect()` calls remain in `IgdbApiClient::new()` which will crash the entire application if they fail:

```rust
headers.insert(
    "CLIENT-ID",
    HeaderValue::from_str(twitch_client.get_client_id().as_str())
        .expect("unable to set igdb client id"),  // panic if client_id has invalid chars
);

tauri_plugin_http::reqwest::Client::builder()
    .default_headers(headers)
    .build()
    .expect("unable to build igdb client"),       // panic if TLS backend fails
```

**Example of failure:** The Twitch client ID contains a non-ASCII character (common with copy-paste errors). `HeaderValue::from_str` fails. The user sees `thread 'main' panicked at 'unable to set igdb client id'` and the app closes.

**Hint:** Change `IgdbApiClient::new` to return `Result<Self, IgdbError>` and use `?` instead of `expect()`. The caller in `lib.rs` already handles `RocadeConfigError` and can be updated to handle the new error:
```rust
pub fn new(twitch_client: TwitchApiClient) -> Result<Self, IgdbError> {
    let header_val = HeaderValue::from_str(twitch_client.get_client_id().as_str())
        .map_err(|e| IgdbError::NoData(e.to_string()))?;
    // ...
}
```

---

### 10. No error handling or loading states on the frontend

**Files:** `src/stores/game.store.ts`, `src/pages/games/[id].vue`

All `invoke` calls are `await`ed with no `try/catch`, no loading indicator, and no error feedback.

```typescript
// game.store.ts - no try/catch
async function init() {
    let res: GameInfo[] = await getGames()
    if (!res.length) {
        await refreshGames()  // can throw silently
        res = await getGames();
    }
    games.value = res
}
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
for developer in game.developers.iter().flatten() {
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
for publisher in game.publishers.iter().flatten() {
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
pub async fn refresh_access_token(&mut self) -> Result<String, TwitchError> {
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

### 19. Twitch OAuth secrets passed as URL query parameters

**File:** `src-tauri/src/twitch.rs:41`

The Twitch client secret is still embedded directly in the URL query string:

```rust
// twitch.rs
let url = format!(
    "https://id.twitch.tv/oauth2/token?client_id={}&client_secret={}&grant_type=client_credentials",
    self.client_id, self.client_secret
);
```

If this URL is logged (by reqwest in debug mode, by a proxy, in error messages, or in stack traces), the secrets are exposed. *(The Steam API key was already fixed to use `.query()`.)*

**Hint:** For Twitch (`twitch.rs`), use `.form()` since it's a POST with `client_credentials`:
```rust
let res = self.client
    .post("https://id.twitch.tv/oauth2/token")
    .form(&[
        ("client_id", &self.client_id),
        ("client_secret", &self.client_secret),
        ("grant_type", &"client_credentials".to_string()),
    ])
    .send().await?;
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

### 24. Empty `<style scoped></style>` tags in Vue components

**Files:** `src/pages/games.vue`, `src/pages/games/[id].vue`, `src/components/app-sidebar/AppSidebar.vue`, `src/components/app-sidebar/game-sidebar-item/GameSidebarItem.vue`, `src/App.vue`

Every component has an empty scoped style block. This adds noise and no value.

**Hint:** Remove the `<style scoped></style>` tags from all five files: `games.vue`, `[id].vue`, `AppSidebar.vue`, `GameSidebarItem.vue`, and `App.vue`. They can always be added back when actual styles are needed.

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
Populate it with triggers on insert/update/delete. Then search with `SELECT * FROM games_fts WHERE games_fts MATCH ?`.

Also add a debounce (300ms) on the frontend `search` watcher in `game.store.ts` to avoid firing a query on every keystroke.

---

### 26. `similarity()` function is asymmetric

**File:** `src-tauri/src/commands/game.rs:76-81`

The similarity function divides by the length of `tri_a` (the search term), not by the union or maximum of both sets. This means `similarity("ab", "abcdefghij")` gives a very different result than `similarity("abcdefghij", "ab")`.

```rust
pub fn similarity(a: &str, b: &str) -> f64 {
    let tri_a = trigrams(a);
    let tri_b = trigrams(b);
    tri_a.intersection(&tri_b).count() as f64 / tri_a.len() as f64
    //                                           ^^^^^^^^^^
    //     always divides by first argument's trigram count
}
```

**Example of failure:** Searching for "cs" (short term) gives high similarity to "Counter-Strike" because most of "cs"'s trigrams are found. Short search terms have very few trigrams and the threshold of 0.4 is easy to pass, causing too many false positives.

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
You may need to lower the threshold from `0.4` to something like `0.2` or `0.3` since Jaccard similarity gives lower scores than the current asymmetric formula.

---

### 27. Steam path is Linux-only (no cross-platform support)

**File:** `src-tauri/src/lib.rs:71-75`

The Steam directory is hardcoded to the Linux path:

```rust
let steam_path = home_path
    .join(r".local")
    .join("share")
    .join("Steam")
    .join("steamapps");
```

This means `is_steam_game_installed`, `install_game`, and `uninstall_game` only work on Linux.

**Hint:** Use conditional compilation to build the Steam path per platform:
```rust
#[cfg(target_os = "linux")]
let steam_path = home_path.join(".local/share/Steam/steamapps");

#[cfg(target_os = "windows")]
let steam_path = PathBuf::from(r"C:\Program Files (x86)\Steam\steamapps");

#[cfg(target_os = "macos")]
let steam_path = home_path.join("Library/Application Support/Steam/steamapps");
```
For Windows, you may also want to check the registry (`HKCU\Software\Valve\Steam\SteamPath`) for non-default install locations.

---

### 28. `prepare_db` and `insert_games` take `State` wrapper instead of inner types

**File:** `src-tauri/src/commands/game.rs:105-117`

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
Then in `refresh_games`, call them as `prepare_db(&db_state)` and `insert_games(&game_repository, igdb_games)`.

---

### 29. `is_steam_game_installed` takes `String` by value instead of `&str`

**File:** `src-tauri/src/steam.rs:120`

```rust
pub fn is_steam_game_installed(&self, game_id: String) -> bool { ... }
```

The function only reads the string, so the caller is forced to give up ownership or clone unnecessarily. The call site in `commands/game.rs:131` already clones:
```rust
if let Some(store_id) = game.store_id.clone() {
    is_installed = steam_client.is_steam_game_installed(store_id);
}
```

Also, if the manifest file exists but lacks both `BytesToDownload` and `BytesDownloaded` fields (a fully installed game may not have them), `matches!` returns `false` — a fully installed game is reported as not installed.

**Hint:** Change the signature to take `&str` and fix the fallback:
```rust
pub fn is_steam_game_installed(&self, game_id: &str) -> bool { ... }

match (bytes_to_download, bytes_downloaded) {
    (Some(to_dl), Some(downloaded)) => to_dl == downloaded,
    _ => true, // manifest exists but no download fields = assume installed
}
```
Update the call site to pass `&store_id` instead of cloning.

---

### 30. No SQLite foreign key enforcement

**File:** `src-tauri/src/db.rs:18-21`

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
After enabling this, also add `ON DELETE CASCADE` to your foreign key constraints in a new migration so that deleting a game automatically cleans up its junction table rows.

---

### 31. Inconsistent naming: `studios` alias in SQL while table is `companies`

**File:** `src-tauri/src/db/game.rs:32,87`

```sql
json_group_array(distinct companies.name) as studios,
```

The table was renamed from `studios` to `companies` (migration 11), but the SQL alias still says `studios`. The Rust mapping code also uses `"studios"` as the column name (`row.get("studios")`). While technically functional, this is confusing.

**Hint:** Rename the SQL alias in `BASE_QUERY` from `studios` to `developers` (since the query joins through `developed_by`):
```sql
json_group_array(distinct companies.name) as developers,
```
Then update `map_game_row` to match:
```rust
let developers_json: Option<String> = row.get("developers");
```

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

**Hint:** Since the app hasn't been released yet (no users have existing databases to migrate), delete all 12 migration files in `src-tauri/migrations/` and create a single `0001_initial_schema.sql` that defines the final schema in one clean migration. Include all tables with their final column types and constraints.

---

## New Issues

### 33. `fs::create_dir_all` result is silently ignored

**File:** `src-tauri/src/db.rs:14`

```rust
fs::create_dir_all(&app_dir);  // Result<(), io::Error> is silently dropped
```

The return value of `fs::create_dir_all` is not handled. If the directory cannot be created (permissions issue, invalid path), no error is raised. The app proceeds to `SqlitePool::connect_with`, which will then fail with a confusing SQLite path error rather than a clear "could not create app directory" message.

**Hint:** Propagate the error using `?`:
```rust
fs::create_dir_all(&app_dir)
    .map_err(|e| RocadeConfigError::ConfigError(format!("unable to create app directory: {e}")))?;
```

---

### 34. `try_exists()` bool result is not checked

**File:** `src-tauri/src/lib.rs:77-81`

```rust
steam_path.try_exists().map_err(|_| {
    RocadeConfigError::ConfigError(
        "steam client directory doesn't not exist or is not found".to_string(),
    )
})?;
```

`try_exists()` returns `Result<bool>`. The `.map_err(...)?.` pattern only catches IO errors — it does not check whether the path actually exists. If the Steam directory simply doesn't exist, `try_exists()` returns `Ok(false)` and `?` succeeds silently. The app initializes with a non-existent path, and `is_steam_game_installed` silently returns `false` for every game.

**Hint:** Check the returned boolean explicitly:
```rust
let exists = steam_path.try_exists()
    .map_err(|e| RocadeConfigError::ConfigError(format!("unable to check steam directory: {e}")))?;

if !exists {
    return Err(RocadeConfigError::ConfigError(
        "steam directory not found; is Steam installed?".to_string(),
    ).into());
}
```

---

### 35. `storyline` is fetched from IGDB but never stored or returned

**Files:** `src-tauri/src/igdb.rs:49`, `src-tauri/src/db/game.rs`

`IgdbGameInfo.storyline` and `IgdbGame.storyline` are fetched from IGDB and modeled in Rust, but there is no `storyline` column in the `games` table (no migration adds it), no `storyline` field in the `Game` struct, and `insert_complete_game` does not bind it. The data is fetched, allocated, and silently dropped.

**Example of failure:** You add a "Storyline" section to the game detail page. It never shows data because it was never persisted, even though the IGDB response includes it.

**Hint:** Either add `storyline` to the `games` table via a new migration and persist it in `insert_complete_game`:
```sql
ALTER TABLE games ADD COLUMN storyline TEXT;
```
```rust
"insert into games (name, summary, storyline, release_date) values (?, ?, ?, ?)"
```
Then add the field to the `Game` struct and `map_game_row`. Or, if storyline is intentionally out of scope, remove the `storyline` field from `IgdbGameInfo` and `IgdbGame` to avoid the misleading dead code.

---

### 36. `watch` in game store fires API call on mount with empty search

**File:** `src/stores/game.store.ts:23-31`

```typescript
watch([games, search], async () => {
    if (games.value.length && !search.value.length) {
        filteredGames.value = games.value
        return
    }
    const res = await getGames({ name: search.value })
    filteredGames.value = res
}, { immediate: true })
```

On initial mount, both `games` and `search` are empty. The short-circuit condition `games.value.length && !search.value.length` evaluates to `false` (games is empty). The else branch fires and calls `getGames({ name: "" })` — an unnecessary API call before `init()` has populated the store. Additionally, every keystroke in the search box triggers a Tauri IPC call without debouncing.

**Hint:** Guard the watcher to only run when there is actual search input, and add a debounce:
```typescript
watch(search, useDebounceFn(async (query: string) => {
    if (!query.length) {
        filteredGames.value = games.value;
        return;
    }
    filteredGames.value = await getGames({ name: query });
}, 300));
```
Update `filteredGames` directly in `init()` after loading games, so the initial population doesn't depend on the watcher.

---

### 37. `extract_game_companies` checks full company game lists instead of role fields

**File:** `src-tauri/src/igdb.rs:132-158`

The function determines if a company is a publisher or developer by checking whether `game_id` appears in the company's `published`/`developed` arrays:

```rust
if let Some(published) = &involved.company.published {
    if published.contains(&game_id) {
        publishers.push(involved.company.clone());
    }
}
```

This is unreliable for two reasons:
1. IGDB's `involved_companies` objects have `publisher: bool` and `developer: bool` fields that directly indicate the role for this specific game — those are the correct fields to use.
2. The `published`/`developed` arrays on `IgdbCompany` represent the company's entire catalog across all games. These are separate relations that IGDB may not fully include in the `involved_companies.company.*` response, especially for large publishers.

**Example of failure:** A major publisher like EA has hundreds of games in their `published` list. IGDB may truncate or omit the array. The check `published.contains(&game_id)` fails. The game has no publisher in the database.

**Hint:** Add `developer` and `publisher` boolean fields to `IgdbInvolvedCompany` and update the IGDB query to include them:
```rust
pub struct IgdbInvolvedCompany {
    company: IgdbCompany,
    developer: bool,
    publisher: bool,
}
```
Update the query in `get_game_info` / `get_games_infos` to include these fields:
```
involved_companies.company.*, involved_companies.developer, involved_companies.publisher
```
Then simplify `extract_game_companies`:
```rust
for involved in companies.iter().flatten() {
    if involved.publisher { publishers.push(involved.company.clone()); }
    if involved.developer { developers.push(involved.company.clone()); }
}
```

---

## Summary by Priority

| Priority | Issue | Impact |
|----------|-------|--------|
| Critical | No migration runner (#3) | App unusable on fresh install |
| Critical | `fs::create_dir_all` ignored (#33) | Silent failure on setup |
| Critical | `try_exists()` bool not checked (#34) | Non-existent Steam path silently accepted |
| High | Destructive refresh (#2) | Data loss on API failure |
| High | IGDB 500 limit (#5) | Games silently missing |
| High | No frontend error handling (#10) | Silent failures, blank UI |
| High | Publishers never stored (#11) | Data silently dropped |
| High | `extract_game_companies` unreliable (#37) | Publishers/developers always empty |
| Medium | `expect()` panics in IgdbApiClient::new (#8) | App crash on bad config |
| Medium | watchEffect async race (#20) | Wrong game displayed |
| Medium | Filtering in memory (#25) | Performance on large libraries |
| Medium | Asymmetric similarity (#26) | Inconsistent search results |
| Medium | Twitch secrets in URL (#19) | Credential leakage risk |
| Medium | No FK enforcement (#30) | Data integrity risk |
| Medium | `storyline` never stored (#35) | Fetched data silently dropped |
| Medium | watch fires on empty search (#36) | Unnecessary API call on mount |
| Low | `String` by value in `is_steam_game_installed` (#29) | Unnecessary clone |
| Low | Unused `futures` crate (#17) | Unnecessary build time |
| Low | Excessive cloning in TwitchApiClient (#18) | Minor performance |
| Low | Empty style tags (#24) | Code noise |
| Low | Steam Linux-only (#27) | No cross-platform |
| Low | `State` in helpers (#28) | Testability |
| Low | `studios` alias (#31) | Confusing naming |
| Low | Migration churn (#32) | Maintenance burden |
