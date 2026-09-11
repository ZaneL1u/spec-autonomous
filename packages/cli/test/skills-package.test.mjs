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
    assert.match(content, /spec-autonomous /);
  }
});

test('auto delegates the same CLI, scope and resume contract as autonomous', () => {
  const canonical = readFileSync(join(source, 'skills/autonomous/SKILL.md'), 'utf8');
  const alias = readFileSync(join(source, 'skills/auto/SKILL.md'), 'utf8');
  for (const command of [
    'spec-autonomous detect --json',
    'spec-autonomous progress --all-worktrees --json',
    'spec-autonomous run --milestone <id> --mode autonomous',
    'spec-autonomous milestone new "<goal>" --mode autonomous',
  ]) {
    assert.ok(canonical.includes(command), `autonomous must use ${command}`);
    assert.ok(alias.includes(command), `auto must use ${command}`);
  }
  assert.match(alias, /exactly the `autonomous` entry point/);
  assert.match(alias, /from\/to\/only/);
  assert.match(alias, /spec-autonomous resume <run-id>/);
  assert.match(alias, /Do not create a separate shortcut loop/);
  assert.match(alias, /do not infer whole-milestone completion from a completed range/);
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
