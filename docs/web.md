# Web version

ShelfLedger also runs in a normal browser: the **same Rust core** (same domain
logic, same SQLite database, same typed command contract) served over loopback
HTTP by the `shelfledger-web` binary. No cloud, no accounts — the browser talks
to a server on `127.0.0.1` and the library stays on your machine.

This is a local-first alternative front door to the app, useful when you prefer
a browser tab or your platform's webview lacks a feature (see the camera
capability matrix below).

## Running it

```sh
pnpm build          # build the frontend into dist/
pnpm web:server     # start the local server (cargo run --bin shelfledger-web)
# → open http://127.0.0.1:8787
```

For frontend development, run `pnpm web:server` in one terminal and `pnpm dev`
in another; Vite proxies `/api` to the server.

Environment variables (all optional):

| Variable | Default | Meaning |
| --- | --- | --- |
| `SHELFLEDGER_DATA_DIR` | `./shelfledger-data` | Where the web version keeps its library (SQLite + uploads/exports/backups) |
| `SHELFLEDGER_WEB_DIST` | `./dist` | Built frontend to serve |
| `SHELFLEDGER_WEB_PORT` | `8787` | Loopback port |

The web version keeps its own database directory by default, separate from the
desktop app. To point both at the same library, set `SHELFLEDGER_DATA_DIR` to
the desktop app-data directory (SQLite WAL allows multi-process access; take a
backup first — restore semantics across two frontends are on you).

## How it works

- One command contract, two transports. `src/lib/api.ts` calls Tauri `invoke()`
  in the desktop app and `POST /api/<command>` in the browser; the argument
  object and the `{kind, message}` error shape are identical. Named commands
  are dispatched by `src-tauri/src/web.rs` onto the same `domain/` functions
  the Tauri commands use — no business logic is duplicated.
- File flows: the browser file picker uploads the chosen file to
  `/api/upload`, which stores it under a generated name inside the data
  directory and hands the commands a normal path. Exports and backups land in
  `exports/` and `backups/`; `/api/download` serves only those two directories
  (traversal is rejected). This keeps the AGENTS.md rule intact: every write
  stays inside the app data directory.
- The server binds `127.0.0.1` only. There is no remote access and no
  telemetry.

## Barcode scanning capability matrix

Scanning uses web APIs only, so the same component runs in the webview and in
browsers. Where live camera access is not offered by the platform, the photo
path covers scanning.

| Environment | Live camera | Scan from photo | USB/Bluetooth scanner |
| --- | --- | --- | --- |
| Web version in Chrome / Edge | ✅ (BarcodeDetector) | ✅ | ✅ |
| Web version in Firefox / Safari | ✅ (ZXing JS fallback) | ✅ | ✅ |
| Desktop, Windows (WebView2) | ✅ when the platform exposes the camera API | ✅ | ✅ |
| Desktop, macOS / Linux (WebKit webview) | Platform-dependent; falls back when the camera API is unavailable | ✅ | ✅ |

Opening Scan barcode requests **video only** and uses the platform's camera
permission flow. Allow access when prompted. If access is blocked, allow the
camera in browser site permissions (web version) or system camera privacy
settings, then use **Retry camera**. On macOS, use **System Settings → Privacy
& Security → Camera** and restart the app if necessary. Missing and busy
cameras have separate recovery messages. Closing the scanner releases the camera.

The macOS bundle includes `NSCameraUsageDescription` in `src-tauri/Info.plist`
and the camera entitlement in `src-tauri/Entitlements.plist`. Rebuild the app
to include these declarations; an already installed build does not gain them
automatically. Local macOS builds are ad-hoc signed to embed the entitlement;
public distribution still needs Developer ID signing/notarization. These declarations allow the OS consent flow where the webview
offers camera capture; they cannot provide a missing webview API. Browsers
require HTTPS or localhost for camera access.

"Scan from photo" decodes any photo/screenshot containing the barcode; a
USB/Bluetooth barcode scanner acts as a keyboard and types the ISBN into the
field. ISBN extraction validates checksums and only accepts book-range
barcodes (978/979 EAN-13 or ISBN-10) — nothing is invented.

## Known limitations

- Replacement restore uses a transaction on the live database. It takes a
  consistent safety backup first and does not require a restart. Use one running
  frontend when switching profiles or restoring.
- The web version has not been exposed to the internet by design; do not
  reverse-proxy it without adding your own authentication.

## Request protection (27.10.2026)

GET `/api/session` establishes a process-session token. All API POSTs, including
uploads, require `X-Shelfledger-Token`. Commands additionally require
`Content-Type: application/json`. Loopback Host and same-origin Origin are checked;
only debug builds accept the documented Vite origin. Cross-origin-shaped simple
POSTs, untrusted hosts/origins and missing tokens are rejected by HTTP regression
tests. These checks do not make this a remotely hosted multi-user service.
