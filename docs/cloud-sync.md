# Google Drive sync and iCloud backups

Open **Settings → Cloud sync & backup** in the installed desktop or Android app.
Core features work offline without a Google account. Cloud operations run only
when requested; nothing syncs at startup or in the background.

## Set up Google Drive

Use **one Google Cloud project** for desktop and Android registrations.

1. In [Google Cloud Console](https://console.cloud.google.com/), create/select a
   project and enable **Google Drive API** under **APIs & Services → Library**.
2. Open **Google Auth Platform**. Complete **Branding** with your app name and
   support/developer email. Configure **Audience** for your users. For personal
   testing, use External/Testing and add each Google account under **Test users**.
3. Under **Data Access**, add only this scope:

   ```text
   https://www.googleapis.com/auth/drive.appdata
   ```

4. Under **Clients → Create client**, choose **Desktop app**. Download its JSON
   and retain `installed.client_id` and `installed.client_secret` locally. Do not
   use a Web application client or service-account key.
5. Configure the desktop build in your terminal using your own values:

   ```sh
   export SHELFLEDGER_GOOGLE_CLIENT_ID='YOUR_CLIENT_ID.apps.googleusercontent.com'
   export SHELFLEDGER_GOOGLE_CLIENT_SECRET='YOUR_DESKTOP_CLIENT_SECRET'
   pnpm tauri dev
   ```

   To bundle the configuration into an installer, run `pnpm tauri build` with
   the same environment. Runtime environment values override embedded values.
   An unconfigured installer displays **Google Drive sign-in is not configured
   in this build**. Downloading the JSON alone does not configure ShelfLedger.
   Keep credentials out of Git and chat messages. Desktop clients are public
   installed-app clients; an embedded client secret is not a security boundary.
   Authorization uses PKCE, random state and a temporary loopback callback in
   the system browser. See [Google's desktop OAuth guide](https://developers.google.com/identity/protocols/oauth2/native-app).
6. For GitHub desktop releases, set repository Actions **variable**
   `SHELFLEDGER_GOOGLE_CLIENT_ID` and **secret**
   `SHELFLEDGER_GOOGLE_CLIENT_SECRET`. The release workflow passes them to native
   compilation. Do not use an access token, refresh token or user password.

### Android registration

In the **same project**, create an **Android** OAuth client with:

- Package name: `io.github.shelfledger.app`.
- SHA-1 certificate fingerprint: the certificate signing the installed APK.

For an existing APK, Android SDK Build Tools can print its signing certificate:

```sh
apksigner verify --print-certs /absolute/path/to/shelfledger.apk
```

Use **Signer #1 certificate SHA-1 digest**. Register separate clients for debug
and production certificates if you use both. Keep the production signing key
stable between releases. If Google Play signing is introduced later, register
its app-signing certificate too. Android does not use the desktop client ID/secret
environment variables or need a downloaded JSON in the source tree. The phone
needs Google Play services; unsupported AOSP devices can use ZIP transfers.
See [Android authorization setup](https://developer.android.com/identity/authorization).

## Sync two devices

1. On the device containing your library, choose **Sign in to Google Drive**.
   Authorize app-data access with your test account. Create a named cloud library;
   this creates its descriptor without uploading books yet.
2. Press **Sync now** to upload the active library.
3. On another configured device, sign in with the same Google account, select
   that cloud library and choose **Use selected library**.
4. Press **Sync now**. An empty local library receives the cloud version. Different
   existing records require a choice of which complete library to use. To keep
   local libraries separate, first create/switch to an empty one under
   **Settings → Libraries**.
5. After editing a device, press **Sync now** there, then on the other device.

Connections are local to each library profile. Preferences, language and selected
profile remain local. Books, editions, copies, covers, reading/loan history,
contacts, finances, comparisons, metadata cache and import/activity records sync
together. General attachment-file bytes and receipts remain outside this scope.

Tokens remain in native memory for at most approximately one hour. Sign in again
after restart or expiration; Android may reuse consent. Disconnect removes the
local link and in-memory token, leaving cloud backups and Google's consent intact.

### Conflicts and recovery

If both devices changed since their shared version, or simultaneous uploads
created cloud branches, sync pauses for a choice. **Keep this device** publishes
the complete local version. **Use cloud version** applies the selected complete
cloud version after creating a local recovery ZIP. Confirm in the dialog.
Individual records are not merged. A stale choice cannot overwrite intervening
edits: review the current versions and try again.

Uploads create immutable revisions; older versions are retained. Use **Cloud
backup history**, select a version, then **Use cloud version** and confirm.
Restoring history publishes a new revision while preserving the later ones.
**Recovery backup** saves the pre-replacement ZIP through the system picker.
Recovery ZIPs also remain in the app-data `backups` directory.

Each cloud revision is limited to 64 MiB of JSON, including cover images. The
client refuses lists exceeding 1,000 files or 100 pages. There is no automatic
history pruning; at the limit, retain an independent ZIP and create a new cloud
library. Sync errors leave local records usable; press Sync now to retry.

Revisions live in Google's hidden app-specific `appDataFolder`, separate from
ordinary Drive files. Manage versions through ShelfLedger. Removing the app's
Drive data through Google can remove the entire cloud history. Only the
non-sensitive `drive.appdata` scope is requested, with no access to other Drive
documents. See [Google's app-data documentation](https://developers.google.com/workspace/drive/api/guides/appdata).

## iCloud and other file providers

Choose **Back up to cloud file**, then iCloud Drive or another installed provider
in the system save dialog. A complete ZIP is generated locally first. Cancelling
leaves **Save file** available to retry the same ZIP. The provider manages upload;
ShelfLedger cannot certify its completion.

On macOS, choose an available iCloud Drive location. On Windows, iCloud Drive must
first be available through iCloud for Windows. Android offers installed document
providers, usually Google Drive; ShelfLedger has no native iCloud/CloudKit
integration. Availability depends on device and account configuration.

On the receiving device choose **Open cloud backup**, pick the ZIP, then confirm
**Restore as separate library** or **Replace current library**. Open a separate
restore under **Settings → Libraries**. File backups are manual snapshots and do
not participate in Drive revision history.

Cloud snapshots and ZIPs include contacts and finances, without extra ShelfLedger
encryption. Use an account and location you trust. Removing local app data does
not remove cloud/exported copies. See [PRIVACY.md](../PRIVACY.md).

## Verification and remaining checks

Tests exercise two independent SQLite devices against a local mock Drive HTTP
server: push/pull, conflicts, branches, historical restore, corrupted data, failed
uploads, offline errors, preserved settings and rejected intervening edits/profile
changes. Frontend tests cover explicit actions and confirmation/cancellation.
The Android Kotlin bridge compiles with Play services Auth 22.0.0; its merged
manifest adds no permissions beyond existing camera, internet and app-scoped
AndroidX receiver access.

Real Google consent, desktop/Android cross-device sync and iCloud provider
transfers still require your Cloud project, matching signing registrations and
signed builds. Mock tests do not certify these device/account flows.
