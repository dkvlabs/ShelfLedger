# Import and export formats

Status: implemented 27.10.2026. Import is explicit and local; there is no Goodreads
API or scraper. Select CSV/JSON in Import or Settings. The selected Settings file
is handed directly to preview. Preview and cancellation do not write library rows.

## CSV / Goodreads

Column names are matched case-insensitively. Custom exports can map their headers
to Title, Author, ISBN13, ISBN, Exclusive Shelf, Bookshelves, My Rating, My Review
and Date Read. Refresh the preview after changing mappings; confirmation is
unavailable until the preview matches the mapping.

Title is required. A valid ISBN13 takes precedence; valid ISBN-10 is a fallback.
Checksum failures produce manual editions with a warning. Author strings preserve
commas in names; semicolons separate multiple authors. `Exclusive Shelf` maps
`to-read`, `currently-reading`, `read` to independent reading sessions. Custom
Bookshelves become tags. Rating and review are retained. Date Read accepts
YYYY/MM/DD, YYYY-MM-DD or DD.MM.YYYY. Unknown reading dates stay null; import never
invents a finish date. Arbitrary UTF-8 date input cannot panic.

Preview exposes duplicate matches and selected rows. Choose skip, add a physical
copy to the existing edition, or create a separate work globally or per row.
Conflicting ISBNs on a separate work produce a manual edition, never an orphan.
Each selected row is staged and merged atomically: failure rolls back its work,
editions and copies. Results distinguish imported, skipped and failed rows;
`import_jobs.stats_json` retains the row log and counts. Re-import with skip is
idempotent. Rows not selected are untouched.

## Lossless JSON snapshot, format 1

```json
{"formatVersion": 1, "tables": {"works": [], "authors": [], "editions": []}}
```

The abbreviated example shows the envelope only. Actual exports contain all 22
logical tables listed in [database.md](database.md), with snake_case SQL column
names and bound JSON scalar values. They retain IDs, author names containing
commas, descriptions, editions, archived copies, complete acquisitions, tags,
reading progress/reviews, contacts/loans, valuations/listings/disposals, settings,
metadata cache and import audit records. FTS and migrations are derived/local.

A snapshot imports as a complete unit so references and histories stay together.
Identical existing records are allowed; conflicting IDs/ISBNs/names or invalid
references stop the import transaction. Use a separate library for incompatible
snapshots. Unsupported versions, columns, invalid dates/money/ISBNs and unsafe
relative file paths are rejected. Binary assets are not embedded in JSON.

Legacy arrays of works → editions → copies remain supported for the bundled demo
and older catalog exchanges. They preserve supplied work descriptions, author
arrays, publication fields and copy/acquisition fields. Named locations match or
create local shelves. They do not represent the complete history snapshot.

## CSV export and backup

CSV exports one row per physical copy for spreadsheet use, with integer amount
columns and currency. It is a tabular inventory report; use JSON for logical
round trips. Backup ZIPs contain a consistent SQLite snapshot and SHA-256 manifest.
Restore verifies hashes, integrity and migrations, creates a consistent safety
backup before replacement, and replaces records transactionally without restart.
Separate restore appears under Settings → Libraries; switching preserves each
library's records. Receipts and asset packaging remain deferred.
