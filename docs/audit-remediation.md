# Audit remediation — 27.10.2026

The October audit identified 24 findings. The application changes below address
23 findings; F20 retains two upstream advisory warnings. Release certification
is separate: Windows/Linux execution, physical camera consent and a signed,
notarized public release have not been demonstrated in this workspace.

## Finding-by-finding record

| Finding | Resolution | Evidence |
| --- | --- | --- |
| F01 — live WAL restore loses writes | Replace records transactionally through the existing pool; create a consistent safety archive first. | Public-command regression restores A, then writes C, reopens, and verifies A+C; safety archive restores A+B. |
| F02 — incomplete JSON interchange | Versioned logical snapshots include all 22 data tables, archived inventory and reading/lending/financial history. Internal profile identity stays local. | Complete-history export/import regression, including comma-containing author names and bad-reference rollback. Binary attachments remain a documented deferred feature. |
| F03 — partial failed imports | Stage each legacy work/CSV row and merge atomically; report outcomes matching persisted data. | Invalid second-copy import leaves no work, edition or first copy. |
| F04 — unprotected loopback requests | Check Host and Origin, require process-session token for POSTs and JSON content type for commands. | Actual ephemeral HTTP tests reject untrusted origin/host, missing token and text/plain. |
| F05 — output escapes app data | Canonical ancestor checks, traversal/symlink rejection, generated temporary paths and bound VACUUM path. | Path confinement regression and backup/restore integration tests. |
| F06 — Goodreads history loss | Separate Exclusive Shelf from custom shelves, preserve reviews/ratings, keep unknown dates null, count duplicate skips. | Standard CSV fixture, Unicode/invalid date, ISBN fallback, tags/review/null-date and row-log regressions. |
| F07 — valuation double counting | Select one latest observation by observed date, creation time and row sequence. | Same-date observations select only the last amount in reports and the sell queue. |
| F08 — disposal revives ownership | Transactional disposal closes all active listings; withdrawal checks active listing/state. One-active-listing index repairs old duplicates. | List → donate → withdraw rejection preserves donated state. |
| F09 — wrong currency scale | Shared ISO currency digit metadata, exact safe-integer parsing and currency-aware display. | CAD/CHF, JPY and KWD tests, excessive precision and safe-integer limits. |
| F10 — duplicate dashboard currency buckets | Aggregate years by currency before returning dashboard totals. | Multi-year CAD regression; mixed currencies stay separate. |
| F11 — reading workflow dead end | Session editor exposes start/resume, all statuses, dates, rating and review. | Component journey from want-to-read to reading; public reading/reread/DNF tests and browser start-reading flow. |
| F12 — New contact sentinel | Treat both initial empty and explicit new selection as creation; validate entered name. | Browser flow selects existing contact, switches to New, and successfully lends to the new reader. |
| F13 — missing catalog editing | Typed edition/copy detail commands and work/edition/copy editors; editable tags. | Public-command edit regression verifies saved values, identity, archive rejection and search. Browser work editor saves. |
| F14 — stale copy/location search | Rebuild affected FTS entries in each relevant mutation transaction. | Public commands test create/update/archive/unarchive/location deletion and old/new search results. |
| F15 — inconsistent domain records | Rust validates ISBN identity/checksum, dates/pages, money/currency, reading edition/work and loan dates. Snapshot import validates fields and references. | Invalid inputs never persist; cross-work reading is rejected. |
| F16 — nullable patches overwrite fields | Distinguish omitted values from explicit null in patch DTOs. | Omitted fields remain; explicit null clears; FTS reflects actual values. |
| F17 — archived inventory looks active | Hide archived copies by default; explicit history toggle, confirmation and restoration action. | Archive/unarchive domain and search regressions; active counts exclude archives. |
| F18 — incomplete import/profile workflow | CSV mapping, row selection/policies, review/confirmation and row results; separate restore creates selectable profiles with preserved per-profile changes. | Mapped CSV writes only selected rows; invalid mapping rejection; profile switch/back and restore-identity tests. |
| F19 — localization/date gaps | EN/RO dictionaries cover feature screens/dialogs and state labels; shared date formatting. | Browser Romanian settings and library; formatting tests. User/provider strings and diagnostic errors remain verbatim. |
| F20 — dependency advisories | Upgrade Zod/resolvers, Vitest and ZIP; use SQLite-only SQLx dependencies to remove unused vulnerable RSA chain. | Final pnpm audit: zero vulnerabilities. Cargo audit: zero vulnerability matches, two remaining warnings described below. |
| F21 — prerequisites mismatch | Rust 1.90, Node ≥22.12, pnpm 12.15 source of truth; CI adds minimum Rust check. | Local build on Rust 1.99.1/Node 24.22; minimum/cross-OS jobs configured, not executed here. |
| F22 — selected import file discarded | Settings sends a typed file payload through shell state into preview. | Real Settings → app shell → import component regression; preview once, no commit before confirmation. |
| F23 — theme/record identity | Subscribe to OS theme changes with cleanup; show work/edition/copy identity in inventory dialogs. | Theme listener tests and lending identity browser check. |
| F24 — contradictory docs | Update architecture, schema, design, portability, web instructions and roadmap; distinguish implementation from acceptance/release evidence. | Documentation review; Phase 6 remains explicitly deferred. |

## Verification

- Rust tests: **88 passing**, including actual loopback security tests and a
  100-book create/reopen/search soak check using disposable local data.
- Frontend tests: **59 passing** (camera permission/lifecycle, workflows,
  Settings-to-import integration, formats, transport and theme).
- Strict TypeScript/production frontend build and Clippy with warnings denied pass.
- macOS `.app` bundle builds locally. Local builds use ad-hoc signing so the
  configured camera entitlement is applied; Developer ID/notarization is a
  separate release step. The original bundle identifier is retained to avoid
  moving existing users' app-data directories.
- Browser checks use an isolated data directory: manual add, reading transition,
  work editing, lend/return/new-contact switch, EN/RO and mobile navigation.
  Settings file selection reaches mapped preview, confirmation saves one work,
  and switching to an empty profile and back preserves both original works.
  Desktop (1280×720) and phone (390×844) have no horizontal overflow; the phone
  add-book dialog fits the viewport and scrolls its contents. Final browser
  console checks reported no warnings or errors.
  The finished macOS bundle passes strict signature verification and contains
  `com.apple.security.device.camera = true`.

## Remaining upstream warnings and release evidence

The locked Linux GTK chain still includes `glib 0.18.5`, with
`RUSTSEC-2024-0429` (unsound `VariantStrIter` implementations; patched upstream
in 0.20). `proc-macro-error 1.0.4` carries `RUSTSEC-2024-0370` (unmaintained).
These are not suppressed. Moving GTK bindings across incompatible major APIs
without Linux builds is not a verified fix; reachable use of the affected glib
iterator has not been established. Keep these warnings visible and resolve them
with a compatible upstream Tauri/GTK upgrade or a Linux-tested backport.

The RustSec advisory database was refreshed (latest commit 26.10.2026); package
audit queried the registry. Secret scanning is configured in CI and was
not run locally. No private release signing/notarization credentials were used.
No clean-machine installation, Windows/Linux native execution, physical webcam
capture or OS consent prompt was claimed. Follow `release.md` for these gates.

Phase 6 remains deferred: receipt/OCR/binary attachment portability, automated
marketplace collection, ebooks/devices and optional encrypted sync. Snapshot files preserve
attachment records/relative paths, not attachment bytes; do not represent this as
an implemented receipt archive.

Manual OLX comparisons were subsequently pulled forward on explicit request;
see [market-comparisons.md](market-comparisons.md). They retain user-checked
asking prices and source evidence; automatic collection remains deferred.
