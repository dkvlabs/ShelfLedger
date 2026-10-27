# Release checklist (Phase 5)

Signing/notarization requires platform accounts and secrets that must never live
in this repository; this checklist captures the manual steps per release.

## 1. Prepare

- [ ] Bump versions: `package.json`, `src-tauri/tauri.conf.json`, `src-tauri/Cargo.toml` (workspace member only), and add a `CHANGELOG.md` section.
- [ ] Run the full gate locally: `pnpm lint && pnpm test:web && pnpm test:rust && cargo clippy --all-targets -- -D warnings` inside `src-tauri`.
- [ ] Confirm `cargo audit` and the secret scan pass in CI.
- [ ] Verify the phase acceptance criteria in `docs/implementation.md` are actually met.

## 2. Build artifacts

- [ ] `pnpm tauri build` on macOS, Windows, and Linux (CI `tauri-build` job proves all three compile; release artifacts should be built on each native OS).
- [ ] macOS: sign with Developer ID and **notarize** (`tauri.conf.json` updater/signing config); keep the signing key in CI secrets or a local keychain — never in the repo.
- [ ] Windows: sign the installer with an Authenticode certificate if available.
- [ ] Linux: publish AppImage/deb; consider the flathub route later.
- [ ] Optional: enable the signed Tauri updater (minisign key pair; public key committed, private key in secrets).

## 3. Publish

- [ ] GitHub Release with changelog notes and artifacts + checksums (SHA-256, published alongside).
- [ ] Generate an SBOM (`cargo cyclonedx` / `pnpm sbom`) and attach it.
- [ ] Update the demo dataset (`demo/demo-library.json`) if the schema changed.
- [ ] Post-release smoke test: install on a clean machine, create a library, import, lend, sell, back up, restore — no terminal required (Phase 5 acceptance).

## 4. Disclosure readiness

- [ ] SECURITY.md points at a live private-reporting channel.
- [ ] Tag `good first issue` / `help wanted` starter issues for new contributors.
