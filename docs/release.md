# Automated releases

The [Release workflow](../.github/workflows/release.yml) manages versions,
descriptions and desktop installers. A push to `main` updates a release pull
request. Merging that PR creates a version tag and a draft GitHub release, runs
the checks, builds every installer, uploads checksums and publishes the release.

## Enable it on GitHub

1. Push these files to the default branch (`main`), with GitHub Actions enabled.
2. In **Settings → Actions → General → Workflow permissions**, enable **Allow
   GitHub Actions to create and approve pull requests**. The workflow explicitly
   requests contents and pull requests write access for release-please; build
   jobs have read access only. Organization policy must allow these actions.
3. Optionally add `RELEASE_PLEASE_TOKEN` as a repository Actions secret: a
   fine-grained personal access token with repository contents and
   pull requests **read/write**. Otherwise the workflow uses `GITHUB_TOKEN`.
   GitHub may require a maintainer to approve CI runs for a PR created by
   `GITHUB_TOKEN`; approve them on the release PR. The optional token lets PR CI
   run without that approval. Branch protection checks still need to pass.
4. Configure the six Apple secrets below for Developer ID signed and notarized
   macOS downloads. Without them, builds use the existing ad-hoc identity.
   Windows installers are currently unsigned.

Versioning and packaging need no third-party release service. Apple distribution
requires an Apple Developer account; Actions usage follows your GitHub plan.

## Daily use

Use Conventional Commits on `main`. With squash merges, use the convention in
the **PR title**, which becomes the squash commit.

| Commit / PR title | Description category | Version effect |
| --- | --- | --- |
| `feat(scan): add ISBN photo import` | Features | Minor |
| `fix(import): accept blank Goodreads dates` | Fixes | Patch |
| `perf(search): reduce catalog queries` | Performance | Patch |
| `refactor(catalog): simplify edition updates` | Changes | Patch |
| `docs: explain backups` | Documentation | Patch |
| `deps: update a dependency` | Dependencies | Patch |
| `build: update platform packaging` | Build | Patch |
| `ci: improve release retries` | Automation | Patch |
| `chore: refresh development tools` | Maintenance | Patch |

`feat!:` or a `BREAKING CHANGE:` footer marks a breaking change. Before 1.0,
breaking changes bump the minor version; after 1.0 they bump the major version.
Features also bump the minor version before 1.0. The highest applicable change
determines the version. All configured categories are visible and can open a
release PR; merge it when ready to publish, or leave it open to collect changes.

1. Merge application changes. The workflow opens or updates one release PR
   with categorized descriptions, commit links and versions.
2. Review its changelog and version. It updates `package.json`,
   `src-tauri/tauri.conf.json`, `src-tauri/Cargo.toml`, the `shelfledger` package
   in `src-tauri/Cargo.lock`, and `.release-please-manifest.json` together.
   Dependency package versions in Cargo.lock are preserved.

   The lockfile JSONPath uses `name.value` deliberately: release-please's TOML
   updater wraps scalar values with position metadata. Its real 17.3.0 updater
   has been checked against this lockfile to ensure only the app version changes;
   repeat that check when upgrading the pinned release-please action.
3. Merge the release PR. The workflow creates `vX.Y.Z` and a draft release,
   checks tag/version consistency, runs frontend and release tests, and runs
   Rust tests, formatting and Clippy on each build platform.
4. Once every installer succeeds, the workflow checks the complete asset set,
   computes SHA-256 checksums and publishes the release. Its description contains
   Features/Fixes/Changes/etc. as applicable, plus direct download links.

The manifest starts empty because there has not been a packaged release. The
first version is **0.1.0**. Its published description also includes the curated
`Development history` section in `CHANGELOG.md`, covering work done before
Conventional Commits. Later descriptions use only the matching version section.
Review those initial notes in the first release PR.

For an intentional override, add a `Release-As: 0.2.0` footer to a commit landing
on `main` before merging the release PR. Use stable `X.Y.Z` versions; the workflow
rejects prerelease tags. See [release-please customization](https://github.com/googleapis/release-please/blob/main/docs/customizing.md).

## Assets

| Platform | Runner | Download name (`X.Y.Z` = version) |
| --- | --- | --- |
| macOS Apple Silicon | `macos-15` | `ShelfLedger_X.Y.Z_macos-arm64.dmg` |
| macOS Intel | `macos-15` (Intel cross-build) | `ShelfLedger_X.Y.Z_macos-x64.dmg` |
| Windows x64 | `windows-2022` | `ShelfLedger_X.Y.Z_windows-x64.exe` (NSIS) |
| Linux x64 | `ubuntu-22.04` | `ShelfLedger_X.Y.Z_linux-x64.AppImage` and `.deb` |
| Linux ARM64 | `ubuntu-22.04-arm` | `ShelfLedger_X.Y.Z_linux-arm64.AppImage` and `.deb` |
| Checksums | After all builds succeed | `SHA256SUMS` |

Application builds check out the resolved tag commit and use its committed pnpm
and Cargo lockfiles. Packaging helpers come from the workflow commit in a separate
`.release-tools` checkout, so fixes to note parsing, staging and uploads can repair
older tags without changing their application code.
Missing, empty, duplicate or unexpected installers prevent publication. The
publish job checks that the tag has not moved during the build. Workflow
artifacts last seven days; published release assets remain independently.

Download `SHA256SUMS` alongside installers. Linux:
`sha256sum --ignore-missing -c SHA256SUMS`. On macOS compare
`shasum -a 256 ShelfLedger_X.Y.Z_macos-arm64.dmg` to its line in the file.
On Windows use `Get-FileHash .\ShelfLedger_X.Y.Z_windows-x64.exe -Algorithm SHA256`.
Checksums detect corruption; they do not replace platform signatures.

## Retry a failed release

If the draft's Assets list is empty, inspect **Actions → Release** first. A failed
`prepare` job means installer builds never started. Reading a draft release needs
push access: `prepare` therefore requests `contents: write`, while checks and
installer builds retain read-only contents access. A `403 Resource not accessible
by integration` at that step can indicate missing draft access. After pushing a
workflow fix, start a new manual run on `main` with the draft tag in `retry_tag`
so the corrected workflow is used.

Failures leave the release in **draft**. Fix runner configuration or secrets,
then open **Actions → Release → Run workflow**, select **main**, and enter the
tag (for example `v0.1.0`) in **retry_tag**. It rebuilds that exact tag and replaces
only draft assets, publishing once every check passes. For an already public release with missing installers (including `v0.1.1`), enter
its tag and enable **repair_published**. This adds missing assets to that same
release and updates its download links without moving the tag, changing its
publication state, or replacing any published binary. An interrupted upload can
resume when existing asset SHA-256 digests match. Conflicting existing assets or
an immutable release require a new patch release. Leave the checkbox off for
normal draft retries.

Leave `retry_tag` empty to refresh the release PR. For a code fix in an already
tagged release, create a patch release: retries do not include newer `main`
commits. Do not move release tags. If release-please failed before creating a
tag/draft, rerun with an empty retry field after correcting its permissions.

One concurrency group prevents overlapping release jobs; new pushes do not
cancel an in-flight build. Packaging and publishing run in the **same workflow**,
since events created with `GITHUB_TOKEN` generally do not trigger another workflow.

## Apple signing and notarization

Set all six repository Actions secrets together:

| Secret | Value |
| --- | --- |
| `APPLE_CERTIFICATE` | Base64-encoded Developer ID Application `.p12` certificate and private key |
| `APPLE_CERTIFICATE_PASSWORD` | Password used to export the `.p12` |
| `APPLE_SIGNING_IDENTITY` | Full `Developer ID Application: … (TEAMID)` identity |
| `APPLE_ID` | Apple account used for notarization |
| `APPLE_PASSWORD` | An Apple app-specific password |
| `APPLE_TEAM_ID` | Apple Developer team ID |

Tauri imports the certificate, overrides the default `-` signing identity and
notarizes the bundle. The workflow verifies the app signature and validates its
stapled notarization ticket for distribution builds. Partial credentials fail
early; empty secrets are removed so an unconfigured repository can still use
ad-hoc signing and its camera entitlement. Never commit credentials.

Ad-hoc macOS builds are not Developer ID signed or notarized and may be blocked
by Gatekeeper. Windows builds have no Authenticode signing integration and may
trigger SmartScreen. Published notes state these limitations. Signed updater
feeds, SBOM generation, Windows signing and Windows ARM64 remain
separate additions; no updater feed is generated by this workflow.

References: [Tauri macOS signing](https://v2.tauri.app/distribute/sign/macos/),
[release-please action](https://github.com/googleapis/release-please-action),
[GitHub workflow triggers](https://docs.github.com/en/actions/how-tos/write-workflows/choose-when-workflows-run/trigger-a-workflow).

## Local checks and remaining acceptance

```sh
pnpm release:check
pnpm test:release
pnpm lint
pnpm test:web
pnpm test:rust
cargo fmt --manifest-path src-tauri/Cargo.toml -- --check
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings
```

To exercise the same packaging helper locally on Apple Silicon:

```sh
node scripts/release.mjs build macos-arm64
node scripts/release.mjs stage macos-arm64
```

Other helper platforms are `macos-x64`, `windows-x64`, `linux-x64` and `linux-arm64`; run them
on matching native hosts. Output stays under `src-tauri/target` and
`release-assets`, never in a library profile.

Manual release gates still apply:

- Confirm the existing Security workflow's advisory and secret scans pass;
  assess upstream warnings recorded in `audit-remediation.md`.
  The secret job runs Gitleaks CLI 8.30.1 against all fetched Git history with
  redaction enabled; it does not require `GITLEAKS_LICENSE`.
- Install on clean machines: create/import a library, read, lend, sell, back up
  and restore without a terminal.
- Test camera consent, denied-access guidance, retry and camera release on
  close; check macOS purpose text/entitlement and the photo fallback.
- Check acceptance criteria in `implementation.md`, update demo data if the
  schema changes, and verify the private reporting channel in `SECURITY.md`.

Local checks cannot establish Windows/Linux packaging, actual GitHub runner
results, Developer ID notarization or physical-camera consent. Inspect the first
live workflow and complete the corresponding installation checks.

## v0.1.1 installer recovery

The original run stopped at `Missing changelog section for 0.1.1`: its parser
accepted plain version headings but rejected release-please's linked
`## [0.1.1](compare-url) (date)` heading. The corrected helper handles linked
headings, exact version selection and CRLF. Recovery uses the original tag
commit for every native build, then attaches seven installers plus `SHA256SUMS`
to the existing release. The two automatic source archives are not installers.

Use **Actions → Release → Run workflow → main**, set `retry_tag` to `v0.1.1`
and check `repair_published`. Watch the checks, five installer jobs and publish
job; downloads appear in **Releases → v0.1.1 → Assets** after all builds pass.
