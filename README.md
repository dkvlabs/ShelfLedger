# ShelfLedger

An **offline-first, local-first, open-source desktop app** to catalog books, track reading,
own and manage multiple physical copies, record acquisition costs, lend books to friends,
and make informed decisions about keeping or selling each copy.

**Your data belongs to you.** No account, cloud, or subscription required. The library is a
readable SQLite database in your user data directory.

> Project status: **Phases 0–5 implemented** (catalog, reading, loans, resale &
> money, import/export/backup, theme + EN/RO). See
> [docs/implementation.md](docs/implementation.md) for per-phase status and
> [docs/release.md](docs/release.md) for the packaging checklist. Phase 6 (barcode
> scanning, e-books, sync) is deliberately not started.

## Why ShelfLedger?

Goodreads-style apps track reading activity; library apps track possession. ShelfLedger tracks
the complete **per-copy lifecycle** — from acquisition, to storage, to reading, to lending,
to selling or donating — with truthful, clearly-separated financial fields (cost, estimate,
asking price, realized sale) and zero lock-in.

## Key product principles

1. **Copy-first inventory:** literary *work*, published *edition*, and owned *copy* are distinct objects.
2. **Offline and private by default:** everything important works without internet.
3. **Truthful values:** purchase cost, estimated value, asking price, and actual sale price are distinct fields. We never fabricate price data.
4. **Zero lock-in:** CSV/JSON export, complete backup/restore, plain SQLite, stable IDs and migrations.
5. **Fast entry:** ISBN lookup (Open Library, cached), manual entry, duplicate detection.
6. **Low maintenance:** single desktop app; no server, Docker, or hosted database.

## Platforms

macOS, Windows, Linux (Tauri 2).

## Tech stack

- **Shell:** Tauri 2 (Rust backend, typed IPC commands)
- **UI:** React + TypeScript + Vite + Tailwind CSS + shadcn/ui + TanStack Query/Table
- **Data:** SQLite via `sqlx` with migrations, foreign keys, WAL, and FTS5 full-text search
- **Money:** integer minor units + ISO 4217 currency (never floats)

## Development

Prerequisites: [Node.js ≥ 20](https://nodejs.org), [pnpm ≥ 9](https://pnpm.io),
[Rust ≥ 1.77](https://rustup.rs), plus the [Tauri 2 prerequisites](https://tauri.app/start/prerequisites/)
for your OS.

```sh
pnpm install
pnpm tauri dev       # run the desktop app in dev mode
pnpm test:rust       # Rust unit/integration tests (cargo test)
pnpm test:web        # frontend tests (Vitest)
pnpm lint            # typecheck + build
```

## Documentation

- [docs/architecture.md](docs/architecture.md) — module boundaries and stack rationale
- [docs/database.md](docs/database.md) — schema, constraints, currency/date policies
- [docs/design.md](docs/design.md) — UI principles, screens, design tokens
- [docs/implementation.md](docs/implementation.md) — phased plan and current status
- [docs/import-format.md](docs/import-format.md) — CSV/JSON import schema
- [docs/release.md](docs/release.md) — release/signing checklist
- [docs/decisions/](docs/decisions/) — architecture decision records
- [PRIVACY.md](PRIVACY.md) — what leaves your computer (almost nothing)
- [demo/demo-library.json](demo/demo-library.json) — sample dataset (import via Settings)

## License

[MIT](LICENSE). Third-party metadata (Open Library) and cover images are used under their own
terms and are not redistributed with the app; see [docs/architecture.md](docs/architecture.md).
