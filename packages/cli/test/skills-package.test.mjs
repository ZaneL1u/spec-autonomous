import test from 'node:test';
import assert from 'node:assert/strict';
import { existsSync, mkdtempSync, readFileSync, readdirSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { npmCommand } from '../../../scripts/npm-command.mjs';

const source = fileURLToPath(new URL('../', import.meta.url));
const names = ['auto', 'autonomous', 'milestone', 'progress', 'resume'];

test('all five public entry points ship as CLI-owned skill assets', () => {
  const manifest = JSON.parse(readFileSync(join(source, 'package.json'), 'utf8'));
  assert.ok(manifest.files.includes('skills'));
  assert.deepEqual(readdirSync(join(source, 'skills')).sort(), names);
  for (const name of names) {
    const content = readFileSync(join(source, 'skills', name, 'SKILL.md'), 'utf8');
    assert.match(content, new RegExp(`^---\\r?\\nname: ${name}\\r?\\n`));
    assert.match(content, /description: .+/);
    assert.match(content, /sa_(prepare|progress|next|inspect|apply_result)/);
    assert.doesNotMatch(content, /codex exec|claude -p|runner\.profile/);
  }
});

test('auto uses the same host-owned capability and receipt protocol as autonomous', () => {
  const canonical = readFileSync(join(source, 'skills/autonomous/SKILL.md'), 'utf8');
  const alias = readFileSync(join(source, 'skills/auto/SKILL.md'), 'utf8');
  for (const entry of ['sa_prepare','sa_apply_result','work.claim','fresh','host']) {
    assert.ok(canonical.includes(entry), `autonomous must describe ${entry}`);
    assert.ok(alias.includes(entry), `auto must describe ${entry}`);
  }
  assert.match(alias, /Exact alias/);assert.match(alias, /from\/to\/only/);
  assert.match(alias, /scope_completed/);assert.match(alias, /CLI never starts an agent/);
});

test('local npm tarball includes public skills when lifecycle scripts are disabled', (t) => {
  const output = mkdtempSync(join(tmpdir(), 'spec autonomous local skills '));
  t.after(() => rmSync(output, { recursive: true, force: true }));
  const result = npmCommand(['pack', '--ignore-scripts', '--json', '--workspaces=false', '--pack-destination', output], { cwd: source, encoding: 'utf8' });
  const [packed] = JSON.parse(result.stdout);
  assert.ok(existsSync(join(output, packed.filename)));
  const paths = new Set(packed.files.map((entry) => entry.path));
  for (const name of names) assert.ok(paths.has(`skills/${name}/SKILL.md`), `missing packed skill: ${name}`);
  assert.ok(paths.has('bin/spec-autonomous.mjs'));
  assert.equal([...paths].some((path) => path.includes('.references/') || path.includes('.spec-autonomous/')), false);
  const installed = join(output, 'installed');
  npmCommand(['install', '--prefix', installed, '--ignore-scripts', '--no-audit', '--no-fund', '--package-lock=false', join(output, packed.filename)], { encoding: 'utf8' });
  for (const name of names) {
    assert.deepEqual(
      readFileSync(join(installed, 'node_modules/spec-autonomous/skills', name, 'SKILL.md')),
      readFileSync(join(source, 'skills', name, 'SKILL.md')),
    );
  }
  assert.ok(existsSync(join(installed, 'node_modules/.bin', process.platform === 'win32' ? 'spec-autonomous.cmd' : 'spec-autonomous')));
});
