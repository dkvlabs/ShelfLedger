# Physical Android verification — 31.10.2026

## Follow-up: fresh installation of 0.4.0

After the user removed the old installation, the ARM64 debug APK from Android
CI run `41151651605`, source commit
`0b2e48335e792b9519ae144a8a186b80da971e55`, was installed on the same Pixel.
The downloaded artifact matched its SHA-256 manifest and passed the packaged
permission/OS-backup policy check. It was signed with this Mac's existing Android
debug key, which was not uploaded or committed. Future USB updates can use the
same key. Installed version: **0.4.0**, version code **4000**.

The initial incremental installation produced a blank first-launch screen.
A streamed installation (`adb install --no-incremental -r`) and cold launch
rendered the library successfully. The cause of the first blank screen was not
established. No application data was cleared during these installation checks.

On the actual phone, Settings displayed version 0.4.0, Check for updates,
personal Google Drive sign-in and file backup/restore controls. Pressing Check
for updates completed with “You're up to date,” latest release 0.4.0, and the
notice that the release has no published Android APK. Google authorization and
real cross-device backup remain unverified pending publisher OAuth registration;
no Google account was connected during this installation check.

The detailed core-workflow audit below still describes the earlier 0.2.0 build.

## Earlier 0.2.0 audit

Result: the exercised core workflows passed on the connected phone, but this
does not certify every feature or the latest Android build. One date-default
defect was reproduced, and updating to the current CI APK was blocked by a
signing-certificate mismatch. No application code was changed in this audit.

## Device and build scope

- Pixel 10 Pro XL, ARM64, Android 17 / API 37.
- Android System WebView 156.0.8078.47.
- Installed application: **0.2.0**, version code 2000, debug build.
- Repository checked: **0.4.0**, commit
  `e60c0b6e4ae1bd478bca0657c0ccc8ea7447731e`.
- The successful Android CI run `40934201407` supplied the current ARM64 debug
  APK. Its SHA-256 matched the accompanying checksum file, and
  `scripts/check-android-apk.mjs` passed its debug permission/backup checks.
- `adb install -r` rejected that APK with
  `INSTALL_FAILED_UPDATE_INCOMPATIBLE`: its signing certificate differs from
  the installed app's certificate. The installed app was not uninstalled,
  cleared, or replaced. **Physical functional results below apply to 0.2.0.**

## Device checks

Tests used a generated empty ZIP restored as a separate QA library, a clearly
labeled synthetic book/contact, and a synthetic RON 12.34 purchase. The main
library was kept separate throughout.

| Check | Observed result |
| --- | --- |
| Native ZIP picker and separate restore | Selected an empty test archive; explicit confirmation created a selectable separate library without replacing the main library. |
| Actual offline operation | Airplane mode enabled, Wi-Fi disabled, and Android reported no active default network. Local writes and reads continued. |
| Manual catalog entry | Saved a work, edition, and owned copy with 240 pages and a RON 12.34 acquisition. |
| Search | The saved title was searchable; actual Android keyboard entry filtered the collection correctly. |
| Reading | Started and finished a session; finished history persisted. |
| Loans | Created a synthetic contact, lent the copy, recorded its return, and retained returned-loan history. |
| Money | Finance IPC and the exported SQLite database both contained integer `1234` with currency `RON`. |
| Live physical ISBN scan | With the user holding a book barcode in view, the rear camera decoded `9786067882629`, populated the ISBN field, and closed the scanner. Uncached metadata left manual entry available offline. |
| Camera lifecycle | Live rear-camera video reached 720 × 1280 and ready state 4. Back closed the scanner and changed its retained camera track to `ended`. |
| Android Back | Closed the scanner before Add Book, dismissed the remaining dialog, dismissed the navigation drawer, and returned from a secondary screen to the collection. |
| Native keyboard and layout | Search viewport resized from approximately 523 × 1063 to 523 × 669 CSS pixels. Back dismissed the keyboard, retained the query, and restored the viewport. No horizontal overflow was observed in these states. |
| Offline cold start | Force-stop followed by a cold launch succeeded with no network; native activity startup reported 255 ms. Book, finished reading, returned loan, and exact finance data persisted. This timing is not a full interactive-render benchmark. |
| Export cancellation/retry | Canceling the OS Save dialog left Save file available; retry opened the save dialog and completed with “File saved.” |
| Export integrity | The QA ZIP's database SHA-256 matched its manifest; SQLite integrity check returned `ok`. The archive retained book, copy, money, finished reading, and returned-loan records. |
| Populated ZIP restore | Selected that export through the native picker and explicitly restored it over the active QA profile after checking its identity. The running app retained the expected counts, exact money, and returned-loan history. |
| Crash log | The restarted app's filtered log showed no native crash or application error; Chromium emitted an offline DNS configuration warning. This was not a long-duration crash soak. |

## Reproduced defect: stale acquisition date

At 01:55 on **31.10.2026**, a newly opened Add Book form in the long-running
application still defaulted Acquired on to **2026-10-29**. Saving the synthetic
book earlier had persisted that stale date. After a cold restart on the same
phone, the default became **2026-10-31**.

The current repository still constructs `emptyDraft.acquiredAt` using
`todayIso()` at module evaluation in `src/features/catalog/AddBookDialog.tsx`.
Both initialization and reset reuse that object. A long-running app therefore
reuses the module's original date. This can silently misdate a new acquisition.
Refreshing the default when starting a new draft, with a regression covering a
date rollover without restarting the app, is the recommended follow-up.

## Repository verification

- `pnpm test:web`: **323 passed**, 17 files.
- `pnpm test:rust`: **128 passed, 3 ignored**, no failures when run outside the
  sandbox. The initial sandbox run denied the loopback listener required by an
  HTTP security test; the unrestricted rerun passed.
- `pnpm lint`: passed TypeScript checking and the production Vite build. The
  existing large-chunk warning remains.
- `pnpm test:release`: **20 passed**.

These automated results cover the current 0.4.0 source, rather than proving that
its APK ran on this phone. The local Android NDK was absent; the current APK was
obtained from the successful matching CI run instead of rebuilding locally.

## Cleanup and remaining limits

The original main library was reopened and verified, the generated QA profile
and shared Downloads fixtures were removed, and airplane mode was restored to
disabled with Wi-Fi connected. The app's own internal QA export/staging/safety
artifacts may remain in private storage. No real library records were exported
off the phone. Automatic approval review rejected an attempted private-app-data
backup; that backup was not performed. The later inspected export contained only
the generated QA records.

The latest APK still needs a device test using a compatible signing identity.
Release-build-only permission behavior, camera denial/recovery, cover capture
and photo import, alternate document providers, orientation changes,
accessibility/font scaling, every language, and extended large-library use were
not physically verified in this session.
