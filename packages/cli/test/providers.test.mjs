import test from 'node:test';
import assert from 'node:assert/strict';
import { mkdtemp, mkdir, writeFile, readFile, rm, readdir } from 'node:fs/promises';
import { existsSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, dirname } from 'node:path';
import { createHash } from 'node:crypto';
import { createProviderManager, downloadVerified, withInstallLock } from '../lib/providers.mjs';
import { versions, uvHashes } from '../lib/provider-versions.mjs';
import { mergeScaffold, mcpNeedsProvider, needsProvider, createProviderContext, cliSource } from '../lib/provider-cli.mjs';
import { runProcess } from '../lib/provider-process.mjs';

async function setup(t, options = {}) {
  const home = await mkdtemp(join(tmpdir(), 'sa providers unit '));
  t.after(() => rm(home, { recursive: true, force: true }));
  const calls = [];
  const run = async (argv, opts = {}) => {
    calls.push({ argv, opts });
    if (argv.includes('install') && argv.some(a => a.startsWith('@fission-ai/openspec@'))) {
      const file = join(opts.cwd, 'node_modules/@fission-ai/openspec/bin/openspec.js');
      await mkdir(dirname(file), { recursive: true }); await writeFile(file, '// installed');
      if (options.fail) return { code: 1, stdout: '', stderr: 'mock installation failed' };
    }
    if (argv.includes('tool') && argv.includes('install')) {
      const file = join(opts.env.UV_TOOL_BIN_DIR, process.platform === 'win32' ? 'specify.exe' : 'specify');
      await mkdir(dirname(file), { recursive: true }); await writeFile(file, 'specify fixture');
    }
    return { code: options.badProbe && ['--version', 'version'].includes(argv.at(-1)) ? 1 : 0,
      stdout: argv.at(-1) === 'version' ? `Spec Kit CLI: ${versions.speckit}` : argv[0].includes('uv') ? `uv ${versions.uv}` : versions.openspec, stderr: '' };
  };
  const env = { ...process.env, PATH: '', ...options.env };
  const manager = createProviderManager({ env, home, find: name => name === 'uv' && options.uv ? '/mock/uv' : null, run, log: () => {} });
  return { home, calls, manager, options };
}

test('missing OpenSpec installs once, validates and reuses an isolated receipt', async t => {
  const { manager, home, calls } = await setup(t);
  assert.equal((await manager.status('openspec', { root: home })).source, 'missing');
  assert.equal(existsSync(join(home, 'openspec')), false);
  const first = await manager.ensure('openspec', { root: home });
  assert.equal(first.ready, true); assert.equal(first.source, 'managed');
  const second = await manager.ensure('openspec', { root: home });
  assert.deepEqual(first.command, second.command);
  assert.equal(calls.filter(c => c.argv.includes('install')).length, 1);
  assert.equal(existsSync(join(home, 'package.json')), false);
});
test('parallel worktree requests converge on one verified installation', async t => {
  const { manager, home, calls } = await setup(t);
  const results = await Promise.all(Array.from({ length: 6 }, () => manager.ensure('openspec', { root: home })));
  assert.equal(new Set(results.map(r => r.command.at(-1))).size, 1);
  assert.equal(calls.filter(c => c.argv.includes('install')).length, 1);
});
test('failed install leaves no receipt and retry uses a fresh generation', async t => {
  const fixture = await setup(t, { fail: true });
  await assert.rejects(fixture.manager.ensure('openspec', { root: fixture.home }), /provider_install_failed/);
  assert.equal(existsSync(join(fixture.home, 'openspec/current.json')), false);
  fixture.options.fail = false;
  assert.equal((await fixture.manager.ensure('openspec', { root: fixture.home })).ready, true);
  assert.equal((await readdir(join(fixture.home, 'openspec'))).length, 3);
});
test('offline missing provider never invokes installer or creates state', async t => {
  const { manager, home, calls } = await setup(t, { env: { SPEC_AUTONOMOUS_OFFLINE: '1' } });
  await assert.rejects(manager.ensure('openspec', { root: home }), /provider_offline/);
  assert.equal(calls.length, 0); assert.deepEqual(await readdir(home), []);
});
test('configured command is preserved; a failing existing tool is not replaced', async t => {
  const fixture = await setup(t);
  const command = [process.execPath, 'custom openspec.mjs', '--literal'];
  const ready = await fixture.manager.ensure('openspec', { root: fixture.home, command });
  assert.equal(ready.source, 'configured'); assert.deepEqual(ready.command, command);
  assert.equal(fixture.calls.length, 1); assert.equal(fixture.calls[0].opts.cwd, fixture.home);
  fixture.options.badProbe = true;
  await assert.rejects(fixture.manager.ensure('openspec', { root: fixture.home, command }), /provider_unusable/);
  assert.equal(existsSync(join(fixture.home, 'openspec')), false);
});
test('Spec Kit installs with dedicated tool, binary, Python and cache directories', async t => {
  const { manager, home, calls } = await setup(t, { uv: true });
  const result = await manager.ensure('speckit', { root: home });
  assert.equal(result.ready, true);
  const install = calls.find(c => c.argv.includes('tool'));
  assert.ok(install.argv.includes(`specify-cli==${versions.speckit}`));
  for (const key of ['UV_TOOL_DIR', 'UV_TOOL_BIN_DIR', 'UV_PYTHON_INSTALL_DIR', 'UV_CACHE_DIR']) assert.ok(install.opts.env[key].startsWith(home));
  assert.equal(install.opts.env.UV_PYTHON_DOWNLOADS, 'automatic');
});
test('uv download rejects mismatched bytes before writing any archive', async t => {
  const { home } = await setup(t), file = join(home, 'uv.tar.gz'), bytes = Buffer.from('fake archive');
  const fakeFetch = async () => new Response(bytes);
  await assert.rejects(downloadVerified('https://example.invalid/uv', file, '0'.repeat(64), fakeFetch), /provider_integrity_mismatch/);
  assert.equal(existsSync(file), false);
  await downloadVerified('https://example.invalid/uv', file, createHash('sha256').update(bytes).digest('hex'), fakeFetch);
  assert.deepEqual(await readFile(file), bytes);
  assert.equal(Object.keys(uvHashes).length, 6);
  assert.ok(Object.values(uvHashes).every(hash => /^[a-f0-9]{64}$/.test(hash)));
});
test('dead installer lock is recoverable; live locks time out without being removed', async t => {
  const { home } = await setup(t), lock = join(home, 'lock');
  await mkdir(lock); await writeFile(join(lock, 'owner.json'), JSON.stringify({ pid: 99999999, child: null }));
  assert.equal(await withInstallLock(lock, async () => 'recovered'), 'recovered');
  await mkdir(lock); await writeFile(join(lock, 'owner.json'), JSON.stringify({ pid: process.pid, child: null }));
  await assert.rejects(withInstallLock(lock, async () => assert.fail(), { timeout: 30 }), /provider_busy/);
  assert.equal(existsSync(lock), true);
});
test('scaffolding preflights all files and never overwrites unrelated project content', async t => {
  const { home } = await setup(t), source = join(home, 'source'), target = join(home, 'target');
  await mkdir(source); await mkdir(target);
  await writeFile(join(source, 'a-new.md'), 'new'); await writeFile(join(source, 'z-existing.md'), 'template');
  await writeFile(join(target, 'z-existing.md'), 'user content');
  await assert.rejects(mergeScaffold(source, target), /provider_init_conflict/);
  assert.equal(existsSync(join(target, 'a-new.md')), false);
  assert.equal(await readFile(join(target, 'z-existing.md'), 'utf8'), 'user content');
  await writeFile(join(source, 'z-existing.md'), 'user content');
  assert.equal((await mergeScaffold(source, target)).length, 1);
  assert.equal((await mergeScaffold(source, target)).length, 0);
});
test('passive operations stay offline', () => {
  for (const name of ['progress', 'doctor', 'next', 'detect']) assert.equal(needsProvider(name), false);
  assert.equal(mcpNeedsProvider({ method: 'tools/call', params: { name: 'sa_progress' } }), false);
  assert.equal(mcpNeedsProvider({ method: 'tools/call', params: { name: 'sa_prepare' } }), true);
  assert.equal(mcpNeedsProvider({ method: 'tools/call', params: { name: 'sa_tools', arguments: { operation: 'call', capability: 'native.instructions' } } }), true);
});
test('structured source and continuation retain their explicit or ledger provider', async t => {
  const { home } = await setup(t); const ensured = [];
  const manager = { env: {}, ensure: async name => { ensured.push(name); return { provider: name, ready: true }; },
    run: async argv => ({ code: 0, stdout: JSON.stringify(argv.includes('status')
      ? { data: { milestone: { framework: 'speckit' } } }
      : { root: home, selected: null, detected: [{ framework: 'speckit' }, { framework: 'openspec' }] }) }) };
  const context = createProviderContext('/mock/native', { path: home }, manager);
  await context.ensureSource({ run_id: 'run-1' });
  await context.ensureSource(await cliSource({ command: { name: 'tools', arguments: { command: { name: 'call', arguments: { input: '{"framework":"openspec"}' } } } } }));
  assert.deepEqual(ensured, ['speckit', 'openspec']);
  await assert.rejects(context.ensureSource({ run_id: 'run-1', framework: 'openspec' }), /provider_selection_conflict/);
  assert.throws(() => createProviderContext('/mock/native', { provider: 'openspec', framework: 'speckit' }, manager), /provider_selection_conflict/);
});
test('installer subprocesses have bounded deadlines and keep nonzero exit results', { timeout: 5000 }, async () => {
  const failed = await runProcess([process.execPath, '-e', 'process.exit(23)']);
  assert.equal(failed.code, 23);
  await assert.rejects(runProcess([process.execPath, '-e', 'setInterval(()=>{},1000)'], { timeout: 80 }), /provider_timeout/);
});
test('cancelled version probes stop setup instead of falling through to installation', async t => {
  const { home } = await setup(t); let calls = 0;
  const manager = createProviderManager({ home, find: () => null, run: async () => { calls++; throw new Error('provider_cancelled: SIGTERM'); } });
  await assert.rejects(manager.ensure('openspec', { root: home, command: ['custom-openspec'] }), /provider_cancelled/);
  assert.equal(calls, 1); assert.equal(existsSync(join(home, 'openspec')), false);
});
test('an interrupted stale-lock reaper is itself recoverable', async t => {
  const { home } = await setup(t), lock = join(home, 'lock');
  await mkdir(join(lock, 'reaping'), { recursive: true });
  await writeFile(join(lock, 'owner.json'), JSON.stringify({ pid: 99999999, child: null }));
  await writeFile(join(lock, 'reaping/owner.json'), JSON.stringify({ pid: 99999999 }));
  assert.equal(await withInstallLock(lock, async () => 'recovered', { timeout: 1000 }), 'recovered');
});
