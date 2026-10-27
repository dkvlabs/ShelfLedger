-- ShelfLedger 0002: Phase 2 (reading + loans) and Phase 3 (resale + money).
-- Strictly additive to 0001_init (docs/database.md).

-- Reading: re-reads are separate sessions; a book can be finished and lent
-- simultaneously because reading state never lives on copies.
CREATE TABLE reading_sessions (
    id           TEXT PRIMARY KEY,
    work_id      TEXT NOT NULL REFERENCES works(id) ON DELETE CASCADE,
    edition_id   TEXT REFERENCES editions(id) ON DELETE SET NULL,
    status       TEXT NOT NULL CHECK (status IN ('want_to_read', 'reading', 'finished', 'paused', 'dnf')),
    started_at   TEXT,
    finished_at  TEXT,
    rating       INTEGER CHECK (rating IS NULL OR (rating BETWEEN 0 AND 5)),
    review       TEXT,
    created_at   TEXT NOT NULL,
    updated_at   TEXT NOT NULL
);

CREATE INDEX idx_reading_sessions_work ON reading_sessions (work_id);
CREATE INDEX idx_reading_sessions_status ON reading_sessions (status);

CREATE TABLE reading_progress_events (
    id                 TEXT PRIMARY KEY,
    reading_session_id TEXT NOT NULL REFERENCES reading_sessions(id) ON DELETE CASCADE,
    occurred_at        TEXT NOT NULL,
    page               INTEGER CHECK (page IS NULL OR page >= 0),
    percent            INTEGER CHECK (percent IS NULL OR (percent BETWEEN 0 AND 100)),
    note               TEXT
);

CREATE INDEX idx_reading_progress_session ON reading_progress_events (reading_session_id);

-- Loans: one active loan per copy, enforced by the partial unique index below
-- AND re-checked in the domain layer for a friendly error message.
CREATE TABLE contacts (
    id           TEXT PRIMARY KEY,
    display_name TEXT NOT NULL,
    contact_info TEXT,
    notes        TEXT,
    created_at   TEXT NOT NULL
);

CREATE TABLE loans (
    id          TEXT PRIMARY KEY,
    copy_id     TEXT NOT NULL REFERENCES copies(id) ON DELETE RESTRICT,
    contact_id  TEXT NOT NULL REFERENCES contacts(id) ON DELETE RESTRICT,
    lent_at     TEXT NOT NULL,
    due_at      TEXT,
    returned_at TEXT,
    notes       TEXT,
    created_at  TEXT NOT NULL,
    updated_at  TEXT NOT NULL
);

CREATE UNIQUE INDEX idx_loans_active_per_copy ON loans (copy_id) WHERE returned_at IS NULL;
CREATE INDEX idx_loans_contact ON loans (contact_id);
CREATE INDEX idx_loans_active ON loans (returned_at) WHERE returned_at IS NULL;

-- Value is time-bound evidence, never a live quote (docs/database.md §9).
CREATE TABLE valuation_records (
    id          TEXT PRIMARY KEY,
    copy_id     TEXT NOT NULL REFERENCES copies(id) ON DELETE CASCADE,
    value_minor INTEGER NOT NULL CHECK (value_minor >= 0),
    currency    TEXT NOT NULL,
    source_type TEXT NOT NULL CHECK (source_type IN ('manual', 'comparable')),
    source_note TEXT,
    observed_at TEXT NOT NULL,
    confidence  TEXT NOT NULL CHECK (confidence IN ('low', 'medium', 'high')),
    created_at  TEXT NOT NULL
);

CREATE INDEX idx_valuations_copy ON valuation_records (copy_id, observed_at);

CREATE TABLE sale_listings (
    id                TEXT PRIMARY KEY,
    copy_id           TEXT NOT NULL REFERENCES copies(id) ON DELETE RESTRICT,
    platform          TEXT,
    listed_at         TEXT NOT NULL,
    asking_price_minor INTEGER NOT NULL CHECK (asking_price_minor >= 0),
    currency          TEXT NOT NULL,
    status            TEXT NOT NULL DEFAULT 'active' CHECK (status IN ('active', 'sold', 'withdrawn')),
    created_at        TEXT NOT NULL,
    updated_at        TEXT NOT NULL
);

CREATE INDEX idx_listings_copy ON sale_listings (copy_id);

-- Sold/donated/traded/lost history; never deletes the copy row.
CREATE TABLE disposals (
    id                TEXT PRIMARY KEY,
    copy_id           TEXT NOT NULL REFERENCES copies(id) ON DELETE RESTRICT,
    disposition       TEXT NOT NULL CHECK (disposition IN ('sold', 'donated', 'traded', 'lost')),
    date              TEXT NOT NULL,
    sale_price_minor  INTEGER CHECK (sale_price_minor IS NULL OR sale_price_minor >= 0),
    fee_minor         INTEGER CHECK (fee_minor IS NULL OR fee_minor >= 0),
    shipping_minor    INTEGER CHECK (shipping_minor IS NULL OR shipping_minor >= 0),
    currency          TEXT,
    buyer_alias       TEXT,
    notes             TEXT,
    created_at        TEXT NOT NULL
);

CREATE INDEX idx_disposals_copy ON disposals (copy_id);

-- Import audit (Phase 4).
CREATE TABLE import_jobs (
    id          TEXT PRIMARY KEY,
    source      TEXT NOT NULL,
    started_at  TEXT NOT NULL,
    finished_at TEXT,
    status      TEXT NOT NULL DEFAULT 'running' CHECK (status IN ('running', 'completed', 'failed')),
    stats_json  TEXT
);

-- Wishlist (P1 backlog): desired items are NOT owned copies.
CREATE TABLE wishlist (
    id                  TEXT PRIMARY KEY,
    work_id             TEXT NOT NULL REFERENCES works(id) ON DELETE CASCADE,
    edition_id          TEXT REFERENCES editions(id) ON DELETE SET NULL,
    maximum_price_minor INTEGER CHECK (maximum_price_minor IS NULL OR maximum_price_minor >= 0),
    currency            TEXT,
    priority            INTEGER NOT NULL DEFAULT 0,
    note                TEXT,
    created_at          TEXT NOT NULL
);

-- Manual sell-queue override (transparent decision assistant, user-controlled).
ALTER TABLE copies ADD COLUMN sell_review_override TEXT
    CHECK (sell_review_override IS NULL OR sell_review_override IN ('keep', 'sell', 'donate'));
