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
commas in primary names; semicolons separate multiple primary authors. Goodreads
`Additional Authors` is read as its comma/semicolon-separated contributor list.
Publisher, binding, positive page count, and edition publication year are retained.
`Exclusive Shelf` maps
`to-read`, `currently-reading`, `read` to independent reading sessions. Custom
Bookshelves become tags. Rating and review are retained. Integral ratings such as
`5.0` and `4.00` are accepted; Goodreads `0` means unrated and stays null. Invalid
optional ratings, pages, publication years and reading dates produce preview
warnings rather than rejecting the book. Date Read accepts
YYYY/MM/DD, YYYY-MM-DD, DD.MM.YYYY, or a valid RFC3339 timestamp. Unknown
reading dates stay null; import never
invents a finish date. Arbitrary UTF-8 date input cannot panic.

Preview exposes duplicate matches and selected rows. Choose skip, add a physical
copy to the existing edition, or create a separate work globally or per row.
Conflicting ISBNs on a separate work produce a manual edition, never an orphan.
Each selected row is staged and merged atomically: failure rolls back its work,
editions and copies. Results distinguish imported, skipped and failed rows;
`import_jobs.stats_json` retains the row log and counts. Re-import with skip is
idempotent. Live duplicates selected for skipping bypass expensive staging. Rows
not selected are untouched. Trusted existing rows are cloned internally without
revalidating legacy values; only new staged records are merged and validated.
This prevents an old `works.created_at = 'x'` from failing every CSV row while
keeping incoming JSON snapshot validation strict. Existing dates are not guessed
or rewritten.

CSV is a catalog/reading import, not a lossless Goodreads backup. Date Added does
not establish purchase or reading-start dates, and shelves do not establish
physical ownership. No physical copies are created on the initial CSV import;
add owned copies explicitly afterwards. Goodreads IDs, average/community ratings,
Original Publication Year, shelf positions, spoiler/private-note fields and Read
Count are not mapped into invented local history. A read count greater than one
does not create fabricated reread sessions. Keep the source export for those fields.

### Verification with the supplied export — 27.10.2026

The private 209-row Goodreads export was exercised through the same Rust file
preview and commit functions as the wizard, using a disposable on-disk library
containing an existing `x` timestamp. All 209 works and editions imported without
row errors; 81 supplied ratings and 83 finish dates were retained. A second import
skipped every row and created no duplicate works, editions or copies. The private
CSV is not stored in this repository. The macOS app bundle was rebuilt and its
ad-hoc signature verified after the fix.

The synthetic regression also checks an empty library, quoted commas, multiline
Unicode reviews, BOM headers, ISBN wrappers and fallback, manual editions,
additional authors, publication metadata, unknown dates, persistence after reopen,
and strict snapshot rejection/atomic invalid-row rollback. To repeat the optional
local full-file check:

```sh
SHELFLEDGER_TEST_GOODREADS_CSV=/absolute/path/to/export.csv \
  cargo test --manifest-path src-tauri/Cargo.toml \
  private_goodreads_export_imports_every_row_and_repeat_is_idempotent -- --ignored --nocapture
```

This opt-in check expects an export whose selected rows are distinct and have no
matches with the test fixture. It imports into a temporary library and removes
that test library when complete; it never commits the CSV or writes the real library.

## Lossless JSON snapshot, format 4

```json
{"formatVersion": 4, "tables": {"works": [], "authors": [], "editions": [], "edition_covers": []}}
```

The abbreviated example shows the envelope only. Actual exports contain all 24
logical tables listed in [database.md](database.md), with snake_case SQL column
names and bound JSON scalar values. They retain IDs, author names containing
commas, descriptions, editions, archived copies, complete acquisitions, tags,
reading progress/reviews, contacts/loans, valuations/listings/disposals, settings,
metadata cache, import audit records and checked marketplace comparisons. FTS and migrations are derived/local.

Format 1 snapshots remain supported and are upgraded using the old release’s
currency-scale conventions. Format 2–4 amounts already use corrected ISO minor
units and are never rescaled. Complete format 1/2 exports may omit the new
`market_comparables` table; formats 3/4 require it. Complete format 1–3 exports
may omit `edition_covers`; format 4 requires it. Cover rows contain normalized
JPEG bytes as a data URL, their source and saved date. Invalid images or
inconsistent edition/cover identifiers roll back the entire import.
See [currency upgrades](localization.md#upgrading-older-libraries).

A snapshot imports as a complete unit so references and histories stay together.
Identical existing records are allowed; conflicting IDs/ISBNs/names or invalid
references stop the import transaction. Use a separate library for incompatible
snapshots. Unsupported versions, columns, invalid dates/money/ISBNs and unsafe
relative file paths are rejected. Edition cover images are embedded in JSON;
general attachment files are not.

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
