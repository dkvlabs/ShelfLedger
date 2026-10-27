-- ShelfLedger 0001_init: Phase 1 core catalog & inventory schema.
--
-- Conventions (docs/database.md):
-- - IDs are UUIDv4 strings for portability.
-- - Money is INTEGER minor units + ISO 4217 currency TEXT. Never floats.
-- - Dates/timestamps are ISO 8601 TEXT (UTC).
-- - A work (literary identity) has editions (published versions) which have
--   copies (owned physical items). Reading status lives in reading_sessions
--   (Phase 2); ownership/loan/disposal state lives on copies and related
--   tables — the two concepts are never merged into one column.

CREATE TABLE settings (
    key        TEXT PRIMARY KEY,
    value_json TEXT NOT NULL
);

CREATE TABLE authors (
    id         TEXT PRIMARY KEY,
    name       TEXT NOT NULL,
    sort_name  TEXT,
    created_at TEXT NOT NULL
);

CREATE TABLE works (
    id                TEXT PRIMARY KEY,
    title             TEXT NOT NULL,
    subtitle          TEXT,
    original_language TEXT,
    description       TEXT,
    created_at        TEXT NOT NULL,
    updated_at        TEXT NOT NULL
);

CREATE TABLE work_authors (
    work_id    TEXT NOT NULL REFERENCES works(id) ON DELETE CASCADE,
    author_id  TEXT NOT NULL REFERENCES authors(id) ON DELETE CASCADE,
    role       TEXT NOT NULL DEFAULT 'author',
    sort_order INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY (work_id, author_id)
);

CREATE TABLE editions (
    id                TEXT PRIMARY KEY,
    work_id           TEXT NOT NULL REFERENCES works(id) ON DELETE CASCADE,
    isbn10            TEXT,
    isbn13            TEXT,
    publisher         TEXT,
    published_at      TEXT,
    language          TEXT,
    page_count        INTEGER,
    binding           TEXT,
    cover_path        TEXT,
    provider_ids_json TEXT,
    created_at        TEXT NOT NULL
);

-- ISBN uniqueness applies only to non-null values; real-world duplicate or
-- wrong ISBNs are handled via manual override (docs/database.md).
CREATE UNIQUE INDEX idx_editions_isbn13 ON editions (isbn13) WHERE isbn13 IS NOT NULL AND isbn13 <> '';
CREATE UNIQUE INDEX idx_editions_isbn10 ON editions (isbn10) WHERE isbn10 IS NOT NULL AND isbn10 <> '';
CREATE INDEX idx_editions_work ON editions (work_id);

CREATE TABLE locations (
    id         TEXT PRIMARY KEY,
    parent_id  TEXT REFERENCES locations(id) ON DELETE RESTRICT,
    name       TEXT NOT NULL,
    type       TEXT NOT NULL CHECK (type IN ('room', 'bookcase', 'shelf', 'box')),
    sort_order INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL
);

CREATE TABLE copies (
    id               TEXT PRIMARY KEY,
    edition_id       TEXT NOT NULL REFERENCES editions(id) ON DELETE RESTRICT,
    inventory_code   TEXT,
    location_id      TEXT REFERENCES locations(id) ON DELETE SET NULL,
    condition_grade  TEXT NOT NULL DEFAULT 'good'
                     CHECK (condition_grade IN ('new', 'like_new', 'very_good', 'good', 'acceptable', 'poor')),
    ownership_status TEXT NOT NULL DEFAULT 'owned'
                     CHECK (ownership_status IN ('owned', 'lent', 'listed', 'sold', 'donated', 'traded', 'lost')),
    acquisition_type TEXT CHECK (acquisition_type IS NULL OR acquisition_type IN ('bought', 'gift', 'trade', 'inherited')),
    acquired_at      TEXT,
    note             TEXT,
    sentimental_score INTEGER CHECK (sentimental_score IS NULL OR (sentimental_score BETWEEN 0 AND 5)),
    archived_at      TEXT,
    created_at       TEXT NOT NULL,
    updated_at       TEXT NOT NULL
);

CREATE INDEX idx_copies_edition ON copies (edition_id);
CREATE INDEX idx_copies_location ON copies (location_id);
CREATE INDEX idx_copies_status ON copies (ownership_status);

-- One acquisition record per copy for v1; provenance expansion comes later
-- (docs/database.md).
CREATE TABLE acquisitions (
    id               TEXT PRIMARY KEY,
    copy_id          TEXT NOT NULL UNIQUE REFERENCES copies(id) ON DELETE CASCADE,
    acquired_at      TEXT,
    price_minor      INTEGER CHECK (price_minor IS NULL OR price_minor >= 0),
    currency         TEXT,
    shipping_minor   INTEGER CHECK (shipping_minor IS NULL OR shipping_minor >= 0),
    tax_minor        INTEGER CHECK (tax_minor IS NULL OR tax_minor >= 0),
    store            TEXT,
    acquisition_type TEXT CHECK (acquisition_type IS NULL OR acquisition_type IN ('bought', 'gift', 'trade', 'inherited')),
    receipt_path     TEXT,
    created_at       TEXT NOT NULL
);

CREATE TABLE activity_events (
    id           TEXT PRIMARY KEY,
    entity_type  TEXT NOT NULL,
    entity_id    TEXT NOT NULL,
    action       TEXT NOT NULL,
    occurred_at  TEXT NOT NULL,
    payload_json TEXT
);

-- Phase 1 stubs so later migrations can be strictly additive.
CREATE TABLE metadata_cache (
    cache_key    TEXT PRIMARY KEY,
    payload_json TEXT NOT NULL,
    fetched_at   TEXT NOT NULL
);

CREATE TABLE tags (
    id   TEXT PRIMARY KEY,
    name TEXT NOT NULL UNIQUE
);

CREATE TABLE work_tags (
    work_id TEXT NOT NULL REFERENCES works(id) ON DELETE CASCADE,
    tag_id  TEXT NOT NULL REFERENCES tags(id) ON DELETE CASCADE,
    PRIMARY KEY (work_id, tag_id)
);

-- Full-text search over works. Managed by application code (delete + insert on
-- work/author/edition writes); standalone so it can also index copy notes.
CREATE VIRTUAL TABLE works_fts USING fts5(
    work_id UNINDEXED,
    title,
    subtitle,
    author_names,
    isbns
);
