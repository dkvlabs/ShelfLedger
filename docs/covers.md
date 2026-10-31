# Book covers

Covers belong to an **edition**, so different editions of one work can keep
different covers. Open a book from the library and find its edition card.

- **Fetch cover** requests the edition's ISBN from [Open Library Covers](https://openlibrary.org/dev/docs/api/covers).
  It replaces the saved cover only after a valid image is received and stored.
  A missing cover, provider failure, or changed ISBN preserves the previous image.
  Availability is not guaranteed, including for Romanian editions. Check that
  the supplied image matches your book; choose your own photo if it does not.
- **Choose cover photo** opens the image picker. JPEG, PNG and WebP files up to
  10 MiB and 32 megapixels are accepted, with a maximum side of 8192 pixels.
  A valid selection is saved immediately. For HEIC/other formats, export as JPEG first.
- **Photograph cover** asks for video-only camera access. Frame the front cover,
  select **Take photo**, inspect the preview, then **Save photo** or **Retake photo**.
  Cancelling the preview saves nothing. Closing or previewing releases the camera.
  If access is denied or unavailable, choose a photo instead.
- **Remove cover** removes the image from this edition. The library returns to
  its title/author treatment when the work has no other saved edition cover.

All saved images are re-encoded as JPEG, resized only when the longest side
exceeds 1200 pixels. Phone orientation is applied; EXIF/GPS and other metadata
are not retained. Transparency is flattened onto white. Source and saved date
appear beneath the cover. Saving a different image replaces that edition's cover.
Changes do not alter existing backup files.

## Offline and transfer

Saved covers display locally. Viewing a collection, opening a book, choosing a
photo and using the camera never requests a remote cover. Fetch is an explicit
action on Android/desktop/web and obeys the metadata provider setting. Choosing
or taking a photo and transferring a saved cover remain available offline.

Normalized JPEG bytes live in SQLite, inside app data. Logical cover identifiers
are not file paths, and arbitrary remote image URLs are never rendered. Full
JSON exports (format 4) and library ZIP backups include the image bytes. Use
**Settings → Export for another device** to transfer covers with your library.
Older complete format 1–3 JSON files and older ZIP databases remain importable.
CSV does not carry images. Exports are not encrypted; share them accordingly.

## Verification

Rust regressions cover image format/size limits, EXIF orientation and stripping,
atomic replacement/removal, failed or disabled fetches, provider redirect bounds,
JSON image validation and rollback, legacy JSON, and ZIP restore. Frontend tests
cover file selection, saved image/source display, no background provider calls,
disabled/offline guidance, camera confirmation/retake, binding the camera stream
after the dialog portal mounts, and cleanup after pending consent. Browser QA
exercised real ISBN cover download, photo upload, reload persistence, simulated
camera capture/retake/save, and desktop/mobile layouts.
Real packaged-device camera consent still needs an Android/macOS smoke test.
