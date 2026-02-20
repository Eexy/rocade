-- Migration to recreate all tables with CASCADE constraints while preserving data
-- Step 1: Create new tables with CASCADE constraints

CREATE TABLE games_new (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL,
    summary TEXT,
    release_date INTEGER
);

CREATE TABLE genres_new (
    id INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL,
    name TEXT NOT NULL
);

CREATE TABLE companies_new (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    igdb_id INTEGER UNIQUE NOT NULL,
    name TEXT NOT NULL
);

CREATE TABLE covers_new (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    game_id INTEGER NOT NULL,
    cover_id TEXT NOT NULL,
    FOREIGN KEY (game_id) REFERENCES games_new(id) ON DELETE CASCADE ON UPDATE CASCADE
);

CREATE TABLE artworks_new (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    game_id INTEGER NOT NULL,
    artwork_id TEXT NOT NULL,
    FOREIGN KEY (game_id) REFERENCES games_new(id) ON DELETE CASCADE ON UPDATE CASCADE
);

CREATE TABLE games_store_new (
    id INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL,
    game_id INTEGER NOT NULL,
    store_id TEXT NOT NULL,
    FOREIGN KEY (game_id) REFERENCES games_new(id) ON DELETE CASCADE ON UPDATE CASCADE
);

CREATE TABLE belongs_to_new (
    id INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL,
    game_id INTEGER NOT NULL,
    genre_id INTEGER NOT NULL,
    FOREIGN KEY (game_id) REFERENCES games_new(id) ON DELETE CASCADE ON UPDATE CASCADE,
    FOREIGN KEY (genre_id) REFERENCES genres_new(id) ON DELETE CASCADE ON UPDATE CASCADE
);

CREATE TABLE developed_by_new (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    game_id INTEGER NOT NULL,
    studio_id INTEGER NOT NULL,
    FOREIGN KEY (game_id) REFERENCES games_new(id) ON DELETE CASCADE ON UPDATE CASCADE,
    FOREIGN KEY (studio_id) REFERENCES companies_new(id) ON DELETE CASCADE ON UPDATE CASCADE
);

-- Step 2: Copy data from old tables to new tables (parent tables first)

INSERT INTO games_new (id, name, summary, release_date)
SELECT id, name, summary, release_date FROM games;

INSERT INTO genres_new (id, name)
SELECT id, name FROM genres;

INSERT INTO companies_new (id, igdb_id, name)
SELECT id, igdb_id, name FROM companies;

-- Copy data from child tables
INSERT INTO covers_new (id, game_id, cover_id)
SELECT id, game_id, cover_id FROM covers;

INSERT INTO artworks_new (id, game_id, artwork_id)
SELECT id, game_id, artwork_id FROM artworks;

INSERT INTO games_store_new (id, game_id, store_id)
SELECT id, game_id, store_id FROM games_store;

INSERT INTO belongs_to_new (id, game_id, genre_id)
SELECT id, game_id, genre_id FROM belongs_to;

INSERT INTO developed_by_new (id, game_id, studio_id)
SELECT id, game_id, studio_id FROM developed_by;

-- Step 3: Drop old tables (child tables first to avoid foreign key issues)

DROP TABLE developed_by;
DROP TABLE belongs_to;
DROP TABLE games_store;
DROP TABLE artworks;
DROP TABLE covers;
DROP TABLE companies;
DROP TABLE genres;
DROP TABLE games;

-- Step 4: Rename new tables to original names

ALTER TABLE games_new RENAME TO games;
ALTER TABLE genres_new RENAME TO genres;
ALTER TABLE companies_new RENAME TO companies;
ALTER TABLE covers_new RENAME TO covers;
ALTER TABLE artworks_new RENAME TO artworks;
ALTER TABLE games_store_new RENAME TO games_store;
ALTER TABLE belongs_to_new RENAME TO belongs_to;
ALTER TABLE developed_by_new RENAME TO developed_by;

-- Step 5: Recreate the unique index on genres
CREATE UNIQUE INDEX unique_genre_name ON genres(name);
