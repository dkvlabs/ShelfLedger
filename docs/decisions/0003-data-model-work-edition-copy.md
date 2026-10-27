# ADR 0003 — Data model: work / edition / copy separation; money in minor units

**Status:** Accepted (2026-10)

## Context

Collectors own multiple editions and multiple physical copies of the same literary work.
Goodreads-style single-table models cannot represent that, nor the separation between
reading status and ownership/loan/disposal state.

## Decision

1. Three core entities: `works` (literary identity) → `editions` (published versions,
   ISBNs) → `copies` (owned items with condition, location, acquisition, disposal).
2. Reading status will live in `reading_sessions` (Phase 2), never on copies; ownership
   status lives on `copies`; loans will be their own table with a partial unique index
   for "one active loan per copy" (Phase 2). A book can be *finished* and *lent* at once.
3. Money is stored as **integer minor units + ISO 4217 currency text** everywhere; no
   floats, no cross-currency sums, no invented exchange rates. Cost fields are nullable
   (gifts, unknown provenance) and "unknown" is displayed, never guessed.
4. IDs are UUIDv4; disposed copies are soft-archived, never deleted, so history and
   financial reporting stay intact.

## Consequences

- Every book entry flows work → edition → copy, which shaped the Add Book dialog.
- Reports must group by currency (Phase 3); unpriced books show unknown margins.
- FTS5 index is maintained by repository code because the entities span multiple tables.
