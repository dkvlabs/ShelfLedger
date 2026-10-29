# Privacy Policy

ShelfLedger is **local-first**: your library, reading history, contacts, and
financial records live in a SQLite database and media files inside your operating
system's user-data directory on your own computer or Android device.

The Android release has no INTERNET permission and disables Android's automatic
cloud backup. Open Library fetching is disabled on Android; transferred metadata
caches remain usable offline. Development builds allow a local Vite server.

## What we collect

**Nothing.** There is no account, no server, no analytics, no telemetry, no ads,
and no tracking of any kind.

## What leaves your computer

Only what you explicitly trigger:

- **ISBN metadata lookup (Open Library):** when you press "Look up", the app sends
  the ISBN you typed to openlibrary.org with a User-Agent identifying the app.
  Responses are cached locally so repeat lookups work offline. You can disable
  this provider in settings; manual entry always works offline.
- **OLX links:** clicking a comparison search or listing opens OLX in your
  browser, sharing the chosen ISBN/title/author or URL with OLX. Saved comparisons
  remain local; no pages are fetched or monitored in the background.

Contacts, loans, saved valuations and financial records are never automatically
transmitted. There is no automatic message sending.

## Your data

Camera scanning requests video access when you open the barcode scanner. Frames
are processed on your device and are not saved or uploaded. Closing the scanner
releases the camera. Microphone access is not requested. You can use a photo or
manual ISBN entry without camera access.

- **Location:** OS user-data directory (e.g.
  `~/Library/Application Support/io.github.shelfledger.app` on macOS).
- **Portability:** plain SQLite; CSV/JSON export and complete backup/restore from
  Settings. Native exports are copied only to a location chosen in the system
  save dialog. You can transfer a full-library ZIP by USB between PC and Android.
  These archives are not encrypted; they include contacts and financial history.
- **Deletion:** delete the app's data directory to remove everything. Backups you
  create are ordinary files you control.

## Third-party content

Metadata fetched from Open Library remains subject to its own
terms; the app caches it locally for your personal use and does not redistribute
it.

OLX’s own privacy policy applies in your browser. ShelfLedger does not scrape
pages, embed OLX, or send your library or saved comparisons to OLX.

## Changes

Any future change that would add network behavior will be opt-in and documented
here before it ships.
