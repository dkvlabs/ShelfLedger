# Android: offline library and device transfer

ShelfLedger for Android uses the same React screens, Rust domain and SQLite
schema as the desktop app. Android 8.0 (API 26) or newer and a current Android
System WebView are required. The provided build target is ARM64, suitable for
most current phones and tablets.

## Install and use

Download the APK artifact from the repository's **Actions → Android** workflow,
extract the artifact ZIP and open the APK on your phone. Android may ask you to
allow installation from the browser or file manager you used. CI's default APK
is a development build; a signed production build is available through the
manual workflow after signing secrets have been configured.

Open the navigation menu to reach your collection, reading journal, loans,
sales, insights, imports and settings. Everything is stored on the device.
Android Back closes an open book dialog or navigation drawer, returns from a
screen to the collection, then exits from the collection. The keyboard follows
Android's normal dismissal behavior.

Cataloging, search, reading, loans, financial records, comparisons already saved,
CSV/JSON import, ZIP export and restore work without an account or connection.
All translations, script fonts and barcode decoding are bundled. Scan ISBN asks
for camera permission when opened; denial leaves photo/manual entry available.
Live camera hardware and vendor-specific document providers still need testing
on physical phones.

**The release APK has no INTERNET permission**, disables Android cloud backup,
and never fetches Open Library metadata, even after importing desktop settings.
Metadata already cached in a transferred library remains available. A new ISBN
can be entered/scanned offline; fill in uncached book details manually. OLX
links explicitly open another app/browser and require its connection. Debug
builds permit INTERNET for the optional Vite development server; the library
still works in airplane mode and Android metadata fetching remains disabled.

## Transfer a complete library between PC and phone

Use the same or a newer ShelfLedger version on the receiving device.

1. On the source device open **Settings → Data → Export for another device**.
2. Choose a local folder, such as **Downloads**, in the system save dialog. Save
   the `.zip` unchanged. Wait for the app's exported/saved confirmation before
   copying it. If you cancel, **Save file** retries the existing backup.
3. Copy the ZIP to the other device using USB, a removable drive, or another
   method you choose. USB works entirely offline.
4. On the destination open **Settings → Data → Import from another device** and
   select the ZIP in the system picker.
5. Choose **Replace current library** or **Restore as separate library**. A
   separate library becomes available under **Settings → Libraries**; open it
   there. Picking a file alone never replaces data.

This works in both directions: Windows/macOS/Linux ↔ Android, Android ↔ Android,
and PC ↔ PC. Desktop must include the new Save file/native document commands.
The ZIP includes books, editions, copies, reading progress/reviews, contacts and
loans, tags, locations, acquisition costs, listings/sales, saved comparisons,
valuations, preferences and cached metadata. IDs and money/currency stay intact.
Device profile identity is intentionally local. Other saved library profiles
are separate: open and export each one individually.

The receiver validates the archive's manifest, hash, database integrity,
migrations and record invariants before applying it transactionally. Replacing
also preserves a safety backup of the previous library in private app storage.
Corrupted or newer unsupported formats are rejected. Native picker uploads are
limited to 512 MiB; decompressed database entries have the same limit.

Transfers are **manual snapshots**, not automatic synchronization or a merge of
two independently edited libraries. Choose which copy is authoritative before
replacing. ZIPs are not encrypted and can contain contacts/financial history;
keep them where you intend. Uninstalling Android removes its private library,
so export before uninstalling or changing signing keys.

CSV is for spreadsheet/Goodreads interoperability; it is not a full backup.
JSON snapshots are also portable through the existing preview/confirm importer.
Use ZIP for the complete-library transfer described above.

## Build locally

Install Node/pnpm and Rust from the [main tutorial](tutorial.md), plus JDK 21,
Android Studio's SDK Platform 36, Build Tools 36.0.0, Platform Tools and NDK
28.2.13676358 (r28c). Install the Android Rust target:

```sh
rustup target add aarch64-linux-android
```

Set your own paths; example for macOS:

```sh
export JAVA_HOME="$(/usr/libexec/java_home -v 21)"
export ANDROID_HOME="$HOME/Library/Android/sdk"
export NDK_HOME="$ANDROID_HOME/ndk/28.2.13676358"
pnpm install --frozen-lockfile
pnpm android:build:debug
```

The native project is committed under `src-tauri/gen/android`; **do not run
`tauri android init` over it**, since that can replace its offline manifest and
navigation customizations. Tauri regenerates its ignored dependency glue at
build time. SDK locations and signing material are never committed.

The debug APK is written to:

```text
src-tauri/gen/android/app/build/outputs/apk/universal/debug/app-universal-debug.apk
```

Despite the Gradle flavor name `universal`, `--target aarch64` includes ARM64 only.
For another architecture install its Rust target and pass the matching Tauri
`--target` (`x86_64`, `armv7`, etc.) directly. Android Studio can open the committed
project. For live development on a USB-debugging device:

```sh
adb devices
pnpm android:dev
```

For a bundled app with no development server, install the built APK instead:

```sh
adb install -r src-tauri/gen/android/app/build/outputs/apk/universal/debug/app-universal-debug.apk
```

Building requires network access initially for toolchains/dependencies; using
the installed app does not.

## Signed production APK

Keep a private Android signing keystore backed up securely: updates must use the
same key. Set these environment variables locally, pointing to your own key:

- `ANDROID_KEYSTORE_PATH`: absolute path to the `.jks` file.
- `ANDROID_KEYSTORE_PASSWORD`: keystore password.
- `ANDROID_KEY_ALIAS`: alias inside the keystore.
- `ANDROID_KEY_PASSWORD`: that key's password.

Then run `pnpm android:build`. Without the variables the release APK is unsigned
and cannot be installed until signed. Signed output is under
`app/build/outputs/apk/universal/release/`. Check permissions with:

```sh
node scripts/check-android-apk.mjs path/to/app-universal-release.apk release
```

For GitHub Actions set `ANDROID_KEYSTORE_BASE64` (base64 of the keystore),
`ANDROID_KEYSTORE_PASSWORD`, `ANDROID_KEY_ALIAS`, and `ANDROID_KEY_PASSWORD` as
repository secrets. Run **Android → Run workflow → signed_release**. The workflow
builds the release, checks that no network/storage permissions were added,
verifies the APK signature and uploads the APK plus SHA-256 checksums as an
Actions artifact. This workflow does not publish to Google Play or change the
existing desktop release. Signing secrets are not required for pull-request
checks or ordinary debug builds.

## Verification — 29.10.2026

- ARM64 debug and optimized release APKs compiled on macOS. The release was
  signed with a development certificate for local installation; this is a
  preview, not the production signing identity. Its packaged permissions and
  v2/v3 signatures were verified. No network or broad-storage permissions.
- The installed release was exercised on Android 14 in airplane mode: desktop
  ZIP → system picker → restore → Android save dialog → desktop restore.
  Comparing complete snapshots showed identical records. An additional book
  entered offline persisted after restart and transferred back to the desktop.
- Camera permission grant/denial, manual entry, keyboard dismissal, nested
  dialog Back, save cancellation/retry and native restore were checked.
- 108 Rust tests passed (one optional private-file test ignored), 295 frontend
  tests and 10 release tests passed; build/typecheck, format, Clippy and Android
  workflow syntax checks passed. The Actions workflow has not yet run remotely.
- Physical-device camera decoding, vendor document providers and production
  signing still require release acceptance. Upstream Android Gradle/Kotlin
  deprecation warnings and the frontend chunk-size warning remain non-fatal.

Official setup references: [Tauri Android prerequisites](https://v2.tauri.app/start/prerequisites/#android),
[Android signing](https://v2.tauri.app/distribute/sign/android/), and
[native document dialogs](https://v2.tauri.app/plugin/dialog/).
Native window insets follow the [Android edge-to-edge guidance](https://developer.android.com/develop/ui/views/layout/edge-to-edge).
