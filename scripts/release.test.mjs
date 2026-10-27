import { createHash } from 'node:crypto';
import { existsSync, mkdtempSync, mkdirSync, readFileSync, rmSync, symlinkSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import assert from 'node:assert/strict';
import { test } from 'node:test';
import { assetNames, checksumAssets, releaseNotes, signingEnvironment, stageAssets, validateVersions } from './release.mjs';

function fixture(t) {
  const root = mkdtempSync(join(tmpdir(), 'shelfledger-release-'));
  t.after(() => rmSync(root, { recursive: true, force: true }));
  mkdirSync(join(root, 'src-tauri'));
  writeFileSync(join(root, 'package.json'), '{"version":"0.1.0"}');
  writeFileSync(join(root, '.release-please-manifest.json'), '{}');
  writeFileSync(join(root, 'src-tauri/tauri.conf.json'), '{"version":"0.1.0"}');
  writeFileSync(join(root, 'src-tauri/Cargo.toml'), '[package]\nname = "shelfledger"\nversion = "0.1.0"\n\n[dependencies]\nother = "2"\n');
  writeFileSync(join(root, 'src-tauri/Cargo.lock'), 'version = 4\n[[package]]\nname = "dependency"\nversion = "9.8.7"\n[[package]]\nname = "shelfledger"\nversion = "0.1.0"\n');
  return root;
}

test('draft release preparation has write access while build jobs remain read-only', () => {
  const workflow = readFileSync(new URL('../.github/workflows/release.yml', import.meta.url), 'utf8');
  assert.match(workflow, /^permissions:\n  contents: read$/m);
  const jobs = workflow.split(/(?=^  [\w-]+:\n)/m);
  for (const [name, access] of Object.entries({
    'release-please': 'write', prepare: 'write', publish: 'write',
    checks: 'read', installers: 'read',
  })) {
    const job = jobs.find((section) => section.startsWith(`  ${name}:\n`));
    assert.ok(job, `Missing ${name} job`);
    const override = job.match(/^    permissions:\n      contents: (read|write)$/m)?.[1];
    assert.equal(override ?? 'read', access, `${name} contents permission`);
  }
});

test('all four versions and the release manifest must agree with the tag', (t) => {
  const root = fixture(t);
  assert.equal(validateVersions(root, 'v0.1.0'), '0.1.0');
  assert.throws(() => validateVersions(root, 'v0.2.0'), /Tag/);
  assert.throws(() => validateVersions(root, 'v0.1.0; bad'), /Tag/);
  for (const file of ['src-tauri/tauri.conf.json', 'src-tauri/Cargo.toml', 'src-tauri/Cargo.lock']) {
    const path = join(root, file);
    const before = readFileSync(path, 'utf8');
    writeFileSync(path, before.replace('0.1.0', '0.2.0'));
    assert.throws(() => validateVersions(root), /differs/);
    writeFileSync(path, before);
  }
  writeFileSync(join(root, '.release-please-manifest.json'), '{".":"0.2.0"}');
  assert.throws(() => validateVersions(root), /manifest/);
});

test('missing or duplicate application entries in Cargo.lock are rejected', (t) => {
  const root = fixture(t);
  const lock = join(root, 'src-tauri/Cargo.lock');
  writeFileSync(lock, '[[package]]\nname = "other"\nversion = "0.1.0"\n');
  assert.throws(() => validateVersions(root), /one shelfledger/);
  writeFileSync(lock, '[[package]]\nname = "shelfledger"\nversion = "0.1.0"\n'.repeat(2));
  assert.throws(() => validateVersions(root), /one shelfledger/);
});

test('installer staging requires exactly one nonempty installer per bundle', (t) => {
  const root = fixture(t);
  const bundle = join(root, 'src-tauri/target/x86_64-pc-windows-msvc/release/bundle/nsis');
  const output = join(root, 'output');
  mkdirSync(bundle, { recursive: true });
  assert.throws(() => stageAssets(root, 'windows-x64', output), /Expected one/);
  writeFileSync(join(bundle, 'setup.exe'), '');
  assert.throws(() => stageAssets(root, 'windows-x64', output), /nonempty/);
  writeFileSync(join(bundle, 'setup.exe'), 'installer');
  stageAssets(root, 'windows-x64', output);
  assert.equal(readFileSync(join(output, 'ShelfLedger_0.1.0_windows-x64.exe'), 'utf8'), 'installer');
  writeFileSync(join(bundle, 'stale.exe'), 'old');
  assert.throws(() => stageAssets(root, 'windows-x64', output), /Expected one/);
  assert.throws(() => stageAssets(root, 'unsupported', output), /Unknown/);
});

test('checksums require the complete five-installer matrix and deterministic SHA-256 values', (t) => {
  const root = fixture(t);
  const names = assetNames('0.1.0');
  assert.deepEqual(names, [
    'ShelfLedger_0.1.0_linux-x64.AppImage', 'ShelfLedger_0.1.0_linux-x64.deb',
    'ShelfLedger_0.1.0_macos-arm64.dmg', 'ShelfLedger_0.1.0_macos-x64.dmg',
    'ShelfLedger_0.1.0_windows-x64.exe',
  ]);
  const directory = join(root, 'assets');
  mkdirSync(directory);
  for (const name of names.slice(1)) writeFileSync(join(directory, name), name);
  assert.throws(() => checksumAssets(directory, '0.1.0'), /missing/);
  writeFileSync(join(directory, names[0]), names[0]);
  const expected = names.map((name) => `${createHash('sha256').update(name).digest('hex')}  ${name}\n`).join('');
  assert.equal(checksumAssets(directory, '0.1.0'), expected);
  assert.equal(checksumAssets(directory, '0.1.0'), expected);
  writeFileSync(join(directory, 'unexpected.exe'), 'extra');
  assert.throws(() => checksumAssets(directory, '0.1.0'), /unexpected/);
});

test('Linux staging requires both formats before writing any staged files', (t) => {
  const root = fixture(t);
  const directory = join(root, 'src-tauri/target/x86_64-unknown-linux-gnu/release/bundle');
  const output = join(root, 'output');
  mkdirSync(join(directory, 'deb'), { recursive: true });
  mkdirSync(join(directory, 'appimage'));
  writeFileSync(join(directory, 'deb/app.deb'), 'deb package');
  assert.throws(() => stageAssets(root, 'linux-x64', output), /Expected one appimage/);
  assert.equal(existsSync(output), false);
  writeFileSync(join(directory, 'appimage/app.AppImage'), 'portable app');
  stageAssets(root, 'linux-x64', output);
  assert.equal(readFileSync(join(output, 'ShelfLedger_0.1.0_linux-x64.deb'), 'utf8'), 'deb package');
  assert.equal(readFileSync(join(output, 'ShelfLedger_0.1.0_linux-x64.AppImage'), 'utf8'), 'portable app');
});

test('prerelease and malformed package versions cannot be published as stable', (t) => {
  const root = fixture(t);
  for (const version of ['0.1.0-beta.1', '01.1.0', '0.1', 'v0.1.0']) {
    writeFileSync(join(root, 'package.json'), JSON.stringify({ version }));
    assert.throws(() => validateVersions(root), /stable SemVer/);
  }
});

test('symlinks cannot substitute for release binaries', { skip: process.platform === 'win32' }, (t) => {
  const root = fixture(t);
  const directory = join(root, 'assets');
  mkdirSync(directory);
  for (const name of assetNames('0.1.0')) writeFileSync(join(directory, name), 'binary');
  const name = assetNames('0.1.0')[0];
  rmSync(join(directory, name));
  symlinkSync(join(directory, assetNames('0.1.0')[1]), join(directory, name));
  assert.throws(() => checksumAssets(directory, '0.1.0'), /regular file/);
});

test('release descriptions select the exact version and include curated initial history once', () => {
  const changelog = '# Changelog\n## [Unreleased]\n## Development history (before automated releases)\n### Added\n- Initial library.\n## [0.1.1] (2026-10-28)\n### Fixes\n- CSV fix.\n## 0.1.0 (2026-10-27)\n### Features\n- Automation.\n';
  const first = releaseNotes(changelog, '0.1.0', 'dkvlabs/ShelfLedger');
  assert.match(first, /Initial library/);
  assert.match(first, /Automation/);
  assert.doesNotMatch(first, /CSV fix/);
  assert.match(first, /releases\/download\/v0.1.0\/SHA256SUMS/);
  const later = releaseNotes(changelog, '0.1.1', 'dkvlabs/ShelfLedger');
  assert.match(later, /CSV fix/);
  assert.doesNotMatch(later, /Initial library|Automation\./);
  assert.throws(() => releaseNotes(changelog, '0.2.0', 'dkvlabs/ShelfLedger'), /Missing changelog/);
  assert.throws(() => releaseNotes(changelog, '0.1.0', '../bad'), /repository/);
  assert.throws(() => releaseNotes('# Changelog\n## 0.2.0', '0.2.0', 'dkvlabs/ShelfLedger'), /must not be empty/);
});

test('empty Apple secrets allow ad-hoc builds and partial distribution credentials fail early', () => {
  assert.deepEqual(signingEnvironment({ PATH: 'safe', APPLE_CERTIFICATE: '', APPLE_ID: '' }), { PATH: 'safe' });
  assert.throws(() => signingEnvironment({ APPLE_CERTIFICATE: 'private' }), /all six/);
  assert.throws(() => signingEnvironment({ APPLE_ID: 'private' }), /all six/);
  const complete = Object.fromEntries(['APPLE_CERTIFICATE', 'APPLE_CERTIFICATE_PASSWORD', 'APPLE_SIGNING_IDENTITY', 'APPLE_ID', 'APPLE_PASSWORD', 'APPLE_TEAM_ID'].map((key) => [key, 'secret']));
  assert.deepEqual(signingEnvironment(complete), complete);
});
