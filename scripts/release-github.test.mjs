import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import { mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { test } from 'node:test';
import { prepareRelease, publishRelease, validateRelease } from './release-github.mjs';

function fixture(t, overrides = {}, existing = []) {
  const directory = mkdtempSync(join(tmpdir(), 'shelfledger-publish-'));
  t.after(() => rmSync(directory, { recursive: true, force: true }));
  for (const name of ['app.dmg', 'app.exe', 'SHA256SUMS']) writeFileSync(join(directory, name), name);
  const calls = [];
  const release = { id: 123, tag_name: 'v0.1.1', draft: false, ...overrides };
  const github = {
    paginate: async () => existing,
    rest: { repos: {
      getRelease: async () => ({ data: release }),
      getCommit: async () => ({ data: { sha: 'original' } }),
      listReleaseAssets: () => {},
      deleteReleaseAsset: async args => calls.push(['delete', args]),
      uploadReleaseAsset: async args => calls.push(['upload', args]),
      updateRelease: async args => calls.push(['update', args]),
    } },
  };
  return { args: { github, owner: 'o', repo: 'r', releaseId: 123, tag: 'v0.1.1', sha: 'original', directory, notes: 'Verified downloads', allowPublished: true }, calls };
}

test('published repair requires explicit opt-in, exact stable tag and mutable release', () => {
  const release = { tag_name: 'v0.1.1', draft: false };
  assert.throws(() => validateRelease(release, 'v0.1.1'), /repair_published/);
  validateRelease(release, 'v0.1.1', true);
  validateRelease({ ...release, draft: true }, 'v0.1.1');
  assert.throws(() => validateRelease({ ...release, immutable: true }, 'v0.1.1', true), /immutable/);
  assert.throws(() => validateRelease(release, 'v0.1.10', true), /exact stable/);
  assert.throws(() => validateRelease({ tag_name: 'v0.1.1-beta' }, 'v0.1.1-beta', true), /exact stable/);
});

test('a validated draft may become public during builds without manual dispatch inputs', async t => {
  const release = { id: 123, tag_name: 'v0.1.1', draft: true };
  const prepared = prepareRelease(release, 'v0.1.1');
  assert.deepEqual(prepared, { tag: 'v0.1.1', release_id: '123', allow_published: 'true' });
  // The exact draft accepted by prepare is now public when publish rereads it.
  const { args, calls } = fixture(t);
  await publishRelease({ ...args, tag: prepared.tag, releaseId: prepared.release_id,
    allowPublished: prepared.allow_published === 'true' });
  assert.equal(calls.filter(([op]) => op === 'upload').length, 3);
  assert.ok(calls.every(([op]) => op !== 'delete'));
  assert.equal(calls.at(-1)[1].draft, undefined);
  assert.throws(() => prepareRelease({ ...release, draft: false }, 'v0.1.1'), /repair_published/);
  assert.throws(() => prepareRelease(release, 'v0.1.2'), /exact stable/);
  assert.throws(() => prepareRelease({ ...release, immutable: true }, 'v0.1.1'), /immutable/);
});

test('draft-to-public transition still refuses conflicting public assets before any write', async t => {
  const prepared = prepareRelease({ id: 123, tag_name: 'v0.1.1', draft: true }, 'v0.1.1');
  const { args, calls } = fixture(t, {}, [{ id: 1, name: 'app.exe', digest: 'conflicting' }]);
  await assert.rejects(publishRelease({ ...args, allowPublished: prepared.allow_published === 'true' }), /overwrite public asset/);
  assert.equal(calls.length, 0);
});

test('public repair uploads missing files, checksum last, and preserves publication state', async t => {
  const { args, calls } = fixture(t);
  await publishRelease(args);
  assert.deepEqual(calls.filter(([op]) => op === 'upload').map(([, arg]) => arg.name), ['app.dmg', 'app.exe', 'SHA256SUMS']);
  assert.ok(calls.every(([op]) => op !== 'delete'));
  assert.equal(calls.at(-1)[1].body, 'Verified downloads');
  assert.equal(calls.at(-1)[1].draft, undefined);
  assert.equal(calls.at(-1)[1].make_latest, undefined);
});

test('public repair resumes identical uploads and preserves unrelated assets', async t => {
  const digest = `sha256:${createHash('sha256').update('app.dmg').digest('hex')}`;
  const { args, calls } = fixture(t, {}, [{ id: 1, name: 'app.dmg', digest }, { id: 2, name: 'other.zip' }]);
  await publishRelease(args);
  assert.deepEqual(calls.filter(([op]) => op === 'upload').map(([, arg]) => arg.name), ['app.exe', 'SHA256SUMS']);
  assert.ok(calls.every(([op]) => op !== 'delete'));
});

test('conflicting public assets or moved tags prevent every write', async t => {
  const { args, calls } = fixture(t, {}, [{ id: 1, name: 'app.exe', digest: 'different' }]);
  await assert.rejects(publishRelease(args), /overwrite public asset/);
  assert.equal(calls.length, 0);
  args.sha = 'moved';
  await assert.rejects(publishRelease(args), /tag moved/);
  assert.equal(calls.length, 0);
});

test('draft retries can replace incomplete assets and only publish after successful uploads', async t => {
  const { args, calls } = fixture(t, { draft: true }, [{ id: 1, name: 'app.dmg', digest: 'old' }]);
  await publishRelease(args);
  assert.equal(calls[0][0], 'delete');
  assert.equal(calls.at(-1)[1].draft, false);
  assert.equal(calls.at(-1)[1].make_latest, 'true');
  calls.length = 0;
  args.github.rest.repos.uploadReleaseAsset = async () => { throw new Error('upload failed'); };
  await assert.rejects(publishRelease(args), /upload failed/);
  assert.ok(calls.every(([op]) => op !== 'update'));
});
