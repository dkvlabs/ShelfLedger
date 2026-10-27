# Changelog

All notable changes to ShelfLedger are documented here. Format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/); versioning is
[SemVer](https://semver.org/).

## [Unreleased]

## Development history (before automated releases)

### Added — release automation (2026-10-27)
- GitHub Actions release PRs synchronize all app versions and generate categorized
  descriptions from Conventional Commits. Merging builds macOS Apple Silicon and
  Intel, Windows x64 and Linux x64 installers and publishes SHA-256 checksums.
- Releases remain drafts until all checks and installers succeed, with retry by
  tag, optional Apple Developer ID signing/notarization, and documented setup.

### Added — localization and local market comparisons (2026-10-27)
- Offline translations for ten languages and the complete active ISO 4217
  currency list, with currency-specific decimal scales and right-to-left Arabic.
- User-initiated OLX Romania searches and offline comparable listings with
  source, date and confidence; evidenced per-currency valuation estimates.

### Fixed — Goodreads CSV import (2026-10-27)
- Unrelated legacy timestamps such as `x` no longer reject every imported row.
  Internal staging preserves existing data and validates only the incoming delta;
  external snapshot validation stays strict and row commits remain atomic.
- Decimal ratings such as `5.0` are retained; Goodreads zero stays unrated.
  Publisher, binding, page count, publication year and additional authors are imported.
- Blank spreadsheet ISBN wrappers produce manual editions, unknown dates stay
  blank with a warning, and repeated imports skip duplicates without cloning the library.
- Verified a private 209-row export with zero errors, all 81 ratings and 83 finish
  dates retained, and an idempotent second import. See `docs/import-format.md`.

### Added — audit remediation and cinematic redesign (2026-10-27)
- Ink, copper and ivory interface with bundled literary typography, original
  atmospheric artwork, collection-first navigation and responsive layouts.
- Work/edition/copy and reading-session editors, tags, archived history with
  restoration, complete import review/mapping/row outcomes, and library profiles.
- Versioned logical JSON snapshots preserving catalog, reading, lending and
  financial history; regression coverage for complete interchange and live restore.

### Fixed — audit remediation (2026-10-27)
- Live WAL restore and consistent safety backups; app-data confinement and safe
  temporary paths; atomic import rows and accurate result counts.
- Web request-origin, Host, content-type and session-token protection.
- Goodreads reviews/shelves/unknown dates; search-index lifecycle updates;
  nullable patches; Rust-side date, ISBN, money and relationship validation.
- Same-date valuations, disposal/listing transitions and per-currency dashboard
  totals; currency-specific input/display scales; reading/lending dead ends.
- EN/RO product copy, system-theme tracking, import-file handoff and record
  identity in inventory dialogs. Updated dependency and toolchain declarations.
- See `docs/audit-remediation.md` for evidence and remaining release limitations.

### Added — user-requested follow-ups (2026-10-27)
- ISBN barcode scanning in Add Book: live camera where available, photo
  scanning everywhere, USB/Bluetooth scanner input, checksum-validated
  extraction (book-range barcodes only).
- Web version: run the app in a browser against the same local Rust core
  (`pnpm web:server`, loopback HTTP; see docs/web.md), with browser file
  pickers, uploads, and download links for exports/backups.
- Original app icon across all platforms plus a web favicon.

### Fixed
- `isbn10ToIsbn13` rejected ISBN-10s whose check digit is `X` (~1 in 11
  books); such ISBN-10s now derive their ISBN-13 correctly.

### Added — Phase 0–1 (scaffold + core catalog & inventory)
- Tauri 2 + React 19 + TypeScript + Tailwind + shadcn-style UI scaffold with
  least-privilege capabilities and CI on macOS/Windows/Linux.
- SQLite data layer (sqlx, embedded migrations, foreign keys, WAL, FTS5 search):
  works → editions → copies with authors, locations hierarchy, acquisitions,
  settings, activity events, and provider cache.
- Catalog UI: library grid/table with full-text search, Add Book flow (Open Library
  ISBN lookup with local cache and offline manual fallback, duplicate warning),
  work detail with editions and copies, archive (soft delete), location manager
  with room › bookcase › shelf › box hierarchy.
- Typed Tauri commands; money stored as integer minor units + ISO 4217 currency.

### Added — Phase 2 (reading & loans)
- Reading journal: want-to-read / reading / finished / paused / DNF sessions with
  rereads and DNF-restarts as separate sessions, progress log, ratings, reviews.
- Contacts (local only) and lending: one active loan per copy enforced in the
  domain layer AND by a partial unique index; lend/return flows; overdue view on
  the user's local date.
- Dashboard: works/copies/unread counts, currently reading, loan counters.

### Added — Phase 3 (resale & money)
- Time-bound value observations (manual or user-verified comparables) with source,
  date and confidence — never live quotes, never invented prices.
- Sale listings and disposals (sold/donated/traded/lost) that preserve all copy
  history; a lent copy cannot be sold or disposed before its return.
- Transparent keep/sell/donate assistant: deterministic rules over user data with
  2–4 concrete reasons, manual override, and a what-if net-proceeds calculator.
- Per-currency reports (spending/proceeds by year, estimated collectible value);
  mixed currencies are never summed.

### Added — Phase 4 (portability & polish)
- Goodreads CSV import wizard with preview, per-row selection, duplicate
  resolution, and import audit (`import_jobs`); generic JSON import.
- CSV and JSON export; crash-safe backup archives (VACUUM INTO snapshot +
  SHA-256 manifest, zipped) with validated, confirmed restore (replace or
  separate profile).
- Light/dark/system theme and English/Romanian localization; DD.MM.YYYY UI dates.
