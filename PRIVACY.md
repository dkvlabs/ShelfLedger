# Privacy Policy

ShelfLedger is **local-first**: your library, reading history, contacts, and
financial records live in a SQLite database and media files inside your operating
system's user-data directory on your own computer or Android device.

The Android release has no INTERNET permission and disables Android's automatic
cloud backup. All online metadata fetching is disabled on Android; transferred metadata
caches remain usable offline. Development builds allow a local Vite server.

## What we collect

**Nothing.** There is no account, no server, no analytics, no telemetry, no ads,
and no tracking of any kind.

## What leaves your computer

Only what you explicitly trigger:

- **ISBN metadata lookup:** when you press "Look up", the app sends the ISBN
  you typed to openlibrary.org, then www.googleapis.com (Google Books) if Open
  Library has no match or fails. Only checksum-valid matching editions are used.
  Requests identify ShelfLedger in the User-Agent. Successful responses are
  cached locally so repeat lookups work offline. No requests run while typing.
- **Libris metadata import:** only when you press "Import from Libris", the app
  fetches the supplied HTTPS book page from www.libris.ro. It reads factual Book
  metadata (ISBN, title, authors, publisher, publication date, language, pages),
  checks an entered ISBN against it, and caches the result locally. It does not
  load images, execute page scripts, fetch tracking resources, or import reviews
  and prices from the page.
  Manual entry always works offline. The `metadata.lookup_enabled` setting
  disables all provider requests; Android enforces this regardless of settings.
- **OLX links:** clicking a comparison search or listing opens OLX in your
  browser, sharing the chosen ISBN/title/author or URL with OLX. Saved comparisons
  remain local; no pages are fetched or monitored in the background.
- **Cover fetch:** only when you press "Fetch cover", the edition ISBN is sent
  to covers.openlibrary.org. Its image may be served through archive.org and
  Internet Archive image servers. Saved images display from SQLite with no
  subsequent provider request. Android and disabled metadata settings block
  this fetch. There are no background or bulk cover downloads.

Contacts, loans, saved valuations and financial records are never automatically
transmitted. There is no automatic message sending.

## Your data

Camera scanning requests video access when you open the barcode scanner. Frames
are processed on your device and are not saved or uploaded. Closing the scanner
releases the camera. Microphone access is not requested. You can use a photo or
manual ISBN entry without camera access.

Cover photography separately requests video only after **Photograph cover** is
opened. It stops the camera on preview or close. Only **Save photo** stores the
captured image. Selected/captured photos stay local and are never uploaded to a
provider. JPEG/PNG/WebP input is normalized to a bounded JPEG, correcting phone
orientation and removing EXIF/GPS metadata. JSON and ZIP exports include saved
cover images, so anyone receiving an export can see those images too.

- **Location:** OS user-data directory (e.g.
  `~/Library/Application Support/io.github.shelfledger.app` on macOS).
- **Portability:** plain SQLite; CSV/JSON export and complete backup/restore from
  Settings. Native exports are copied only to a location chosen in the system
  save dialog. You can transfer a full-library ZIP by USB between PC and Android.
  These archives are not encrypted; they include contacts and financial history.
- **Deletion:** delete the app's data directory to remove everything. Backups you
  create are ordinary files you control.

## Third-party content

Metadata fetched from Open Library, Google Books, or Libris remains subject to
the respective provider’s terms. The app caches factual metadata locally for
your personal use; your explicit library exports include these caches.

OLX’s own privacy policy applies in your browser. ShelfLedger does not fetch
OLX pages, embed OLX, or send your library or saved comparisons to OLX.

## Changes

Any future change that would add network behavior will be opt-in and documented
here before it ships.
