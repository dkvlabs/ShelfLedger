# Architecture

> Status: matches Phase 0–1 implementation. Update alongside code.

## Overview

ShelfLedger is a single desktop application. There is **no server, no cloud, no account,
no telemetry**. All state lives in a SQLite database inside the OS user-data directory.

```text
React UI (src/features/*, src/components/*)
  → typed frontend client (src/lib/api.ts — named commands + DTOs only)
    → Tauri IPC commands (src-tauri/src/commands — thin, no business logic)
      → domain services (src-tauri/src/domain — invariants, transactions, events)
        → repositories (src-tauri/src/repositories — all SQL)
        → metadata adapters (src-tauri/src/metadata — Open Library, cached)
          → SQLite (sqlx, migrations, foreign_keys=ON, WAL) + local file storage
```

## Stack decisions (summary; see docs/decisions/)

- **Tauri 2 + Rust commands** instead of letting the webview run SQL: one backend layer
  enforces invariants (valid ISBN checksums, unique ISBNs, valid references, one
  acquisition per copy in v1), keeps file access inside the app-data directory, and
  controls schema changes. The webview never sends SQL — only typed DTOs.
- **SQLite via sqlx** with embedded migrations (`sqlx::migrate!`), foreign keys enabled at
  connection level, WAL journaling. FTS5 virtual table for full-text search, kept in sync
  by repository code on work/author/edition writes.
- **React + TypeScript + Vite + Tailwind + shadcn/ui** with TanStack Query for the data
  cache. DTO types are hand-mirrored in `src/types/domain.ts` — both sides must agree.
- **Money**: integer minor units + ISO 4217 currency code everywhere. No floats. UI
  converts user decimal input at the edge (`src/lib/format.ts`) and Rust validates.
- **Dates**: ISO 8601 (UTC) in storage; `DD.MM.YYYY` in the UI.

## Error model

Rust commands return `Result<T, AppError>`; `AppError` serializes as
`{ kind: "validation" | "not_found" | "conflict" | "network" | "database" | "io" | "other", message }`.
The frontend wraps invocations in `CommandError` so the UI can branch on `kind`
(e.g. network failures during ISBN lookup fall back to manual entry).

## Network behavior

The only network feature in Phase 0–1 is the user-initiated ISBN metadata lookup
(Open Library):

- identified with a proper User-Agent including a contact URL,
- 10-second timeout,
- responses cached in the `metadata_cache` table,
- fully disable-able via the `metadata.lookup_enabled` setting,
- degrades gracefully: offline → `AppError::Network` → manual entry.

## Security posture

- Tauri capabilities are least-privilege (`src-tauri/capabilities/default.json`): core
  window/event access only. No filesystem, shell, or HTTP permissions are granted to the
  webview.
- All writes go through validated domain services; SQL text never crosses the IPC
  boundary.
- Third-party metadata (Open Library) and cover images are used under their own terms;
  covers are not bundled or redistributed with the app.

## Module status (Phase 1)

| Module | State |
| --- | --- |
| catalog (works/editions/copies, search, duplicates, ISBN lookup) | implemented |
| inventory (locations hierarchy, condition, notes) | implemented |
| reading / loans / finance / imports | not implemented (Phase 2–4) |
| backups | not implemented (Phase 4) |
