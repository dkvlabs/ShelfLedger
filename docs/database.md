# Database

Status: migrations `0001_init` through `0004_listing_invariants`, 27.10.2026.
SQLite uses WAL, foreign keys and integer money. Existing migration SQL is never
rewritten; the embedded migration list preserves its checksums.

## Logical records

| Tables | Responsibility |
| --- | --- |
| `works`, `authors`, `work_authors` | Literary identity, ordered authors, description and original language |
| `editions` | Work-linked publication, ISBN-10/13, publisher, language, pages and binding |
| `copies`, `acquisitions`, `locations` | Physical inventory, ownership, location hierarchy, condition and provenance |
| `tags`, `work_tags` | Editable categorization and searchable tags |
| `reading_sessions`, `reading_progress_events` | Re-reads, independent reading status, dates, rating, review and progress |
| `contacts`, `loans` | Local contacts, due dates, return and retained loan history |
| `valuation_records` | Source/date/confidence observations, never live prices |
| `sale_listings`, `disposals` | Listing lifecycle and retained sold/donated/traded/lost history |
| `settings`, `metadata_cache`, `activity_events`, `import_jobs` | Preferences, fetched metadata, activity and row import logs |
| `wishlist`, `attachments` | Reserved schema; dedicated workflows deferred |
| `works_fts` | Derived searchable title, subtitle, authors, ISBN, notes, locations and tags |

## Invariants

IDs for newly created records are UUID v4 generated in Rust. Work, edition and
copy identity never share a status column. Reading editions must belong to the
session's work. At most one active loan and one active sale listing exists per
copy. A loan must be returned before disposal. Disposal closes active listings;
withdrawing an old listing cannot make a disposed copy owned again.

Archiving preserves the record and history, removes the copy from active counts
and note/location search, and is reversible in the detail screen. The retained
history can be displayed explicitly. ISBN forms must pass checksums and identify
the same edition. ISBN-10 conversion supports an `X` check digit. Page counts are
positive. Publication precision can be YYYY, YYYY-MM or YYYY-MM-DD; other dates
are real ISO calendar dates or RFC3339 timestamps.

Money is nonnegative integer minor units with a supported ISO 4217 code. Shared
`src/lib/currencies.json` defines the accepted codes and frontend decimal scale.
Every cost, including shipping or tax without a purchase price, requires a
currency. Missing amounts stay unknown. Reports never mix currencies. The latest
valuation uses observed date, creation time, then SQLite row sequence to select
exactly one observation per copy, including same-day observations.

## Migrations and interchange

Migration 0004 repairs old terminal-copy listings, withdraws duplicate active
listings, and adds an active-listing unique index. Snapshot replacement/merge
runs in a transaction with deferred FK checking, semantic validation and a final
FTS rebuild. Replace rejects duplicate input rows; merge permits identical rows
and rejects differing conflicts. Invalid input rolls the transaction back.

The snapshot contains the 22 logical tables above (excluding derived FTS and
migration bookkeeping). Full reading, loan and finance fixture round trips are
covered in `src-tauri/src/audit_regressions.rs`. See [import-format.md](import-format.md).
