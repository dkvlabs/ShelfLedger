# Architecture

Status: implemented catalog, reading, lending, finance, portability and local web
transport, reviewed 27.10.2026. Release acceptance is tracked separately in
[release.md](release.md).

ShelfLedger stores the library on this device. The desktop application uses Tauri
2 and named, typed commands. The optional `shelfledger-web` binary serves the same
Rust domain over **127.0.0.1 only**; ShelfLedger operates no hosted service or account
server. Native apps optionally sync through the user's Google Drive. Both transports validate Rust DTOs before writes. Neither accepts
SQL from the frontend.

The Android application shares these typed commands and SQLite database. Its
release manifest permits INTERNET for explicit update checks and Drive sync and disables OS
cloud backups.
All online metadata providers are always disabled on Android, while imported caches
remain readable. React Query uses `networkMode: always` for local IPC operations.
Native document commands stage bounded content-provider streams under app data;
only generated export/backup files may be copied to a destination chosen through
the OS save dialog. This explicit user-directed transfer is the only external
file-write exception. No raw content URI reaches import/restore domain code.
See [Android and portability](android.md).

## Boundaries

- `src/features/*`: React screens; `src/components/ui`: shared accessible primitives.
- `src/lib/api.ts` / `src/types/domain.ts`: command wrappers and matching DTOs.
- `src-tauri/src/commands`: Tauri entry points and shared data-file operations.
- `src-tauri/src/domain`: catalog, reading, loans, finance, imports, backup, profiles.
- `src-tauri/src/repositories`: SQLite persistence, derived FTS index and versioned snapshots.
- `src-tauri/src/db.rs`: WAL, foreign keys, seven embedded migrations and connection pool.
- `src-tauri/src/sqlite_db.rs`: SQLite-only SQLx facade. Direct driver/core versions
  are pinned together at 0.8.6; upgrades must verify the facade and migration checksums.
  Unused MySQL/Postgres drivers and their crypto dependencies are excluded.

Works, editions and physical copies are distinct. Reading sessions belong to works
and optionally an edition of that work. Ownership belongs to copies. Loans,
listings and disposals retain their own history. Money uses integer minor units
and a validated ISO currency; reports group by currency and never invent rates.

## Offline and privacy

All core workflows use SQLite without network. ISBN lookup tries Open Library
then Google Books on explicit request, with exact edition matching. A separate
user-supplied Libris product link imports Book JSON-LD and rejects ISBN mismatches.
Shared bounded HTTPS requests restrict providers and redirects; all successful
results are cached, including compatibility with older Open Library cache keys.
Every provider obeys `metadata.lookup_enabled`, and Android blocks network
lookups regardless of restored settings. Manual entry retains typed ISBNs after
failed lookups. See [metadata.md](metadata.md). Covers are fetched only through
an explicit edition action, or supplied as a local photo. Validated, resized JPEGs
live in SQLite and display offline without remote image URLs. See [covers.md](covers.md).
Settings reads the running binary version locally through `get_app_info`. Only
pressing Check for updates calls `check_for_updates`: a bounded (1 MiB), ten-second
HTTPS request to the fixed official GitHub latest-release endpoint, without
redirects, credentials or library data. Stable SemVer precedence determines
availability; matching platform/architecture assets determine download availability.
`open_update_page` accepts an enum of fixed project-owned browser destinations.
Downloads and installation remain manual.

Google Drive authorization and sync run only through native typed commands.
Desktop uses the system browser, PKCE and a state-checked loopback callback;
Android uses Google's AuthorizationClient. Tokens stay in native memory, with
no refresh token persistence. Only `drive.appdata` is requested. Cloud operations
are serialized; opening Settings never initiates them. See [cloud-sync.md](cloud-sync.md).

Display fonts and decorative artwork are bundled.
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

JSON snapshot format 4 preserves every logical table, including edition cover bytes, archived copies,
financial/reading/loan history, settings, cache and import logs. Restore first
verifies archive hashes, SQLite integrity and migrations. Replace creates a
consistent safety ZIP, then replaces records in a transaction on the live pool:
**the open database, WAL and SHM files are never overwritten or removed**. No
restart is required. Separate restore creates a selectable library profile.

Drive sync stores immutable format-4 logical snapshots, excluding settings.
Parent revision IDs describe history; multiple heads or changes on both devices
require a whole-library choice. No remote version is overwritten or deleted.
Cloud history can restore an older revision by publishing a new revision.
Pulls save a local recovery ZIP and compare the captured snapshot and active
profile inside the replacement transaction, rejecting intervening edits. Valuation
row order is preserved. Profile links/checkpoints live outside library exports;
device preferences and profile identity remain local. iCloud backups use the
existing native file picker and ZIP restore, with no CloudKit integration.

CSV and legacy-array imports stage trusted local records without revalidating
old values, then validate and atomically merge only newly created records.
An unrelated legacy timestamp cannot block new rows. Incoming snapshots retain
full validation. Re-importing with skip rechecks live duplicates before staging.

Generated exports, backups, uploads and profile files stay under app data.
Canonical ancestor checks reject traversal and symlink escapes before directory
creation. Snapshot attachment paths must be relative. Edition covers are embedded
in JSON and ZIP backups; general attachment files and receipts remain deferred.

## Deliberate limits

One local operator per running library is the supported deployment. Automatic
sync, per-record merging, end-to-end encrypted sync, ShelfLedger accounts, remote hosting, e-books, receipt management, automated marketplace collection
and very-large-library virtualization remain outside the current phase. See
[implementation.md](implementation.md) for verified coverage and remaining gates.
