# Changelog

All notable changes to ShelfLedger are documented here. Format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/); versioning is
[SemVer](https://semver.org/).

## [Unreleased]

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

## [0.1.0] — initial development release (planned)

First packaged builds for macOS, Windows, and Linux once signing/notarization is
in place (see docs/release.md).
