import { createHash } from 'node:crypto';
import { copyFileSync, lstatSync, mkdirSync, readFileSync, readdirSync, writeFileSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';
import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';

export const platforms = {
  'macos-arm64': { target: 'aarch64-apple-darwin', bundles: ['dmg'] },
  'macos-x64': { target: 'x86_64-apple-darwin', bundles: ['dmg'] },
  'windows-x64': { target: 'x86_64-pc-windows-msvc', bundles: ['nsis'] },
  'linux-arm64': { target: 'aarch64-unknown-linux-gnu', bundles: ['deb', 'appimage'] },
  'linux-x64': { target: 'x86_64-unknown-linux-gnu', bundles: ['deb', 'appimage'] },
};
const extensions = { dmg: '.dmg', nsis: '.exe', deb: '.deb', appimage: '.AppImage' };
const stableVersion = /^(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)$/;
const json = (path) => JSON.parse(readFileSync(path, 'utf8'));

function tomlValue(text, key) {
  const match = text.match(new RegExp(`^${key}\\s*=\\s*"([^"]+)"`, 'm'));
  if (!match) throw new Error(`Missing TOML ${key}`);
  return match[1];
}

export function validateVersions(root, tag) {
  const version = json(join(root, 'package.json')).version;
  if (!stableVersion.test(version)) throw new Error(`Expected a stable SemVer version, got ${version}`);
  const cargo = readFileSync(join(root, 'src-tauri/Cargo.toml'), 'utf8');
  const packageSection = cargo.match(/^\[package\]\s*\n([\s\S]*?)(?=^\[|$(?![\s\S]))/m)?.[1];
  if (!packageSection) throw new Error('Missing Cargo package section');
  const lockPackages = readFileSync(join(root, 'src-tauri/Cargo.lock'), 'utf8').split(/^\[\[package\]\]\s*$/m);
  const ownPackages = lockPackages.filter((block) => /^name\s*=\s*"shelfledger"\s*$/m.test(block));
  if (ownPackages.length !== 1) throw new Error('Expected one shelfledger package in Cargo.lock');
  const versions = {
    'tauri.conf.json': json(join(root, 'src-tauri/tauri.conf.json')).version,
    'Cargo.toml': tomlValue(packageSection, 'version'),
    'Cargo.lock': tomlValue(ownPackages[0], 'version'),
  };
  const manifest = json(join(root, '.release-please-manifest.json'));
  if (manifest['.'] !== undefined) versions['release manifest'] = manifest['.'];
  for (const [file, value] of Object.entries(versions)) {
    if (value !== version) throw new Error(`${file} version ${value} differs from package.json ${version}`);
  }
  if (tag !== undefined && tag !== `v${version}`) throw new Error(`Tag ${tag} must match v${version}`);
  return version;
}

export function assetNames(version) {
  return Object.entries(platforms).flatMap(([platform, { bundles }]) =>
    bundles.map((bundle) => `ShelfLedger_${version}_${platform}${extensions[bundle]}`)).sort();
}

function regularNonempty(path) {
  const stat = lstatSync(path);
  if (!stat.isFile() || stat.size === 0) throw new Error(`Expected a nonempty regular file: ${path}`);
}

export function stageAssets(root, platform, destination) {
  const spec = platforms[platform];
  if (!spec) throw new Error(`Unknown release platform: ${platform}`);
  const version = validateVersions(root);
  // Validate the complete platform output before copying anything.
  const files = spec.bundles.map((bundle) => {
    const directory = join(root, 'src-tauri/target', spec.target, 'release/bundle', bundle);
    const candidates = readdirSync(directory).filter((name) => name.endsWith(extensions[bundle]));
    if (candidates.length !== 1) throw new Error(`Expected one ${bundle} installer in ${directory}`);
    const source = join(directory, candidates[0]);
    regularNonempty(source);
    return { source, name: `ShelfLedger_${version}_${platform}${extensions[bundle]}` };
  });
  mkdirSync(destination, { recursive: true });
  for (const { source, name } of files) copyFileSync(source, join(destination, name));
}

export function checksumAssets(directory, version) {
  const expected = assetNames(version);
  const actual = readdirSync(directory).filter((name) => name !== 'SHA256SUMS').sort();
  if (JSON.stringify(actual) !== JSON.stringify(expected)) throw new Error('Release assets are missing or unexpected');
  const checksums = expected.map((name) => {
    const path = join(directory, name);
    regularNonempty(path);
    return `${createHash('sha256').update(readFileSync(path)).digest('hex')}  ${name}`;
  }).join('\n') + '\n';
  writeFileSync(join(directory, 'SHA256SUMS'), checksums);
  return checksums;
}

export function releaseNotes(changelog, version, repository) {
  if (!/^[A-Za-z0-9][A-Za-z0-9-]*\/[A-Za-z0-9][A-Za-z0-9_.-]*$/.test(repository)) throw new Error('Invalid GitHub repository');
  const sections = changelog.replaceAll('\r\n', '\n').split(/^## /m).slice(1);
  const section = sections.find((text) => new RegExp(`^\\[?${version.replaceAll('.', '\\.')}\\]?(?:\\([^\\n]*\\))?(?:\\s|$)`).test(text));
  if (!section) throw new Error(`Missing changelog section for ${version}`);
  const content = (text) => text.includes('\n') ? text.slice(text.indexOf('\n') + 1).trim() : '';
  let notes = content(section);
  // Keep the pre-automation history in the first packaged release description.
  const initial = sections.find((text) => /^Development history \(before automated releases\)/.test(text));
  if (version === '0.1.0' && initial) notes += `\n\n### Initial application features and fixes\n\n${content(initial)}`;
  if (!notes.trim()) throw new Error('Release notes must not be empty');
  const base = `https://github.com/${repository}/releases/download/v${version}`;
  const downloads = assetNames(version).map((name) => `- [${name}](${base}/${name})`).join('\n');
  return `${notes}\n\n## Downloads\n\n${downloads}\n- [SHA256SUMS](${base}/SHA256SUMS)\n\nVerify downloads against SHA256SUMS. macOS builds use ad-hoc signing unless the repository's Apple signing and notarization secrets are configured; Windows installers are unsigned. See the [release guide](https://github.com/${repository}/blob/v${version}/docs/release.md) for signing and installation details.\n`;
}

export function signingEnvironment(environment) {
  const result = { ...environment };
  const keys = ['APPLE_CERTIFICATE', 'APPLE_CERTIFICATE_PASSWORD', 'APPLE_SIGNING_IDENTITY', 'APPLE_ID', 'APPLE_PASSWORD', 'APPLE_TEAM_ID'];
  for (const key of keys) if (!result[key]) delete result[key];
  if (keys.some((key) => result[key]) && !keys.every((key) => result[key])) {
    throw new Error('Apple distribution requires all six signing and notarization secrets; see docs/release.md');
  }
  return result;
}

function run(command, args, env = process.env) {
  const result = spawnSync(command, args, { env, stdio: 'inherit', shell: process.platform === 'win32' });
  if (result.error) throw result.error;
  if (result.status !== 0) throw new Error(`${command} failed with status ${result.status}`);
}

function main() {
  const root = process.env.RELEASE_ROOT ? resolve(process.env.RELEASE_ROOT) : resolve(dirname(fileURLToPath(import.meta.url)), '..');
  const [command = 'check', argument] = process.argv.slice(2);
  const version = validateVersions(root, process.env.RELEASE_TAG);
  if (command === 'check') console.log(`Release versions agree: ${version}`);
  else if (command === 'stage') stageAssets(root, argument, join(root, 'release-assets'));
  else if (command === 'checksums') checksumAssets(join(root, 'release-assets'), version);
  else if (command === 'notes') {
    const notes = releaseNotes(readFileSync(join(root, 'CHANGELOG.md'), 'utf8'), version, process.env.GITHUB_REPOSITORY);
    writeFileSync(join(root, 'release-notes.md'), notes);
  } else if (command === 'build') {
    const spec = platforms[argument];
    if (!spec) throw new Error(`Unknown release platform: ${argument}`);
    const env = process.platform === 'darwin' ? signingEnvironment(process.env) : process.env;
    run('pnpm', ['tauri', 'build', '--ci', '--target', spec.target, '--bundles', spec.bundles.join(','), '--', '--locked'], env);
    if (process.platform === 'darwin') {
      const app = join(root, 'src-tauri/target', spec.target, 'release/bundle/macos/ShelfLedger.app');
      run('codesign', ['--verify', '--deep', '--strict', app]);
      if (env.APPLE_CERTIFICATE) run('xcrun', ['stapler', 'validate', app]);
    }
  } else throw new Error(`Unknown release command: ${command}`);
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  try { main(); } catch (error) { console.error(error.message); process.exitCode = 1; }
}
