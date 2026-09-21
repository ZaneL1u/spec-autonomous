import test from 'node:test';
import assert from 'node:assert/strict';
import type { TestContext } from 'node:test';
import { mkdtemp, writeFile, readFile, rm, readdir } from 'node:fs/promises';
import { existsSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { createHash } from 'node:crypto';
import { ensureGitBinary, gitDelivery, type EnsureGitBinaryOptions, type GitDelivery } from '../lib/git-binary.mjs';
import type { Platform } from '../lib/platform.mjs';

interface DownloadState { downloads: number; wrongHash: boolean; wrongVersion: boolean; fail: boolean }
type FailureKey = 'wrongHash' | 'wrongVersion' | 'fail';

const bytes = Buffer.from('native binary fixture');
const sha256 = createHash('sha256').update(bytes).digest('hex');
// Only the fields `ensureGitBinary` reads; the rest of the descriptor is unused.
const platform = { key: 'darwin-arm64', executable: 'spec-autonomous' } as Platform;
function manifest(): GitDelivery { return { schema_version: 1, repository: 'owner/repo', version: '0.1.0-alpha.5', tag: 'v0.1.0-alpha.5', assets: { 'darwin-arm64': { name: 'spec-autonomous-darwin-arm64', sha256 } } }; }
async function setup(t: TestContext) {
  const root = await mkdtemp(join(tmpdir(), 'sa git binary '));
  t.after(() => rm(root, { recursive: true, force: true }));
  const state: DownloadState = { downloads: 0, wrongHash: false, wrongVersion: false, fail: false };
  const run = async (argv: string[]) => {
    if (argv[1] === 'release') {
      state.downloads++;
      if (state.fail) return { code: 4, stdout: '', stderr: 'not authenticated' };
      const dir = argv[argv.indexOf('--dir') + 1], name = argv[argv.indexOf('--pattern') + 1];
      await writeFile(join(dir, name), state.wrongHash ? 'wrong bytes' : bytes);
      return { code: 0, stdout: '', stderr: '' };
    }
    return { code: 0, stdout: `spec-autonomous ${state.wrongVersion ? '0.0.0' : '0.1.0-alpha.5'}\n`, stderr: '' };
  };
  const options: EnsureGitBinaryOptions & { cache: string } = { cache: join(root, 'cache'), env: {}, platform, run, find: () => '/mock/gh', log: () => {} };
  return { root, state, options };
}

test('concurrent private downloads converge on a verified binary and reuse cache offline', async t => {
  const { options, state } = await setup(t);
  const paths = await Promise.all(Array.from({ length: 4 }, () => ensureGitBinary(manifest(), options)));
  assert.equal(new Set(paths).size, 1); assert.equal(state.downloads, 1);
  assert.deepEqual(await readFile(paths[0]), bytes);
  assert.equal(await ensureGitBinary(manifest(), { ...options, env: { SPEC_AUTONOMOUS_OFFLINE: '1' }, find: () => null }), paths[0]);
});
for (const [key, message] of [['wrongHash', /integrity_mismatch/], ['wrongVersion', /version_mismatch/], ['fail', /github_download_failed/]] as [FailureKey, RegExp][]) {
  test(`${key} never leaves a runnable cache entry and a later request retries`, async t => {
    const { options, state } = await setup(t); state[key] = true;
    await assert.rejects(ensureGitBinary(manifest(), options), message);
    for (const dir of await readdir(options.cache)) assert.equal(existsSync(join(options.cache, dir, 'spec-autonomous')), false);
    state[key] = false;
    const file = await ensureGitBinary(manifest(), options); assert.deepEqual(await readFile(file), bytes);
    assert.equal(state.downloads, 2);
  });
}
test('missing gh, offline and unavailable platform fail before any downloads', async t => {
  const { options, state } = await setup(t);
  await assert.rejects(ensureGitBinary(manifest(), { ...options, find: () => null }), /github_cli_missing/);
  await assert.rejects(ensureGitBinary(manifest(), { ...options, env: { SPEC_AUTONOMOUS_OFFLINE: '1' } }), /git_binary_offline/);
  await assert.rejects(ensureGitBinary(manifest(), { ...options, platform: { key: 'linux-x64' } as Platform }), /git_binary_unavailable/);
  assert.equal(state.downloads, 0);
});
test('only the root Git facade opts in; reading delivery metadata never downloads', async t => {
  const { root, options, state } = await setup(t);
  await writeFile(join(root, 'package.json'), JSON.stringify({ name: 'spec-autonomous', private: false, bin: { 'spec-autonomous': 'bin/spec-autonomous.mjs' } }));
  assert.equal(await gitDelivery(root), null);
  await writeFile(join(root, 'package.json'), JSON.stringify({ name: 'spec-autonomous', private: true, version: '0.1.0-alpha.5', bin: { 'spec-autonomous': 'packages/cli/bin/spec-autonomous.mjs' } }));
  await assert.rejects(gitDelivery(root), /manifest_missing/);
  await writeFile(join(root, 'git-install.json'), JSON.stringify(manifest()));
  assert.equal((await gitDelivery(root))!.repository, 'owner/repo');
  assert.equal(state.downloads, 0);
  assert.ok(await ensureGitBinary((await gitDelivery(root))!, options)); assert.equal(state.downloads, 1);
});
test('manifest versions and asset paths are validated before use', async t => {
  const { root, options } = await setup(t);
  await writeFile(join(root, 'package.json'), JSON.stringify({ name: 'spec-autonomous', private: true, version: '0.1.0-alpha.5', bin: { 'spec-autonomous': 'packages/cli/bin/spec-autonomous.mjs' } }));
  await writeFile(join(root, 'git-install.json'), JSON.stringify({ ...manifest(), version: '../outside' }));
  await assert.rejects(gitDelivery(root), /manifest_invalid/);
  const bad = manifest(); bad.assets['darwin-arm64'].name = '../outside';
  await assert.rejects(ensureGitBinary(bad, options), /manifest_invalid/);
});
