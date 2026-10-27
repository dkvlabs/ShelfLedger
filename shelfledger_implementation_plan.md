# ShelfLedger — Open-Source, Local-First Personal Library

> **Status:** Product brainstorm + implementation blueprint (21.10.2026)  
> **Working name:** ShelfLedger (not trademark/domain-checked)  
> **Platforms:** macOS, Windows, Linux  
> **Principle:** Data belongs to the user. No account, cloud, or subscription required.

## 1. Vision

An offline-first desktop app to catalog books, track reading, own and manage multiple physical copies, record acquisition costs, lend books to friends, and make informed decisions about keeping or selling each copy.

**Product positioning:** Goodreads-style reading journal + personal library inventory + purchase/loan/sale ledger. It is **not** a social network, marketplace, or automated book-price oracle.

### Product principles
1. **Copy-first inventory:** a literary work, a published edition, and an owned copy are separate objects.
2. **Offline and private by default:** all important interactions work without internet.
3. **Truthful values:** purchase cost, asking price, estimated market value, and actual sale price are distinct fields.
4. **Zero lock-in:** backups, CSV/JSON export, readable SQLite database, stable IDs and schema migrations.
5. **Fast entry:** ISBN, paste, manual lookup, duplicate detection, later USB/webcam barcode support.
6. **Low maintenance:** single-desktop-app architecture; no server, Docker, user registration, or hosted database.

### Primary users
- Readers with 20–2,000 books who want reading history and collections.
- Collectors who own multiple editions/copies and care about condition and provenance.
- People lending books to friends and wanting to remember who has what.
- Readers decluttering or selling duplicates, unread books, and old editions.

## 2. Why another app?

- Goodreads and BookWyrm focus on reading activity, discovery, reviews, and community.
- BookBuddy already offers owned-book cataloging, scanning, and loans.
- LibrisLog offers open-source book tracking, ISBN lookup, imports and statistics.

Therefore **reading status + loans alone are not a differentiator**. ShelfLedger's opportunity is the complete **per-copy lifecycle** from acquisition to storage to reading to lending to selling/donating, with financial decision support and local-first desktop usability.

## 3. Core user stories

1. I scan an ISBN and see the metadata, cover, and a duplicate warning before adding a copy.
2. I record when, where, for how much, and in what condition I acquired a copy.
3. I read a title twice, with separate started/finished dates and progress history.
4. I lend one of two copies to a friend and track the expected return and actual return.
5. I search for a book and see which shelf/box/room holds my copy.
6. I decide whether to sell based on current condition, whether I read/loved it, sentiment, expected proceeds, and verified/manual market comparables.
7. I list, sell, donate, lose, or trade a copy without deleting its history.
8. I see purchase spending, recovered sale proceeds, net outlay, unread backlog, and active loans.
9. I import my Goodreads CSV, review mappings, and avoid accidental duplicates.
10. I move the complete library to another computer using a backup archive.

## 4. Functional specification — feature backlog

Priority: **P0 = MVP**, **P1 = first release**, **P2 = later**.

| Module | Feature | Priority |
| --- | --- | --- |
| Catalog | Add/edit title, subtitle, author(s), language, genres, series | P0 |
| Catalog | Separate work, edition, and copy | P0 |
| Catalog | ISBN-10/ISBN-13 lookup; manual entry offline | P0 |
| Catalog | Multiple owned copies, each with unique copy ID | P0 |
| Catalog | Cover caching and manual cover/image overrides | P1 |
| Catalog | Duplicate detection by ISBN/edition and existing ownership | P0 |
| Catalog | Bulk actions, tags, personal collections, advanced filters | P1 |
| Reading | Want to read, reading, finished, paused, DNF, rereading | P0 |
| Reading | Reading session, start/end date, pages, rating, review | P0 |
| Reading | Progress log and reading timer | P1 |
| Reading | Quotes, highlights, notes, goals, annual stats | P1 |
| Inventory | Physical location (room > bookcase > shelf > box) | P0 |
| Inventory | Condition, notes, wear, photos, personal value | P0 |
| Inventory | Inventory labels/QR codes | P2 |
| Finance | Purchase date, purchase price, currency, seller/store | P0 |
| Finance | Purchase type (bought, gift, trade, inherited), shipping, tax | P1 |
| Finance | Current manual estimated resale price; confidence/source/date | P0 |
| Finance | Sale listing, fees, shipping paid, sold date/price | P1 |
| Finance | Realized net proceeds and per-copy gain/loss | P1 |
| Finance | Costs and proceeds by year/category/store, currency-safe reporting | P1 |
| Lending | Friend/contact, date lent, due date, returned date, notes | P0 |
| Lending | Active/overdue list, local reminders and loan history | P1 |
| Lending | Contact details optional; no automatic message sending | P0 |
| Decision | Rule-based keep/sell/donate review queue | P1 |
| Decision | Transparent sale calculator with editable assumptions | P1 |
| Wishlist | Books wanted, maximum price, priority, reason | P1 |
| Imports | Goodreads CSV import with mapping and preview | P1 |
| Data | Local SQLite, migrations, CSV/JSON exports, backup and restore | P0 |
| UX | Dark/light modes, keyboard navigation, EN/RO localization | P1 |
| Extras | USB scanner/webcam barcode, phone companion | P2 |
| Extras | EPUB/PDF management (metadata only at first), optional AI | P2 |
| Extras | Sale marketplace link-outs or user-owned export listings | P2 |

### Fields for one physical copy

- Identifiers: `copy_id`, `edition_id`, optional inventory code.
- Acquisition: `acquisition_type`, `purchase_at`, `purchase_price_minor`, `currency`, `store`, `order_reference`, optional receipt attachment; all nullable as relevant.
- Location: `location_id`, local shelf position/slot.
- Condition: `condition_grade` (new/like_new/very_good/good/acceptable/poor), defects, signed/annotated, dust jacket, photos.
- Ownership: `ownership_status` (owned/lent/listed/sold/donated/traded/lost), acquisition and disposal timestamps.
- Value: `estimated_value_minor`, `estimate_currency`, `estimate_source`, `estimate_at`, `estimate_confidence`, `minimum_acceptable_price_minor`, optional sentimental rating.
- Book-specific custom fields; freeform notes and tags.

**Important:** A wishlist book is not an owned copy. A book can be `finished` and `lent` simultaneously. Do not use one status column for both concepts.

## 5. Workflows

### A. Add book
1. Click **Add Book** or use a keyboard shortcut.
2. Enter/scan ISBN or search title/author; show provider source and missing metadata.
3. Fetch optional metadata; display matching edition and duplicate warning.
4. Select an existing edition or create/edit a manual edition.
5. Add 0 or more owned copies; for each capture purchase price/date, condition and location (0 copies means tracking-only/wishlist).
6. Save transactionally; cache cover locally where permitted, show a compact success card.

### B. Loan a copy
1. Open a copy, click **Lend**.
2. Select a saved contact or add a name; enter loaned/due dates and notes.
3. Validate the copy is available and no active loan exists.
4. Save active loan, set inventory availability to lent.
5. On return, record returned date/condition; retain loan history.
6. Overdue views use the user's local date; optional OS notification where supported.

### C. Sell/keep assistant
1. Filter available books to candidates: duplicates, unread for years, DNF, low-rated, no sentimental value, or manually flagged.
2. For each copy show original cost, historical cash outlay, reading history, condition, and current value **only if supplied/verified**.
3. Enter asking price, expected selling fee and shipping paid by seller.
4. Compute estimated net = asking price – fees – seller-paid shipping; estimated gain/loss = net – allocated acquisition cost.
5. Show **Consider selling**, **Consider keeping**, or **Insufficient data** with concrete reasons and editable rules.
6. Do not equate a generated recommendation with actual market valuation.
7. Once sold, record actual realized amounts; lock sale in history rather than deleting ownership entry.

### D. Restore after reinstall
1. Select **Restore archive** on welcome/settings page.
2. Validate backup file format, schema version, integrity, and manifest hash.
3. Offer replacing the current library or importing into a separate profile; confirm before destructive replacement.
4. Restore SQLite snapshot and media atomically; run compatible migrations; reopen app.
5. Confirm book count, active loans, and attachments.

## 6. User experience / screen inventory

**Navigation:** Dashboard / My Library / Reading / Loans / Sell & Value / Wishlist / Insights / Settings.

1. **Welcome/Empty state:** Create local library, set currency (RON/EUR), import, sample data toggle.
2. **Dashboard:** book counts, currently reading, unread, loans due, estimated value with provenance, acquisition spending (per currency), quick Add/Lend/Sell.
3. **My Library:** cover grid + dense table toggle; advanced filters; physical shelf/room facets; stable multi-select.
4. **Book detail:** title, author, metadata and editions; tabs **Overview | My Copies | Reading | Notes | History**.
5. **Copy detail:** cost, condition, location, photo gallery, loan actions, resale details, event timeline.
6. **Reading workspace:** sessions, current progress, timelines, ratings, notes.
7. **Loans:** current, due soon, overdue, returned; contact profile; quick return.
8. **Sell & Value:** decision queue, user-entered market comparisons, what-if net-proceeds slider, listing/sale history.
9. **Wishlist:** desired edition/format, price ceiling, tags and priority.
10. **Insights:** spending by year/currency, reading goal, unread purchase backlog, resale totals, lent-out count.
11. **Import wizard:** choose file, map columns, preview, duplicates resolution, commit, result log.
12. **Settings:** language, currency preferences, database location, imports/exports, backup, updates, API provider settings, privacy.

### UX rules
- Display book **title + edition + copy identifier** in any action changing inventory.
- Provide undo/confirmation for sales, deletion, and bulk edits.
- Never silently auto-update user-written metadata or financial values.
- Support keyboard navigation, screen reader labels, reduced motion, fast list virtualization for thousands of books.
- Currency formatting: RON/EUR with localized separators. Dates: DD.MM.YYYY in UI; ISO 8601 in storage.

## 7. Recommended technical architecture

### Stack
- **Desktop shell:** Tauri 2, targeting macOS, Windows, Linux.
- **UI:** React + TypeScript + Vite + Tailwind CSS + shadcn/ui + TanStack Table + React Hook Form + Zod.
- **State:** TanStack Query for app-data query cache; minimal local state (Zustand only if needed).
- **Backend:** Rust Tauri commands with a domain service layer; input validation before database writes.
- **Database:** SQLite via `sqlx`/Rust with migrations and foreign keys enabled; FTS5 for search.
- **Assets:** user data directory, covers and photos cached locally; paths managed by backend.
- **Testing:** Vitest + React Testing Library; Rust service tests; Playwright/Tauri E2E or appropriate native runner; CI builds on 3 OSs.
- **Distribution:** signed/notarized releases where applicable, GitHub Releases, optional signed Tauri updater.

**Why Rust commands instead of unrestricted SQL in the webview?** A single backend layer enforces invariants like only one active loan per copy, correct transactions, file access limits, and controlled schema changes.

### Module boundaries

```text
React UI (components/pages/forms)
  -> typed frontend client (commands/DTOs)
    -> Tauri IPC commands (Rust)
      -> domain services:
          catalog | inventory | reading | loans | finance | import-export
        -> SQLite repositories + file storage + optional remote metadata adapters
```

No mandatory backend server, no telemetry, no account/password by default. Metadata requests occur only when initiated/enabled by the user. Network features degrade gracefully offline.

## 8. Suggested database design

Core tables:

| Table | Essential columns | Why |
| --- | --- | --- |
| `works` | id, title, subtitle, original_language, description, created_at | Literary identity |
| `authors` | id, name, sort_name | People/creators |
| `work_authors` | work_id, author_id, role, sort_order | Many-to-many |
| `editions` | id, work_id, isbn10, isbn13, publisher, published_at, language, page_count, binding, cover_path, provider_ids_json | Published version |
| `copies` | id, edition_id, inventory_code, location_id, condition_grade, ownership_status, acquisition_type, acquired_at, note, sentimental_score | Owned item |
| `acquisitions` | id, copy_id, acquired_at, price_minor, currency, shipping_minor, tax_minor, store, acquisition_type, receipt_path | Historical acquisition; allow nullable cost |
| `locations` | id, parent_id, name, type, sort_order | Shelf hierarchy |
| `reading_sessions` | id, work_id, edition_id_nullable, started_at, finished_at, status, rating, review | Re-reads as separate events |
| `reading_progress_events` | id, reading_session_id, occurred_at, page, percent | Progress audit |
| `contacts` | id, display_name, contact_info_nullable, notes | Borrowers, local only |
| `loans` | id, copy_id, contact_id, lent_at, due_at, returned_at, notes | Complete loan lifecycle |
| `valuation_records` | id, copy_id, value_minor, currency, source_type, source_note, observed_at, confidence | Value is time-bound evidence |
| `sale_listings` | id, copy_id, platform, listed_at, asking_price_minor, currency, status | Optional listing |
| `disposals` | id, copy_id, disposition, date, sale_price_minor_nullable, fee_minor, shipping_minor, currency, buyer_alias_nullable | Sold/donated/traded/lost history |
| `tags` / `work_tags` | ids, tag label | Custom categorization |
| `wishlist` | id, work_id, edition_id_nullable, maximum_price_minor, currency, priority, note | Desired items, not owned copies |
| `attachments` | id, owner_type, owner_id, relative_path, mime_type, checksum | Cover, receipt, condition photos |
| `import_jobs` | id, source, started_at, finished_at, status, stats_json | Import audit |
| `activity_events` | id, entity_type, entity_id, action, occurred_at, payload_json | User-readable history |
| `settings` | key, value_json | Local preferences |

**Rules and constraints:**
- UUID/ULID IDs for portability. `isbn13` may be NULL; normalize/validate checksums; do not require unique work ISBN.
- `editions.isbn13` unique only for non-null values **after** considering real-world duplicate/wrong ISBNs and manual override handling. Record provider IDs and avoid destructive merges.
- Add partial unique index on `loans(copy_id)` where `returned_at IS NULL`; only one active loan per copy.
- `acquisitions.copy_id` can be unique for v1 one acquisition per ownership period; allow provenance expansion later.
- Money is stored in **integer minor units plus ISO 4217 currency**, not floats. Convert only with explicit stored exchange-rate snapshots; do not sum unrelated currencies.
- Status transitions enforced centrally; sold/donated/lost must not have active loan. Loans can return to available while reading history stays unchanged.
- Retain disposed copies and history; use archive/soft-delete for accidental removal.
- Open metadata fields must be editable and tracked separately from the user's notes/values.
- SQLite settings: `PRAGMA foreign_keys=ON`, WAL journaling, schema migrations and indexes; verify behavior in tests.
- FTS5 index covers title, author names, ISBN aliases, notes, tags, and locations, with tested index maintenance.

## 9. Valuation and sell decision logic

### Financial terms (never mix)
- **Historical cost** = acquisition purchase amount + allocatable acquisition shipping/taxes/fees (if captured).
- **Current estimated value** = latest dated manual/verified market observation, not a live quote.
- **Asking price** = owner's hoped-for price; not revenue.
- **Expected sale proceeds** = asking price − expected marketplace fee − shipping paid by seller − other expected sale costs.
- **Realized sale proceeds** = collected actual price − actual selling expenses.
- **Realized gain/loss** = realized sale proceeds − historical cost; show unavailable if purchase cost unknown. This is a personal collection measure, not automatically tax profit.
- **Cost to keep** is subjective; do not invent depreciation or appreciation rates.

### Explainable decision assistant v1
Rule inputs: read status/last-read date, personal rating, duplicated copies, wishlist/collection importance, sentiment, condition, space needed, minimum net sale proceeds, market evidence freshness.

Outputs: `keep_candidate`, `sell_candidate`, `donate_candidate`, `needs_review`. Every result displays 2–4 reasons and a manual override. No AI required.

Sample: "Consider selling: 2 copies owned, not read for 4 years, no sentimental flag. Estimated net from your entered asking price €16; this is not a verified market price."

### Later optional price research
Allow users to paste comparable sale links, price, date, condition, and currency. Only integrate vendor APIs where their terms permit; **avoid fragile marketplace scraping**. Explicitly label listing prices vs completed-sales evidence.

## 10. Integrations, metadata and import

1. **Open Library first:** work and edition search / ISBN lookup / covers; respect usage guidelines and rate limits, cache results, identify the app with a proper User-Agent/contact, and allow manual fallback.
2. **Google Books optional:** provider adapter with user-configured credentials/settings where needed; users can turn it off.
3. **Goodreads CSV import:** accept exported CSV, map fields via preview, preserve rating, dates, shelves, and reviews where present. Do not depend on a Goodreads API or scrape the website.
4. **Generic CSV/JSON import:** stable documented schema, dry-run preview, duplicates resolution and rollback on failure.
5. **Personal metadata editing:** authors and edition data can be corrected without requiring online providers.
6. **Cover image rights:** third-party cover data may carry separate copyright/license restrictions. Store per-source attribution and only ship images you have permission to distribute; don't bundle scraped cover libraries.

## 11. Implementation phases

Estimates are engineering planning ranges for one developer with AI assistance, **not delivery promises**. Validate platform-specific packaging and data migration before announcing dates.

### Phase 0 — Spec and scaffold (2–4 developer-days)
- [ ] Create repo (license, README, CONTRIBUTING, CODE_OF_CONDUCT, SECURITY).
- [ ] Tauri 2 + React + TypeScript + Vite + Tailwind + shadcn/ui.
- [ ] Configure Tauri capabilities, least-privilege file system access, CI lint/tests.
- [ ] Define typed DTOs, migration strategy, currency/date policies.
- [ ] Prepare design tokens, layouts, sample data and architecture decision records.

**Acceptance:** boots on macOS/Windows/Linux CI targets with an empty local database; offline launch works.

### Phase 1 — Core catalog and inventory (5–8 developer-days)
- [ ] SQLite schema: works, authors, editions, copies, locations, acquisitions.
- [ ] CRUD for books, edition metadata and owned copies.
- [ ] Local search and sorting; cover grid and table view.
- [ ] ISBN lookup with caching and manual fallback.
- [ ] Duplicate detection, multiple copies and stored location.
- [ ] Add purchase amount, currency, date and basic condition.

**Acceptance:** enter 100 books and 120 owned copies; identify duplicates; find any by ISBN/title/location; all data survives restart with Wi-Fi disabled.

### Phase 2 — Reading and loans (4–7 developer-days)
- [ ] Reading sessions for want-to-read, reading, finished, paused and DNF.
- [ ] Start/finish dates, progress, rating, note and rereads.
- [ ] Contacts and lending flow; active loan invariant.
- [ ] Due date/overdue UI, return records, filter borrowed books.
- [ ] Dashboard summary and first insights widgets.

**Acceptance:** finish/restart same title twice and preserve both sessions; lend one of two copies; cannot lend it again before return; overdue views accurate.

### Phase 3 — Resale and money (4–7 developer-days)
- [ ] Value observations with source and date.
- [ ] Sell queue using transparent heuristics and user settings.
- [ ] Sale what-if calculator; listed/sold/donated/traded/lost flows.
- [ ] Gross proceeds vs fees vs net and per-copy gain/loss.
- [ ] Dashboard for purchase total, sold total, estimated collectible value by currency.

**Acceptance:** reports never sum mixed currencies; an unpriced book shows unknown margin; sale does not remove the copy/event history.

### Phase 4 — Portability and polished v1 (4–7 developer-days)
- [ ] Goodreads CSV import wizard and mappings.
- [ ] CSV/JSON export; complete SQLite+assets backup & restore.
- [ ] Light/dark theme, accessibility and keyboard shortcuts.
- [ ] EN and RO translations, DD.MM.YYYY localization.
- [ ] Unit/integration/E2E and migration tests; packaging across 3 desktop OSs.
- [ ] Privacy review and documentation.

**Acceptance:** import a sample Goodreads file twice safely; fresh install restore contains same copies, loans, readings, photos; clean install uses no internet.

### Phase 5 — Community release (2–4 developer-days)
- [ ] Public GitHub repository, issue templates, screenshots, demo dataset, changelog.
- [ ] Release artifacts for three OSs; signing/notarization where applicable.
- [ ] SBOM/dependency and secret checks, vulnerability disclosure instructions.
- [ ] Onboarding walkthrough, localization contribution docs and starter issues.

**Acceptance:** a new user can install, create a library, import books, lend, sell and back up without a terminal.

### Phase 6 — After feedback (non-MVP)
- [ ] USB/barcode reader and webcam scanning; optionally a mobile companion.
- [ ] Receipt imaging, collection insurance/valuation export, QR inventory labels.
- [ ] Price comparison adapters (terms-compliant), user-driven sold-price records.
- [ ] Optional ebook references, reading timer, quotes and goals.
- [ ] Optional sync/multi-user via a separate opt-in module, if demanded.

## 12. Data reliability and privacy

- Make an automatic backup before destructive migrations, in addition to explicit backups.
- Use SQLite's consistent backup facility / `VACUUM INTO` pattern, not a naive live copy of a WAL database; include images/attachments and manifest.
- Ensure recoverability: verify schema version, file hashes, referential integrity and a restore drill in tests.
- Consider at-rest encryption / encrypted exported archives as a future option; never market unencrypted local DB as encrypted.
- Loan contacts' names stay local by default. No automatic sending to friends. No analytics/ads/tracking.
- For plugin/integration network calls use opt-in permission gates and sensible timeouts.
- Do not store app signing keys or production secrets in the repo.
- When opening local EPUB/PDF (future), avoid executing embedded content.

## 13. Automated test matrix

| Area | Tests |
| --- | --- |
| Metadata | ISBN checksum; nonexistent results; provider offline/error; edition mismatch |
| Data | Migration upgrade; foreign keys; duplicate detection; manual record without ISBN |
| Money | RON/EUR; decimal entry to integer minor units; purchase no price; fees; refunds; zero-cost gift; mixed currency totals |
| Reading | Reread; DNF then restart; progress history preserved |
| Lending | Double loan rejection; late return; loaned copy cannot be sold before return; rename contact |
| Inventory | Two identical editions; one lent and one sold; archived history intact |
| Valuation | Missing market evidence; stale evidence; conservative result; user override |
| Import | Weird CSV quoting; mixed encodings; duplicates; partial failure rollback; missing fields |
| Backup | Crash-safe snapshot; restore with attachments; old schema migration; corruption rejection |
| Security | Path traversal protection; no arbitrary SQL from UI; Tauri capabilities least privilege |
| UI | Dense catalog performance, keyboard access, offline, Windows/Linux/macOS smoke tests |

## 14. Open-source policy

**Recommended default:** MIT if your priority is frictionless reuse and outside adoption. Choose **GPL-3.0-or-later** if you care more about derivatives remaining open source when distributed. AGPL is mainly relevant if later publishing a network-accessible hosted version and wanting its source-sharing provisions. **Choose one deliberately before publishing.** Trademark and project-name availability are separate questions.

- Document source attribution and licensing of Open Library/Google Books metadata and cover images separately.
- Keep a clear Privacy Policy even if everything is local.
- Make contributions beginner-friendly; tag `good first issue` and `help wanted`.
- Do not imply compatibility with or endorsement by Goodreads.

## 15. Repo and documentation skeleton

```text
shelfledger/
  src/
    app/
    components/
    features/
      catalog/ inventory/ reading/ loans/ finance/ imports/ settings/
    lib/
    types/
  src-tauri/
    src/
      commands/
      domain/
      repositories/
      metadata/
      backups/
    migrations/
    capabilities/
    tauri.conf.json
  tests/
  docs/
    architecture.md
    database.md
    design.md
    implementation.md
    import-format.md
    decisions/
  .github/workflows/
  README.md
  AGENTS.md
  CONTRIBUTING.md
  LICENSE
```

## 16. Prompt for a coding agent

```text
Build a local-first, open-source desktop application (working name ShelfLedger)
using Tauri 2, React, TypeScript, Vite, Tailwind CSS, shadcn/ui, Rust
and SQLite. Do not add cloud, auth, Docker or mandatory APIs.

Read docs/architecture.md, docs/database.md, docs/design.md,
docs/implementation.md and AGENTS.md first. Implement only the
current phase. Distinguish literary works, editions and physical copies.
Distinguish reading status from ownership/loan/disposal state.
Use integer minor currency units, typed Rust Tauri commands, migrations,
foreign keys, tests and offline-first behavior. Never fabricate price data.

Start with Phase 0 and Phase 1, show changed files and run available
tests; report failures honestly. Don't silently implement later phases.
```

## 17. Product decisions to lock before coding

1. **Desktop-only first?** Recommended yes; mobile companion later.
2. **Physical-first?** Recommended yes; optionally track digital editions without owning a copy.
3. **Single-user?** Recommended yes; no login or accounts.
4. **Default locale/currencies?** English/Romanian, RON/EUR with other ISO currencies supported.
5. **Open-source license?** MIT for adoption, GPL-3.0-or-later for copyleft.
6. **AI?** Not necessary for MVP; transparent rules are preferable for selling decisions.
7. **Sync?** Not in MVP; manual portable backups first.

## 18. Public references (checked 21.10.2026)

- Goodreads: https://www.goodreads.com/
- BookWyrm docs: https://docs.joinbookwyrm.com/
- BookBuddy listing: https://play.google.com/store/apps/details?id=bookbuddy.wan.com.release
- LibrisLog (open-source competitor): https://github.com/codebude/librislog
- Open Library APIs/guidelines: https://openlibrary.org/developers/api
- Open Library ISBN and edition: https://openlibrary.org/dev/docs/api/books
- Open Library Search API: https://openlibrary.org/dev/docs/api/search
- Open Library Covers API: https://openlibrary.org/dev/docs/api/covers
- Google Books API: https://developers.google.com/books/docs/v1/reference/volumes
- Tauri SQL plugin reference: https://tauri.app/reference/javascript/sql/
- SQLite FTS5: https://www.sqlite.org/fts5.html
- Tauri updater signing: https://tauri.app/plugin/updater/

---

**Practical MVP success criterion:** A person can install the desktop app, add a purchased book (with date, price and condition), mark reading activity, lend it to a named friend, retrieve it, decide whether to sell it, record a sale, and export/restore every detail — all offline.
