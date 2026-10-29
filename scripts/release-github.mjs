import { createHash } from 'node:crypto';
import { readFileSync, readdirSync } from 'node:fs';
import { join } from 'node:path';

export function validateRelease(release, tag, allowPublished = false) {
  if (!release || release.tag_name !== tag || !/^v(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)$/.test(tag)) {
    throw new Error('Expected an existing release with the exact stable version tag');
  }
  if (release.immutable) throw new Error('This release is immutable; create a new release instead');
  if (!release.draft && !allowPublished) throw new Error('Enable repair_published explicitly to add missing public release assets');
}

export async function publishRelease({ github, owner, repo, releaseId, tag, sha, directory, notes, allowPublished = false }) {
  const release_id = Number(releaseId);
  const { data: release } = await github.rest.repos.getRelease({ owner, repo, release_id });
  validateRelease(release, tag, allowPublished);
  const commit = await github.rest.repos.getCommit({ owner, repo, ref: `tags/${tag}` });
  if (commit.data.sha !== sha) throw new Error('Release tag moved during the build');
  const existing = await github.paginate(github.rest.repos.listReleaseAssets, { owner, repo, release_id, per_page: 100 });
  const names = readdirSync(directory).sort();
  // Check all collisions before uploading anything. Public binaries are never
  // deleted or replaced; identical assets make an interrupted upload resumable.
  const files = names.map(name => {
    const data = readFileSync(join(directory, name));
    const digest = `sha256:${createHash('sha256').update(data).digest('hex')}`;
    const old = existing.find(asset => asset.name === name);
    if (!release.draft && old && old.digest !== digest) {
      throw new Error(`Refusing to overwrite public asset ${name}; create a patch release`);
    }
    return { name, data, digest, old };
  });
  if (release.draft && existing.some(asset => !names.includes(asset.name))) throw new Error('Draft contains unexpected assets');
  // Upload the checksum list last so it never advertises files still missing.
  files.sort((a, b) => Number(a.name === 'SHA256SUMS') - Number(b.name === 'SHA256SUMS'));
  for (const { name, data, digest, old } of files) {
    if (old?.digest === digest) continue;
    if (old) await github.rest.repos.deleteReleaseAsset({ owner, repo, asset_id: old.id });
    await github.rest.repos.uploadReleaseAsset({ owner, repo, release_id, name, data, headers: { 'content-type': 'application/octet-stream' } });
  }
  await github.rest.repos.updateRelease({
    owner, repo, release_id, body: notes,
    ...(release.draft ? { draft: false, prerelease: false, make_latest: 'true' } : {}),
  });
  return names;
}
