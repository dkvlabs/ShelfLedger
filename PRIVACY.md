# Privacy Policy

ShelfLedger is **local-first**: your library, reading history, contacts, and
financial records live in a SQLite database and media files inside your operating
system's user-data directory on your own computer or Android device.

The Android release disables Android's automatic cloud backup. Its INTERNET
permission supports explicit update checks and optional Google Drive sync. All online metadata fetching is disabled on Android; transferred metadata
caches remain usable offline. Development builds allow a local Vite server.

## What we collect

ShelfLedger operates no account server and collects no analytics or telemetry.
There are no ads or tracking. Optional Google Drive sync uses your Google account
and Google's services; file backups use the provider you choose.

## What leaves your computer

Only what you explicitly trigger:

- **Google Drive sync:** sign-in opens Google's consent flow. Choosing/creating a
  cloud library accesses its name and identifiers. **Sync now**, conflict choices
  and restoring cloud history transfer complete library snapshots, including
  contacts, loans, finances, saved covers and metadata caches, to/from Google's
  app-specific Drive storage over HTTPS. Device preferences remain local. Only
  `drive.appdata` permission is requested; the app cannot browse ordinary Drive
  files. Access tokens stay in native memory, expire, and are never included in
  exports. No background sync runs. Disconnect clears the local link and token;
  it does not delete cloud history or revoke Google's authorization. Manage
  authorization/data through your Google account. Cloud snapshots have no
  additional ShelfLedger encryption. See [setup and recovery](docs/cloud-sync.md).
- **Cloud file backups:** the system file picker can save/open ZIPs through
  iCloud Drive or another installed provider. That provider manages transfer and
  account access. Backups include preferences, contacts and financial history
  and are not encrypted by ShelfLedger. Picking a restore file does not apply it
  until you confirm a restore choice.

- **Update checks:** pressing "Check for updates" in Settings requests the latest
  stable release from api.github.com. GitHub receives your IP address and an
  application User-Agent containing the installed version. No library records,
  identifiers or credentials are sent. No check runs at startup or in the
  background. Download buttons open the official GitHub release or Android
  builds page in your browser; downloads and installation are manual.

- **ISBN metadata lookup:** when you press "Look up", the app sends the ISBN
  you typed to openlibrary.org, then www.googleapis.com (Google Books) if Open
  Library has no match or fails, then isbnsearch.org (ISBN Search) if both
  catalogs miss or fail. Only checksum-valid matching editions are used.
  Requests identify ShelfLedger in the User-Agent. Successful responses are
  cached locally so repeat lookups work offline. No requests run while typing.
  ISBN Search reads only the book information on its public ISBN page; it does
  not execute scripts or load ads, tracking, images, offers or prices.
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
captured image. Selected/captured photos stay local unless included in an explicit
export, cloud backup or Drive sync. JPEG/PNG/WebP input is normalized to a bounded JPEG, correcting phone
orientation and removing EXIF/GPS metadata. JSON and ZIP exports include saved
cover images, so anyone receiving an export can see those images too.

- **Location:** OS user-data directory (e.g.
  `~/Library/Application Support/io.github.shelfledger.app` on macOS).
- **Portability:** plain SQLite; CSV/JSON export and complete backup/restore from
  Settings. Native exports are copied only to a location chosen in the system
  save dialog. You can transfer a full-library ZIP by USB between PC and Android.
  These archives are not encrypted; they include contacts and financial history.
- **Deletion:** deleting the app's data directory removes local data. Copies in
  Google Drive, iCloud or other exported locations must be removed separately.

## Third-party content

Metadata fetched from Open Library, Google Books, ISBN Search, or Libris remains subject to
the respective provider’s terms. The app caches factual metadata locally for
your personal use; your explicit library exports include these caches.

OLX’s own privacy policy applies in your browser. ShelfLedger does not fetch
OLX pages, embed OLX, or send your library or saved comparisons to OLX.

## Changes

Any future change that would add network behavior will be opt-in and documented
here before it ships.
