import test from 'node:test';
import assert from 'node:assert/strict';
import { mkdtempSync, mkdirSync, writeFileSync, readFileSync, existsSync, readdirSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { assemble } from '../package-release.mts';
import { npmCommand } from '../npm-command.mts';
import { platforms } from '../../packages/cli/lib/platform.mjs';

function filesIn(directory:string, prefix = ''):string[] {
  return readdirSync(directory, { withFileTypes: true }).flatMap((entry) => {
    const name = prefix + entry.name;
    return entry.isDirectory() ? filesIn(join(directory, entry.name), `${name}/`) : [name];
  }).sort();
}

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
  const release = assemble(root, output, {repository: 'https://github.com/example/spec-autonomous'});
  const wrapper = JSON.parse(readFileSync(join(output, 'spec-autonomous/package.json'), 'utf8')) as any;
  assert.equal(release.packages.length, 7);
  assert.equal(Object.keys(wrapper.optionalDependencies).length, 6);
  assert.ok(Object.values(wrapper.optionalDependencies).every((v) => v === wrapper.version));
  assert.equal(existsSync(join(output, 'spec-autonomous/native')), false);
  assert.ok(existsSync(join(output, 'spec-autonomous/locales/en.json')));
  assert.equal((JSON.parse(readFileSync(join(output, 'spec-autonomous/package.json'), 'utf8')) as any).repository.url, 'https://github.com/example/spec-autonomous');
  const linux = JSON.parse(readFileSync(join(output, 'spec-autonomous-linux-x64/package.json'), 'utf8')) as any;
  assert.equal(linux.repository.url, 'https://github.com/example/spec-autonomous');
  assert.deepEqual(linux.libc, ['glibc']);
  assert.deepEqual(linux.cpu, ['x64']);
  assert.equal(readFileSync(join(output, 'SHA256SUMS'), 'utf8').trim().split('\n').length, 6);
  assert.throws(() => assemble(root, output), /already exists/);
});

test('release wrapper tarball contains every skill asset and no platform binaries', (t) => {
  const root = mkdtempSync(join(tmpdir(), 'spec autonomous skills release '));
  t.after(() => rmSync(root, { recursive: true, force: true }));
  for (const p of platforms) {
    mkdirSync(join(root, p.target));
    writeFileSync(join(root, p.target, p.executable), `fixture-${p.key}`);
  }
  const output = join(root, 'out');
  assemble(root, output, {repository: 'https://github.com/example/spec-autonomous'});
  const wrapper = join(output, 'spec-autonomous');
  const sourceSkills = fileURLToPath(new URL('../../packages/cli/skills/', import.meta.url));
  const skills = filesIn(sourceSkills);
  assert.deepEqual(filesIn(join(wrapper, 'skills')), skills);
  for (const path of skills) {
    assert.deepEqual(readFileSync(join(wrapper, 'skills', path)), readFileSync(join(sourceSkills, path)));
  }
  const result = npmCommand(['pack', '--ignore-scripts', '--json', '--pack-destination', root], { cwd: wrapper, encoding: 'utf8' });
  const [packed] = JSON.parse(result.stdout) as Array<{filename:string;files:Array<{path:string}>}>;
  const paths = packed!.files.map((entry) => entry.path);
  assert.ok(existsSync(join(root, packed!.filename)));
  assert.deepEqual(paths.filter((path) => path.startsWith('skills/')).sort(), skills.map((path) => `skills/${path}`));
  assert.equal(paths.some((path) => path.startsWith('native/')), false);
  assert.equal(paths.some((path) => path.endsWith('.exe')), false);
});
