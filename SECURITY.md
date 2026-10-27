# Security Policy

## Supported versions

ShelfLedger is pre-1.0; only the latest commit on `main` is supported.

## Reporting a vulnerability

Please report security issues privately rather than opening a public issue:

1. Use GitHub's "Report a vulnerability" feature under the repository's Security tab, or
2. Contact the maintainer directly (see the repository profile).

You will receive a response within 7 days. We will coordinate a fix and disclosure timeline
with you and credit reporters in the changelog unless they prefer anonymity.

## Security model

ShelfLedger is a **local-first desktop app**:

- All data lives in a SQLite database plus media files inside the OS user-data directory.
- There is no server, no account system, no telemetry, and no analytics.
- Network requests are limited to **user-initiated** metadata lookups (Open Library by
  default) with explicit settings to disable them.

## What we ask contributors to watch for

- **Path traversal:** any code writing attachments/media must stay inside the app data
  directory; reject `..` and absolute-path components.
- **No arbitrary SQL from the UI:** the webview may only call typed Tauri commands; SQL is
  never accepted as IPC input.
- **Least privilege:** Tauri capabilities in `src-tauri/capabilities/` must grant only what
  a feature needs.
- **No secrets in the repo:** signing keys, API keys, or credentials must never be
  committed. Credentials come from environment variables or OS key services only.
- **Cover/metadata licensing:** do not bundle scraped cover images or redistribute
  third-party metadata in violation of its terms.
