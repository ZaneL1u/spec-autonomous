import test from 'node:test';
import assert from 'node:assert/strict';
import { spawnSync, spawn } from 'node:child_process';
import { mkdtempSync, writeFileSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { once } from 'node:events';
import { platformFor } from '../lib/platform.mjs';

// These two paths stay `.mjs`: they are the committed build output that users
// actually execute, so the launcher contract is verified against the artifact.
const launcher = fileURLToPath(new URL('../bin/spec-autonomous.mjs', import.meta.url));
const forwardModule = new URL('../lib/cli-process.mjs', import.meta.url).href;
const forwardScript = `import {forwardProcess} from ${JSON.stringify(forwardModule)}; process.exitCode = await forwardProcess([process.execPath, ...process.argv.slice(1)]);`;

test('rejects unsupported architecture and musl explicitly', () => {
  assert.throws(() => platformFor('linux', 's390x', '2.35'), /Unsupported/);
  assert.throws(() => platformFor('linux', 'x64', ''), /musl/);
  assert.equal(platformFor('win32', 'arm64').executable, 'spec-autonomous.exe');
});

test('preserves argument boundaries and child exit code without shell interpretation', (t) => {
  const root = mkdtempSync(join(tmpdir(), 'spec autonomous wrapper '));
  t.after(() => rmSync(root, { recursive: true, force: true }));
  const fixture = join(root, 'worker.mjs');
  writeFileSync(fixture, 'console.log(JSON.stringify(process.argv.slice(2))); process.exit(7);');
  const args = ['a b', '$(not-a-command)', '`not-a-command`', '--json'];
  const result = spawnSync(process.execPath, ['--input-type=module', '-e', forwardScript, fixture, ...args], { encoding: 'utf8' });
  assert.equal(result.status, 7);
  assert.deepEqual(JSON.parse(result.stdout), args);
});

test('missing executable produces actionable failure', () => {
  const result = spawnSync(process.execPath, [launcher], { encoding: 'utf8', env: { ...process.env, SPEC_AUTONOMOUS_BINARY: join(tmpdir(), 'spec-autonomous-does-not-exist', 'binary') } });
  assert.equal(result.status, 1);
  assert.match(result.stderr, /ENOENT/);
});

test('forwards SIGTERM to the native child', { skip: process.platform === 'win32', timeout: 10000 }, async (t) => {
  const root = mkdtempSync(join(tmpdir(), 'spec-autonomous-signal-'));
  t.after(() => rmSync(root, { recursive: true, force: true }));
  const fixture = join(root, 'worker.mjs');
  writeFileSync(fixture, "process.on('SIGTERM', () => process.exit(42)); console.log('ready'); setInterval(() => {}, 1000);");
  const child = spawn(process.execPath, ['--input-type=module', '-e', forwardScript, fixture], { stdio: ['ignore', 'pipe', 'pipe'] });
  t.after(() => { if (child.exitCode === null) child.kill('SIGKILL'); });
  const exit = once(child, 'exit');
  await once(child.stdout!, 'data');
  child.kill('SIGTERM');
  assert.equal((await exit)[0], 42);
});
