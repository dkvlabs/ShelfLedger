# Privacy Policy

ShelfLedger is **local-first**: your library, reading history, contacts, and
financial records live in a SQLite database and media files inside your operating
system's user-data directory on your own computer.

## What we collect

**Nothing.** There is no account, no server, no analytics, no telemetry, no ads,
and no tracking of any kind.

## What leaves your computer

Only what you explicitly trigger:

- **ISBN metadata lookup (Open Library):** when you press "Look up", the app sends
  the ISBN you typed to openlibrary.org with a User-Agent identifying the app.
  Responses are cached locally so repeat lookups work offline. You can disable
  this provider in settings; manual entry always works offline.
- **Nothing else.** Contacts, loans, valuations, and financial data are never
  transmitted. There is no automatic message sending, ever.

## Your data

Camera scanning requests video access when you open the barcode scanner. Frames
are processed on your device and are not saved or uploaded. Closing the scanner
releases the camera. Microphone access is not requested. You can use a photo or
manual ISBN entry without camera access.

- **Location:** OS user-data directory (e.g.
  `~/Library/Application Support/io.github.shelfledger.app` on macOS).
- **Portability:** plain SQLite; CSV/JSON export and complete backup/restore from
  Settings.
- **Deletion:** delete the app's data directory to remove everything. Backups you
  create are ordinary files you control.

## Third-party content

Metadata and cover images fetched from Open Library remain subject to their own
terms; the app stores them locally for your personal use and does not redistribute
them.

## Changes

Any future change that would add network behavior will be opt-in and documented
here before it ships.
