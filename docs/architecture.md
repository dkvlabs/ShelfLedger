# Architecture

Status: implemented catalog, reading, lending, finance, portability and local web
transport, reviewed 27.10.2026. Release acceptance is tracked separately in
[release.md](release.md).

ShelfLedger stores the library on this device. The desktop application uses Tauri
2 and named, typed commands. The optional `shelfledger-web` binary serves the same
Rust domain over **127.0.0.1 only**; there is no cloud service, account or remote
access feature. Both transports validate Rust DTOs before writes. Neither accepts
SQL from the frontend.

## Boundaries

- `src/features/*`: React screens; `src/components/ui`: shared accessible primitives.
- `src/lib/api.ts` / `src/types/domain.ts`: command wrappers and matching DTOs.
- `src-tauri/src/commands`: Tauri entry points and shared data-file operations.
- `src-tauri/src/domain`: catalog, reading, loans, finance, imports, backup, profiles.
- `src-tauri/src/repositories`: SQLite persistence, derived FTS index and versioned snapshots.
- `src-tauri/src/db.rs`: WAL, foreign keys, six embedded migrations and connection pool.
- `src-tauri/src/sqlite_db.rs`: SQLite-only SQLx facade. Direct driver/core versions
  are pinned together at 0.8.6; upgrades must verify the facade and migration checksums.
  Unused MySQL/Postgres drivers and their crypto dependencies are excluded.

Works, editions and physical copies are distinct. Reading sessions belong to works
and optionally an edition of that work. Ownership belongs to copies. Loans,
listings and disposals retain their own history. Money uses integer minor units
and a validated ISO currency; reports group by currency and never invent rates.

## Offline and privacy

All core workflows use SQLite without network. Open Library lookup happens only
on request, is cached, and leaves manual entry available after failure. Cover URLs
are not fetched automatically. Display fonts and decorative artwork are bundled.
Camera scanning requests video only when opened, preserves platform consent,
handles denied/missing/busy cameras and releases streams when closed. No scanning
image is uploaded; photo and manual ISBN input remain available.

Tauri permissions are enumerated in `src-tauri/capabilities/default.json`; camera
purpose/entitlement declarations are in `Info.plist` and `Entitlements.plist`.
Production CSP restricts content and allows the local camera preview. The web
server checks loopback Host, Origin, JSON Content-Type and a process-session token
for API POSTs. It sends anti-framing, no-sniff and no-store headers. Development
allows the documented Vite origin; it does not enable permissive CORS.

OLX comparisons are entered by the user, scoped to an edition, and stored locally.
Only explicit clicks open validated HTTPS OLX Romania listings/book searches in
the system browser. A named Rust command uses the opener plugin; generic webview
opener permissions are not granted. No iframe, scraping, marketplace API, remote
images or background requests are used. Saved valuations retain immutable listing
URLs, advertised prices, observation dates, notes and user-selected confidence.

## Persistence and recovery

Catalog mutations update affected FTS rows with the record transaction. Copy
patches distinguish omitted fields from explicit null clears. Active loans and
active sale listings have partial unique indexes. Listing/disposal transitions
are transactional; disposal closes active listings for every terminal state.

JSON snapshot format 3 preserves every logical table, including archived copies,
financial/reading/loan history, settings, cache and import logs. Restore first
verifies archive hashes, SQLite integrity and migrations. Replace creates a
consistent safety ZIP, then replaces records in a transaction on the live pool:
**the open database, WAL and SHM files are never overwritten or removed**. No
restart is required. Separate restore creates a selectable library profile.

CSV and legacy-array imports stage trusted local records without revalidating
old values, then validate and atomically merge only newly created records.
An unrelated legacy timestamp cannot block new rows. Incoming snapshots retain
full validation. Re-importing with skip rechecks live duplicates before staging.

Generated exports, backups, uploads and profile files stay under app data.
Canonical ancestor checks reject traversal and symlink escapes before directory
creation. Snapshot attachment paths must be relative. Binary receipt/cover
attachments are deferred; JSON is a record interchange, not an asset package.

## Deliberate limits

One local operator per running library is the supported deployment. Sync,
accounts, remote hosting, e-books, receipt management, automated marketplace collection
and very-large-library virtualization remain outside the current phase. See
[implementation.md](implementation.md) for verified coverage and remaining gates.
