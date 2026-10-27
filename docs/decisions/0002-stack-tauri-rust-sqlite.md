# ADR 0002 — Stack: Tauri 2, Rust domain layer, SQLite via sqlx

**Status:** Accepted (2026-10)

## Context

The app must be a low-maintenance, offline-first desktop product for macOS/Windows/Linux
with a trustworthy local database. The alternative considered was letting the webview run
SQL directly (e.g. via tauri-plugin-sql), which is fewer moving parts initially.

## Decision

- **Tauri 2** shell; React + TypeScript + Vite + Tailwind + shadcn/ui frontend.
- All persistence behind **typed Rust Tauri commands** → domain services → repositories.
- **SQLite via sqlx** with embedded migrations, foreign keys ON, WAL, FTS5.

## Rationale

A single backend layer enforces the invariants the product depends on: ISBN checksum
validation, unique ISBNs, valid references, one acquisition per copy (v1), correct
transactions, file access limited to the app-data directory, and controlled schema
changes. Unrestricted SQL in the webview would push these invariants into the UI, where
they are hard to test and easy to bypass. sqlx migrations give reproducible schema state
for backup/restore and future upgrades.

## Consequences

- Rust code must mirror DTOs to TypeScript by hand (kept in sync per AGENTS.md).
- CI needs per-OS system deps for the Tauri Linux build.
- Metadata lookups live in Rust adapters with caching, so the webview needs no network
  permission at all.
