# Implementation status

Roadmap from the product plan (§11), with the current state of this repository.
Estimates are planning ranges, not delivery promises.

## Phase 0 — Spec and scaffold ✅ (2026-10)

- [x] Repo skeleton: LICENSE (MIT), README, CONTRIBUTING, CODE_OF_CONDUCT, SECURITY, AGENTS.md
- [x] Tauri 2 + React + TypeScript + Vite + Tailwind CSS + shadcn/ui
- [x] Tauri capabilities least-privilege (core, window-specific dialog/devtools); CSP set
- [x] CI lint/tests workflow (GitHub Actions, macOS/Windows/Linux)
- [x] Typed DTOs (Rust `models.rs` ↔ `src/types/domain.ts`), migration strategy, currency/date policies
- [x] Design tokens, docs + ADRs

**Acceptance:** boots locally on macOS (dev); empty local database on first launch;
offline launch works. Windows/Linux workflows are configured; this local audit does not certify their execution.

## Phase 1 — Core catalog and inventory ✅ (2026-10)

- [x] SQLite schema: works, authors, editions, copies, locations, acquisitions (+ settings,
      metadata cache, editable tags, activity events, FTS5)
- [x] CRUD for works, editions, owned copies (typed Tauri commands)
- [x] Local search (FTS5 prefix matching) and sorting; typographic book grid and table view
- [x] ISBN lookup with cache (Open Library) and manual fallback; offline degrades to manual
- [x] Duplicate detection by ISBN and title+author with user confirmation
- [x] Multiple owned copies with locations; purchase amount, currency, date, condition

**Acceptance status:** the data layer and UI support entering many books/copies,
identifying duplicates, and finding any book by ISBN/title; data persists in the
app-data SQLite database and survives restarts offline. A scripted 100-book create/reopen/search soak test passes locally. Cross-OS
manual verification remains a release follow-up.

## Phase 2 — Reading and loans ✅ (2026-10)

- [x] Reading sessions for want-to-read, reading, finished, paused and DNF
      (rereads and DNF-restarts as separate sessions; progress log, rating, review)
- [x] Contacts and lending flow; one-active-loan invariant (domain check +
      partial unique index backstop)
- [x] Due date/overdue UI on the user's local date, return records, loan history
- [x] Dashboard summary (works/copies/unread, currently reading, loan counters)

**Acceptance status:** finish/restart of the same title twice preserves both
sessions; lending one of two copies blocks a second loan until return (covered by
tests); overdue views computed against the caller-supplied local date.

## Phase 3 — Resale and money ✅ (2026-10)

- [x] Value observations with source, date, and confidence (manual / user-verified
      comparables only; never live quotes)
- [x] Sell queue using transparent, deterministic rules with reasons and manual
      override; what-if net-proceeds calculator (asking − fee − shipping)
- [x] Listed/sold/donated/traded/lost flows that keep all copy history; lent
      copies cannot be sold before return
- [x] Per-currency dashboard (purchase total, sold proceeds, estimated collectible
      value); mixed currencies are never summed

**Acceptance status:** tests cover mixed-currency separation, unknown-margin
display (cost fields nullable), and history retention after a sale.

## Phase 4 — Portability and polished v1 ✅ (2026-10)

- [x] Goodreads CSV import wizard (preview, per-row selection, duplicate policy,
      `import_jobs` audit) and generic JSON import
- [x] CSV/JSON export; crash-safe SQLite+manifest backup archives (VACUUM INTO,
      SHA-256 manifest) with corruption rejection and confirmed restore
      (replace / separate profile)
- [x] Light/dark/system theme; EN and RO localization; DD.MM.YYYY UI dates
- [x] Unit/integration tests; packaging pipeline in CI for the three desktop OSs

**Acceptance status:** importing the same Goodreads file twice is safe (duplicates
skipped); restore is hash-verified with tamper rejection tested; the app boots
with no network (lookup is opt-in). Cross-OS packaged install testing still
requires real release builds — tracked in docs/release.md.

## Phase 5 — Community release ⬜ (release gates pending)

- [x] Repository docs: README, CONTRIBUTING, CODE_OF_CONDUCT, SECURITY, PRIVACY,
      CHANGELOG, issue templates, starter issues list, demo dataset
      (`demo/demo-library.json`), release checklist (docs/release.md)
- [x] Security workflow: `cargo audit` + gitleaks secret scan
- [x] Release automation: reviewed version/changelog PRs, four native build
      targets, installer assets and SHA-256 checksums, draft recovery; first
      live GitHub run and signed distribution remain to be verified
- [ ] Release artifacts for three OSs with signing/notarization — requires
      platform accounts and keys; see docs/release.md (deliberately not faked)

**Acceptance status:** development users can build, create a library,
import, lend, sell, and back up without a terminal. Signed installers, cross-OS clean installs and physical-camera checks remain release gates.


## Phase 6 — After feedback ⬜ (non-MVP)

Receipts, automated marketplace collection, ebooks and optional sync remain
deferred. Barcode scanning and manual OLX comparisons were pulled forward on
request — see below.

## User-requested follow-ups ✅ (2026-10-27)

Scope beyond the original plan, implemented on explicit request:

- [x] OLX Romania comparison workflow per edition: external ISBN/title searches,
      user-verified listing URLs and prices, observation dates, condition notes,
      currency-separated medians/ranges, exclusions, exact net-proceeds simulation,
      and explicit copy valuations with immutable source evidence. Migration 0006,
      snapshot format 3 with format 1/2 compatibility, eleven UI languages.
      No automatic scraping or iframe integration. See [market-comparisons.md](market-comparisons.md).

- [x] ISBN barcode scanning in Add Book — camera where the platform offers it,
      photo scanning where camera access is unavailable, USB/Bluetooth
      scanner input, manual fallback. Web-API-only
      decoding (`BarcodeDetector` → pure-JS ZXing), checksum-validated ISBN
      extraction (plan Phase 6 item, pulled forward).
- [x] Web version: the same Rust core served to a browser over loopback HTTP
      (`shelfledger-web` bin, `POST /api/<command>` transport sharing the
      `domain/` layer with the Tauri commands). See docs/web.md.
- [x] Original app icon (`app-icon.svg` → full platform icon set) replacing the
      Tauri template icons.
- [x] Camera access recovery: macOS camera purpose/signing declarations,
      platform consent prompts, video-only requests, permission guidance and
      retry, missing/busy camera messages, and camera cleanup on close.
      Permission and lifecycle behavior is covered by frontend tests; physical
      camera access in packaged builds still requires a device smoke test.

**Acceptance status:** scanning decodes EAN-13/ISBN-10 and rejects non-book
barcodes (unit-tested); the web version round-trips add-book → search over
HTTP in tests and in a live browser drill; icon rendered and visually checked
at 512 px and 32 px. One pre-existing bug found and fixed along the way:
`isbn10ToIsbn13` used to reject ISBN-10s with an `X` check digit.

## Test matrix status (spec §13)

| Area | Status |
| --- | --- |
| Metadata (ISBN checksum, invalid results, provider off) | ✅ Rust tests |
| Data (FKs, duplicate ISBN, manual record without ISBN) | ✅ Rust tests |
| Money (decimal→minor, no-price, currency validation) | ✅ Rust + Vitest |
| Search (FTS prefix, injection-safe queries) | ✅ Rust tests |
| Locations (hierarchy, parent protection, unknown parent) | ✅ Rust tests |
| Copies (acquisition, archive, update) | ✅ Rust tests |
| Reading / Lending / Valuation / Import / Backup | Rust domain and public-command regressions |
| Browser workflows | Manual desktop/mobile verification; see audit-remediation.md |

## Audit remediation and redesign — 27.10.2026

The October audit found real gaps behind the phase completion markers. The current
implementation adds lossless snapshots, live-pool restore, separate libraries,
transactional import staging and row logs, request/path protection, deterministic
financial reporting, safe copy patches and search refresh, complete catalog and
reading editors, and a cinematic responsive collection interface. See
[audit-remediation.md](audit-remediation.md) for the finding-by-finding disposition,
validation evidence and remaining upstream/release limitations. Phase markers are
implementation summaries, not proof of packaging or acceptance on every OS.

## Localization expansion — 27.10.2026

- [x] Initial ten global languages plus Romanian, bundled offline with native-script fonts.
- [x] Arabic/Urdu document and component direction, logical alignment, mirrored mobile navigation.
- [x] All 165 current ISO fixed-scale currencies/fund units, localized selector names,
      exact zero/two/three/four decimal amounts, localized digit entry.
- [x] One-time v4 library migration and format 1 JSON compatibility preserve old displayed amounts.
- [x] Translation completeness, official-catalog parity, per-code round trips, preference
      validation and atomic upgrade/overflow regressions.

See [localization.md](localization.md) for extension and maintenance instructions.
