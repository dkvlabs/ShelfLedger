# Import format (draft for Phase 4)

> Status: **specification draft** — not yet implemented. The Goodreads CSV import and
> generic CSV/JSON import arrive in Phase 4; this document locks the target behavior so
> the data layer can anticipate it.

## Goals

1. **Goodreads CSV import**: accept an exported CSV, map columns via preview, preserve
   ratings, dates, shelves, and reviews where present. No Goodreads API, no scraping.
2. **Generic CSV/JSON import**: stable documented schema with dry-run preview, duplicate
   resolution, and rollback on failure.

## Planned pipeline

```text
choose file → parse → map columns (default guesses + user overrides)
  → dry-run preview (rows, detected duplicates, validation problems)
  → duplicates resolution (skip / merge-as-copy / add-anyway, per row or global)
  → commit (transactional batches) → result log (import_jobs row + per-row messages)
```

## Mapping rules

- A CSV row becomes a **work + edition (+ optionally one copy)**. Books marked
  "currently-reading"/"read"/"to-read" create reading sessions in Phase 2 terms;
  until Phase 2 ships, reading-only rows import as works/editions with the shelf recorded
  in `activity_events`.
- ISBN columns are checksum-validated; invalid ISBNs are reported per-row, not silently
  dropped. A row without an ISBN may still import (manual edition).
- Money fields (`purchase price`) are parsed per `src/lib/format.ts` rules into integer
  minor units + currency; rows with unparseable values import without cost, never with a
  guessed value.
- Dates parse as ISO 8601 first, then `DD.MM.YYYY`, then Goodreads' `YYYY/MM/DD`.

## Duplicate policy (mirrors the add-book flow)

- Exact normalized ISBN match → duplicate (user chooses: skip / add copy to edition /
  create new edition).
- Normalized title + author match → soft duplicate warning, per-row decision.
- Never auto-merge two works. Never delete user data on import.

## Failure handling

- The whole import is audited in `import_jobs` (source, stats, status).
- Commit happens in batches; a hard failure rolls back the affected batch and reports
  exactly which rows failed and why. Partial successes are explicit, never silent.

## Test expectations (from the test matrix)

Weird CSV quoting, mixed encodings, duplicate rows, partial-failure rollback, missing
fields — each gets a fixture-based test in Phase 4.
