# AGENTS.md — guidance for coding agents working on ShelfLedger

Read `docs/architecture.md`, `docs/database.md`, `docs/design.md`, `docs/implementation.md`
before writing code.

## Ground rules

1. **Implement only the current phase** (see `docs/implementation.md`). Do not silently
   implement later phases; if a later-phase stub is needed for compilation, mark it with
   `// PHASE(n):` and keep it inert.
2. **Domain model is sacred.** Three distinct objects:
   - `work` — the literary identity (title, authors).
   - `edition` — a published version of a work (ISBN, publisher, page count).
   - `copy` — a physical item someone owns (condition, location, acquisition, disposal).
   Reading status (want to read / reading / finished / DNF) is **separate** from ownership
   status (owned / lent / listed / sold / …) and from loan records. Never merge them into
   one status column.
3. **Money:** integer minor units + ISO 4217 currency code stored alongside. No floats.
   Never sum across currencies. Never invent exchange rates.
4. **Offline-first:** every important interaction works without network. Metadata lookups
   (Open Library) are user-initiated, cached, and must degrade to manual entry.
5. **No fabricated data:** never generate prices, valuations, or metadata the user did not
   supply or fetch. Estimates show source, date, and confidence or they are not shown.
6. **Typed IPC only:** the webview calls named Tauri commands with validated DTOs. SQL
   never crosses the IPC boundary. Validate all inputs (Rust side) before writes.
7. **Least privilege:** keep `src-tauri/capabilities/` minimal; add permissions only when a
   feature needs them.
8. **Paths:** all file writes stay inside the OS app-data directory. Reject traversal.

## Code conventions

- Rust: `cargo fmt` + `cargo clippy -- -D warnings` clean; errors via `AppError`; domain
  logic in `src-tauri/src/domain`, SQL in `src-tauri/src/repositories`, thin commands in
  `src-tauri/src/commands`.
- Frontend: TypeScript strict; feature folders under `src/features/*`; shared UI in
  `src/components/ui`; typed command wrappers in `src/lib/api.ts`; DTO types in
  `src/types/domain.ts` (kept in sync with Rust DTOs by hand — both sides must agree).
- Dates: ISO 8601 (UTC) in storage, `DD.MM.YYYY` in the UI (`src/lib/format.ts`).
- IDs: UUID v4 generated in Rust.

## Testing

Every behavior change needs tests. Rust tests live next to the code (`#[cfg(test)]`) and
cover money, ISBN checksums, migrations/FKs, invariants, and duplicate detection; Vitest
covers parsing/formatting utilities. Run: `pnpm test:rust`, `pnpm test:web`, `pnpm lint`.

## Reporting

Report failures honestly. Do not claim a feature works unless its tests or a manual check
demonstrate it.
