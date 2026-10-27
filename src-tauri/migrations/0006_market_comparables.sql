-- User-verified advertised prices, scoped to a published edition.
CREATE TABLE market_comparables (
    id TEXT PRIMARY KEY NOT NULL,
    edition_id TEXT NOT NULL REFERENCES editions(id) ON DELETE CASCADE,
    listing_url TEXT NOT NULL,
    asking_minor INTEGER NOT NULL CHECK(typeof(asking_minor) = 'integer' AND asking_minor BETWEEN 0 AND 9007199254740991),
    currency TEXT NOT NULL,
    condition_note TEXT,
    observed_at TEXT NOT NULL,
    included INTEGER NOT NULL DEFAULT 1 CHECK(included IN (0, 1)),
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    UNIQUE(edition_id, listing_url)
);
