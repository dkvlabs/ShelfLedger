# Release checklist (Phase 5)

Signing/notarization requires platform accounts and secrets that must never live
in this repository; this checklist captures the manual steps per release.

## 1. Prepare

- [ ] Bump versions: `package.json`, `src-tauri/tauri.conf.json`, `src-tauri/Cargo.toml` (workspace member only), and add a `CHANGELOG.md` section.
- [ ] Run the full gate locally: `pnpm lint && pnpm test:web && pnpm test:rust && cargo clippy --all-targets -- -D warnings` inside `src-tauri`.
- [ ] Confirm `cargo audit` and the secret scan pass in CI.
- [ ] Verify the phase acceptance criteria in `docs/implementation.md` are actually met.

## 2. Build artifacts

- [ ] `pnpm tauri build` on macOS, Windows, and Linux (the `tauri-build` job is configured for all three; inspect its actual results before release).
- [ ] macOS: sign with Developer ID and **notarize** (`tauri.conf.json` updater/signing config); keep the signing key in CI secrets or a local keychain — never in the repo.
- [ ] Windows: sign the installer with an Authenticode certificate if available.
- [ ] Linux: publish AppImage/deb; consider the flathub route later.
- [ ] Camera scanning in each packaged app: verify first-use consent, denied
      access guidance, retry after allowing access, and camera release on close.
      On macOS, confirm the bundle includes `NSCameraUsageDescription` and the
      signed app includes `com.apple.security.device.camera`. Where the webview
      does not expose camera capture, verify the photo fallback.
- [ ] Optional: enable the signed Tauri updater (minisign key pair; public key committed, private key in secrets).

## 3. Publish

- [ ] GitHub Release with changelog notes and artifacts + checksums (SHA-256, published alongside).
- [ ] Generate an SBOM (`cargo cyclonedx` / `pnpm sbom`) and attach it.
- [ ] Update the demo dataset (`demo/demo-library.json`) if the schema changed.
- [ ] Post-release smoke test: install on a clean machine, create a library, import, lend, sell, back up, restore — no terminal required (Phase 5 acceptance).

## 4. Disclosure readiness

- [ ] SECURITY.md points at a live private-reporting channel.
- [ ] Tag `good first issue` / `help wanted` starter issues for new contributors.


## Local audit verification (27.10.2026)

See `audit-remediation.md` for the 24-finding register and test evidence. Local
checks pass: 88 Rust tests, 59 frontend tests, strict frontend production build,
Clippy, and a macOS `.app` bundle. These do not check off the cross-platform,
clean-machine or physical-camera release gates above.

macOS builds default to the ad-hoc identity `-`, which embeds the camera
entitlement without private credentials. For distribution, override
`bundle.macOS.signingIdentity` with your Developer ID identity in a local/CI
Tauri configuration and perform notarization. Verify the produced signature and
`NSCameraUsageDescription` before testing consent on a real camera. Ad-hoc
signing is not Developer ID signing or notarization.

The JavaScript advisory scan reports zero vulnerabilities; the Rust scan has
zero vulnerability matches but retains the glib unsoundness and proc-macro-error
maintenance warnings described in the remediation register. Do not hide them.
