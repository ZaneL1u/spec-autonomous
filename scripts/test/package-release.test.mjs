import test from 'node:test';
import assert from 'node:assert/strict';
import { mkdtempSync, mkdirSync, writeFileSync, readFileSync, existsSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { assemble } from '../package-release.mjs';
import { platforms } from '../../packages/cli/lib/platform.mjs';

test('release refuses incomplete matrix without leaving partial output', (t) => {
  const root = mkdtempSync(join(tmpdir(), 'spec-autonomous-release-'));
  t.after(() => rmSync(root, { recursive: true, force: true }));
  assert.throws(() => assemble(root, join(root, 'out')), /Missing native artifact/);
  assert.equal(existsSync(join(root, 'out')), false);
});

test('release isolates platform packages and pins optional dependencies exactly', (t) => {
  const root = mkdtempSync(join(tmpdir(), 'spec-autonomous-release-'));
  t.after(() => rmSync(root, { recursive: true, force: true }));
  for (const p of platforms) {
    mkdirSync(join(root, p.target));
    writeFileSync(join(root, p.target, p.executable), `fixture-${p.key}`);
  }
  const output = join(root, 'out');
  const release = assemble(root, output);
  const wrapper = JSON.parse(readFileSync(join(output, 'spec-autonomous/package.json')));
  assert.equal(release.packages.length, 7);
  assert.equal(Object.keys(wrapper.optionalDependencies).length, 6);
  assert.ok(Object.values(wrapper.optionalDependencies).every((v) => v === wrapper.version));
  assert.equal(existsSync(join(output, 'spec-autonomous/native')), false);
  const linux = JSON.parse(readFileSync(join(output, 'spec-autonomous-linux-x64/package.json')));
  assert.deepEqual(linux.libc, ['glibc']);
  assert.deepEqual(linux.cpu, ['x64']);
  assert.equal(readFileSync(join(output, 'SHA256SUMS'), 'utf8').trim().split('\n').length, 6);
  assert.throws(() => assemble(root, output), /already exists/);
});
