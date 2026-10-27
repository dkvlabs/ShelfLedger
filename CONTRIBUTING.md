# Contributing to ShelfLedger

Thanks for helping build a local-first book library app. This project keeps things simple on
purpose: one desktop app, one SQLite database, no cloud.

## Getting started

1. Install Node.js ≥ 20, pnpm ≥ 9, and Rust ≥ 1.77, plus the
   [Tauri 2 prerequisites](https://tauri.app/start/prerequisites/) for your platform.
2. `pnpm install`
3. `pnpm tauri dev` — you should see the empty-state welcome screen.

## How we work

- **Implement only the current phase.** The phase plan lives in `docs/implementation.md`;
  each phase has checkboxes and an acceptance criterion. Do not silently jump ahead.
- **Respect the domain model.** A *work* (literary identity), an *edition* (published
  version), and a *copy* (owned item) are different tables. Reading status, ownership
  status, and loan state are separate concepts — never collapse them into one column.
- **Money is integer minor units + ISO 4217 currency code.** No floats. Never sum
  different currencies.
- **Offline-first.** Network features (metadata lookup) must degrade gracefully and are
  user-initiated only.
- **No fabricated data.** Never invent prices, valuations, or metadata the user didn't
  provide or fetch from a provider.

## Before opening a PR

- `pnpm lint` (typecheck + build) passes
- `pnpm test:web` passes
- `pnpm test:rust` passes
- New behavior has tests (see the test matrix in `docs/implementation.md`)
- Rust code is formatted (`cargo fmt`) and clippy-clean (`cargo clippy -- -D warnings`)

## Good first issues

Issues tagged `good first issue` and `help wanted` are scoped to be approachable. If the
codebase is new to you, `docs/database.md` and `docs/architecture.md` are the fastest reads.

## Reporting security issues

See [SECURITY.md](SECURITY.md). Please do not open public issues for security problems.

## Licensing

By contributing you agree your contributions are licensed under the MIT license of this
repository.
