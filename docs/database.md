# Database

> Status: matches migration `0001_init` (Phase 1). Later migrations must be additive.

## Conventions

- **IDs**: UUID v4 text, generated in Rust — portable across machines.
- **Money**: `INTEGER` minor units + `currency` ISO 4217 text. Never floats. Never sum
  across currencies.
- **Dates/timestamps**: ISO 8601 text (UTC).
- **Pragmas** (set at connection time in `src-tauri/src/db.rs`): `foreign_keys = ON`,
  WAL journaling, `synchronous = NORMAL`.

## Tables (Phase 1)

| Table | Purpose |
| --- | --- |
| `works` | Literary identity: title, subtitle, original language, description |
| `authors` / `work_authors` | People and the many-to-many link (role, sort order) |
| `editions` | Published versions: ISBN-10/13, publisher, published date, language, page count, binding, cover path, provider IDs |
| `copies` | Owned physical items: edition ref, inventory code, location, condition grade, ownership status, acquisition type/date, note, sentimental score, `archived_at` (soft delete) |
| `acquisitions` | One per copy (v1): price, currency, shipping, tax, store, receipt path — all cost fields nullable (gifts, unknown provenance) |
| `locations` | Hierarchy `room > bookcase > shelf > box`; children RESTRICT parent deletion; deleting a location nulls copy references |
| `settings` | Key/value JSON preferences |
| `metadata_cache` | Provider lookup cache (Open Library) |
| `tags` / `work_tags` | Custom categorization (Phase 1 stub, used by UI later) |
| `activity_events` | Append-only user-readable history |
| `works_fts` | FTS5 index over title, subtitle, author names, ISBNs; maintained by repository code on writes |

## Key constraints

- `editions.isbn13` / `isbn10`: unique **partial** indexes (non-null values only).
  Manual editions without ISBNs are supported; real-world duplicate/wrong ISBNs are
  handled by conflict reporting, not destructive merges.
- `copies.ownership_status` CHECK: owned | lent | listed | sold | donated | traded | lost.
  Reading status is NOT stored here — it belongs to `reading_sessions` (Phase 2). A book
  can be *finished* and *lent* simultaneously; never merge the concepts.
- `copies.condition_grade` CHECK: new | like_new | very_good | good | acceptable | poor.
- `acquisitions.copy_id` UNIQUE: one acquisition record per copy in v1.
- Deleting an edition is RESTRICTed while copies reference it; disposing a copy keeps the
  row and history (soft delete via `archived_at`).

## Invariants enforced in Rust (domain layer)

- ISBN checksums are validated (and 10↔13 converted) before any edition write.
- Duplicate ISBN writes surface as `Conflict`, shown as "an edition with this ISBN
  already exists".
- A purchase price requires a currency, and vice versa.
- FTS rows are rebuilt after work, author, and edition changes.

## Search

`works_fts` (FTS5, standalone with `work_id UNINDEXED`) is rebuilt on writes. User queries
are tokenized into quoted prefix terms joined with `AND` (`fts_match_query`), which is
injection-safe for the FTS5 parser. Empty/invalid queries simply return no rows.

## Migrations

Embedded via `sqlx::migrate!("./migrations")` and applied on every launch. Before
destructive migrations (future), the app must take an automatic backup (Phase 4 work).
