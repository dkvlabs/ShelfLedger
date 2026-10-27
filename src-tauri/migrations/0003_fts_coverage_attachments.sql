-- ShelfLedger 0003: full FTS coverage per plan §8 (title, author names, ISBN
-- aliases, notes, tags, locations) and the `attachments` table from the plan's
-- core-table list. FTS5 columns cannot be ALTERed, so the derived index is
-- rebuilt from source data.

CREATE VIRTUAL TABLE works_fts_new USING fts5(
    work_id UNINDEXED,
    title,
    subtitle,
    author_names,
    isbns,
    notes,
    location_names,
    tag_names
);

INSERT INTO works_fts_new (work_id, title, subtitle, author_names, isbns, notes, location_names, tag_names)
SELECT
    w.id,
    w.title,
    coalesce(w.subtitle, ''),
    coalesce((
        SELECT group_concat(a.name, ' ') FROM work_authors wa
        JOIN authors a ON a.id = wa.author_id WHERE wa.work_id = w.id
    ), ''),
    coalesce((
        SELECT group_concat(e.isbn13 || ' ' || coalesce(e.isbn10, ''), ' ')
        FROM editions e WHERE e.work_id = w.id
    ), ''),
    coalesce((
        SELECT group_concat(coalesce(c.note, ''), ' ')
        FROM copies c JOIN editions e2 ON c.edition_id = e2.id
        WHERE e2.work_id = w.id
    ), ''),
    coalesce((
        SELECT group_concat(coalesce(l.name, ''), ' ')
        FROM copies c2 JOIN editions e3 ON c2.edition_id = e3.id
        LEFT JOIN locations l ON l.id = c2.location_id
        WHERE e3.work_id = w.id
    ), ''),
    coalesce((
        SELECT group_concat(t.name, ' ') FROM work_tags wt
        JOIN tags t ON t.id = wt.tag_id WHERE wt.work_id = w.id
    ), '')
FROM works w;

DROP TABLE works_fts;
ALTER TABLE works_fts_new RENAME TO works_fts;

-- Attachment metadata (covers, receipts, condition photos). Media paths are
-- managed by Rust inside the app-data directory; nothing here is served.
CREATE TABLE attachments (
    id            TEXT PRIMARY KEY,
    owner_type    TEXT NOT NULL CHECK (owner_type IN ('work', 'edition', 'copy', 'acquisition')),
    owner_id      TEXT NOT NULL,
    relative_path TEXT NOT NULL,
    mime_type     TEXT NOT NULL,
    checksum      TEXT,
    created_at    TEXT NOT NULL
);

CREATE INDEX idx_attachments_owner ON attachments (owner_type, owner_id);
