-- Image bytes live in SQLite so JSON/ZIP transfer is complete and atomic.
-- cover_path is a logical identifier, never a filesystem path or remote URL.
CREATE TABLE edition_covers (
    id TEXT PRIMARY KEY,
    edition_id TEXT NOT NULL UNIQUE REFERENCES editions(id) ON DELETE CASCADE,
    data_url TEXT NOT NULL,
    source TEXT NOT NULL CHECK(source IN ('photo', 'openlibrary')),
    created_at TEXT NOT NULL
);
