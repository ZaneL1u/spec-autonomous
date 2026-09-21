import test from 'node:test';
import assert from 'node:assert/strict';
import { existsSync, mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import { spawnSync } from 'node:child_process';
import { dirname } from 'node:path';
import { pathToFileURL } from 'node:url';
import { fixture, cli, start, until, git, edit, join, resolve, workspace } from './helpers.mts';

const milestonePath = '.spec-autonomous/milestones/M001/milestone.toml';
const mockAgent = pathToFileURL(join(workspace, 'tests/mock-agent.mts')).href;

function failure(result: any) {
  return result.data?.blocker ?? result.output?.error?.message ?? result.details;
}

function put(root: any, path: any, body: any) {
  const target = join(root, path);
  mkdirSync(dirname(target), { recursive: true });
  writeFileSync(target, body);
}

function commit(root: any, message = 'test: configure recovery boundary') {
  git(root, ['add', '--all']);
  git(root, ['commit', '-qm', message]);
}

// Fault injection stays in this temporary repository. The shared mock runner is
// imported unchanged, so its normal planning, implementation and checks still run.
function driver(root: any, body: any) {
  const path = join(root, '.mock/recovery-driver.mjs');
  writeFileSync(path, `import assert from 'node:assert/strict';
import { readFileSync, writeFileSync, realpathSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';
const input = JSON.parse(readFileSync(process.env.SPEC_AUTONOMOUS_INPUT, 'utf8'));
${body}
`);
  edit(root, '.spec-autonomous/config.toml', text => text.replace(
    /^command = .*$/m,
    `command = ${JSON.stringify([process.execPath, path])}`,
  ));
}

function setPhaseCheck(root: any, index: any, argv: any) {
  edit(root, milestonePath, text => {
    const parts = text.split('\n[[phases]]');
    assert.ok(parts[index + 1], 'fixture phase must exist');
    parts[index + 1] = parts[index + 1].replace(
      /^verification = .*$/m,
      `verification = [{argv = ${JSON.stringify(argv)}, cwd = "."}]`,
    );
    return parts.join('\n[[phases]]');
  });
}

function attemptDirectory(root: any, run: any, attempt: any) {
  const common = git(root, ['rev-parse', '--path-format=absolute', '--git-common-dir']);
  return join(common, 'spec-autonomous/runs', run.id, 'attempts', attempt.id);
}

function alive(pid: any) {
  const result = spawnSync('ps', ['-p', String(pid), '-o', 'stat='], { encoding: 'utf8' });
  return result.status === 0 && result.stdout.trim() !== '' && !result.stdout.trim().startsWith('Z');
}

function ownProcess(t: any, running: any) {
  const groups = new Set<number>();
  const descendants = new Set<number>();
  t.after(() => {
    for (const pid of groups) {
      try { process.kill(-pid, 'SIGKILL'); } catch {}
    }
    for (const pid of descendants) {
      try { process.kill(pid, 'SIGKILL'); } catch {}
    }
    if (running.child.exitCode === null) running.child.kill('SIGKILL');
  });
  return { groups, descendants };
}

async function observeHungAttempt(root: any, kind: any, owned: any) {
  return until(() => {
    const status = cli(root, ['status']);
    if (status.status !== 0) return null;
    const attempt = status.data.attempts.find((a: any) => a.kind === kind && a.status === 'claimed');
    if (!attempt) return null;
    const directory = attemptDirectory(root, status.data, attempt);
    const processPath = join(directory, 'host-process.json');
    const grandchildPath = join(directory, 'grandchild.pid');
    if (!existsSync(processPath) || !existsSync(grandchildPath)) return null;
    const process = JSON.parse(readFileSync(processPath, 'utf8'));
    const grandchild = Number(readFileSync(grandchildPath, 'utf8'));
    owned.groups.add(process.pid);
    owned.descendants.add(grandchild);
    return { run: status.data, attempt, process, grandchild };
  }, 45000);
}

test('native handoff and resume retain the selected phase and committed native edits', { timeout: 120000 }, t => {
  const root = fixture(t, 'speckit');
  // Existing primitives make the calculator phase independently runnable. Its
  // earlier roadmap phase deliberately remains unfinished and outside the range.
  const scenario = JSON.parse(readFileSync(join(root, '.mock/scenario.json'), 'utf8'));
  for (const task of scenario.phases[0].tasks) put(root, task.path, task.content);
  edit(root, milestonePath, text => text.replace('depends_on = ["P001"]', 'depends_on = []'));
  commit(root);

  const handoff = cli(root, ['run', '--milestone', 'M001', '--only', 'P002', '--mode', 'native']);
  assert.equal(handoff.status, 0, handoff.details);
  assert.equal(handoff.data.status, 'handed_off');
  assert.deepEqual(handoff.data.selected_phases, ['P002']);
  assert.match(handoff.data.blocker, /specs\/calculator/);
  assert.doesNotMatch(handoff.data.blocker, /specs\/arithmetic/);
  assert.equal(handoff.data.attempts.length, 0);

  put(handoff.data.integration, 'NATIVE-NOTES.md', 'Native work retained across the handoff.\n');
  commit(handoff.data.integration, 'test: native user advances the handed-off checkout');
  const resumed = cli(root, ['resume', handoff.data.id, '--mode', 'autonomous']);
  assert.equal(resumed.status, 0, resumed.details);
  assert.equal(resumed.data.id, handoff.data.id);
  assert.equal(resumed.data.status, 'scope_completed');
  assert.deepEqual(resumed.data.selected_phases, ['P002']);
  assert.deepEqual(resumed.data.completed_phases, ['P002']);
  assert.ok(resumed.data.attempts.every((a: any) => a.phase_id === 'P002'));
  assert.equal(readFileSync(join(root, 'NATIVE-NOTES.md'), 'utf8'), 'Native work retained across the handoff.\n');
  assert.ok(existsSync(join(root, 'src/service.mjs')));
  assert.equal(existsSync(join(root, 'specs/arithmetic/tasks.md')), false);
});

test('completed phase evidence is invalidated by code or phase-verification changes', { timeout: 180000 }, t => {
  for (const change of ['code', 'verification']) {
    const root = fixture(t, 'speckit');
    const first = cli(root, ['run', '--milestone', 'M001', '--only', 'P001']);
    assert.equal(first.status, 0, first.details);
    assert.equal(first.data.status, 'scope_completed');
    const tasks = readFileSync(join(root, 'specs/arithmetic/tasks.md'), 'utf8');
    if (change === 'code') {
      put(root, 'src/add.mjs', 'export const add = (a, b) => a - b;\n');
    } else {
      setPhaseCheck(root, 0, [process.execPath, '-e', 'throw new Error("NEW_PHASE_ACCEPTANCE")']);
    }
    commit(root, `test: invalidate completed phase ${change}`);
    const origin = git(root, ['rev-parse', 'HEAD']);

    const rerun = cli(root, ['run', '--milestone', 'M001', '--only', 'P001']);
    assert.notEqual(rerun.status, 0, `${change}: ${rerun.details}`);
    assert.ok(rerun.data, rerun.details);
    assert.ok(!rerun.data.completed_phases.includes('P001'), rerun.details);
    assert.ok(rerun.data.evidence.some((e: any) => e.exit_code !== 0), `${change}: expected fresh failing verification; ${failure(rerun)}`);
    if (change === 'verification') {
      assert.ok(rerun.data.evidence.some((e: any) => e.argv.some((arg: any) => arg.includes('NEW_PHASE_ACCEPTANCE'))));
    }
    assert.equal(git(root, ['rev-parse', 'HEAD']), origin);
    assert.equal(readFileSync(join(root, 'specs/arithmetic/tasks.md'), 'utf8'), tasks);
  }
});

test('an unsupported mandatory after hook is rejected before any implementation or planning worker', { timeout: 30000 }, t => {
  const root = fixture(t, 'speckit');
  put(root, '.specify/extensions.yml', `hooks:
  after_implement:
    - extension: missing-verifier
      command: speckit.missing.verify
      optional: false
      enabled: true
`);
  commit(root);
  const origin = git(root, ['rev-parse', 'HEAD']);
  const result = cli(root, ['run', '--milestone', 'M001', '--only', 'P001'], { env: { SPEC_AUTONOMOUS_LANG: 'en' } });
  assert.notEqual(result.status, 0, result.details);
  assert.match(failure(result), /hook_unsupported|mandatory.*hook|hook.*capability/);
  assert.match(failure(result), /speckit\.missing\.verify/);
  if (result.data?.attempts) assert.equal(result.data.attempts.length, 0);
  const progress = cli(root, ['progress', '--all-worktrees']);
  assert.equal(progress.status, 0, progress.details);
  assert.equal(progress.data.worktrees.filter((w: any) => w.kind === 'managed-worker').length, 0);
  assert.equal(git(root, ['rev-parse', 'HEAD']), origin);
  assert.equal(existsSync(join(root, 'src/add.mjs')), false);
  assert.equal(existsSync(join(root, 'specs/arithmetic/tasks.md')), false);
});

test('Spec Kit workers replace inherited origin feature paths with their own checkout paths', { timeout: 120000 }, t => {
  const root = fixture(t, 'speckit');
  driver(root, `if (input.framework === 'speckit' && input.snapshot) {
  assert.equal(realpathSync(process.env.SPECIFY_INIT_DIR), realpathSync(process.cwd()));
  assert.equal(resolve(process.env.SPECIFY_FEATURE_DIRECTORY), resolve(process.cwd(), input.snapshot.selector));
  assert.notEqual(realpathSync(process.env.SPECIFY_INIT_DIR), realpathSync(${JSON.stringify(root)}));
  writeFileSync(join(dirname(process.env.SPEC_AUTONOMOUS_INPUT), 'observed-env.json'), JSON.stringify({
    root: process.env.SPECIFY_INIT_DIR, feature: process.env.SPECIFY_FEATURE_DIRECTORY, cwd: process.cwd(), selector: input.snapshot.selector
  }));
}
await import(${JSON.stringify(mockAgent)});`);
  put(root, '.specify/feature.json', '{"feature_directory":"specs/origin-selection"}\n');
  commit(root);
  const sourceFeature = readFileSync(join(root, '.specify/feature.json'), 'utf8');
  const result = cli(root, ['run', '--milestone', 'M001', '--only', 'P001'], {
    env: { SPECIFY_INIT_DIR: root, SPECIFY_FEATURE_DIRECTORY: join(root, 'specs/origin-selection'), SPECIFY_FEATURE: 'origin-label' },
  });
  assert.equal(result.status, 0, result.details);
  assert.equal(result.data.status, 'scope_completed');
  const observedKinds = new Set();
  for (const attempt of result.data.attempts) {
    const path = join(attemptDirectory(root, result.data, attempt), 'observed-env.json');
    if (!existsSync(path)) continue;
    const observed = JSON.parse(readFileSync(path, 'utf8'));
    assert.equal(resolve(observed.root), resolve(observed.cwd));
    assert.equal(resolve(observed.feature), resolve(attempt.worktree, observed.selector));
    observedKinds.add(attempt.kind);
  }
  assert.deepEqual([...observedKinds].sort(), ['audit', 'implement', 'native-planning', 'plan-tasks']);
  assert.equal(readFileSync(join(root, '.specify/feature.json'), 'utf8'), sourceFeature);
});

test('host pause stops an external audit and its descendants promptly', { timeout: 90000, skip: process.platform === 'win32' }, async t => {
  const root = fixture(t, 'speckit');
  driver(root, `if (input.kind === 'audit') process.env.MOCK_HANG = '1';
await import(${JSON.stringify(mockAgent)});`);
  commit(root);
  const running = start(root, ['run', '--milestone', 'M001', '--only', 'P001']);
  const owned = ownProcess(t, running);
  const hung = await observeHungAttempt(root, 'audit', owned);
  assert.ok(alive(hung.grandchild));
  const requestedAt = Date.now();
  const request = cli(root, ['pause', hung.run.id]);
  assert.equal(request.status, 0, request.details);
  assert.equal(request.data.requested, 'pause');
  await until(() => running.child.exitCode !== null, 8000);
  const stopped = await running.finished;
  assert.equal(stopped.status, 4, stopped.details);
  assert.match(stopped.data.blocker, /pause|interrupt/);
  assert.ok(Date.now() - requestedAt < 8000, 'pause waited for the full 20-second attempt timeout');
  await until(() => !alive(hung.process.pid) && !alive(hung.grandchild), 3000);
  assert.ok(!stopped.data.completed_phases.includes('P001'));
});

test('the external host honors the run deadline before the work timeout', { timeout: 30000, skip: process.platform === 'win32' }, async t => {
  const root = fixture(t, 'speckit', { goalOnly: true });
  driver(root, `if (input.kind === 'roadmap') process.env.MOCK_HANG = '1';
await import(${JSON.stringify(mockAgent)});`);
  edit(root, '.spec-autonomous/config.toml', text => text.replace('run_timeout_seconds = 180', 'run_timeout_seconds = 3'));
  commit(root);
  const startedAt = Date.now();
  const running = start(root, ['milestone', 'new', 'Build a tested arithmetic service', '--id', 'MBUDGET', '--mode', 'autonomous']);
  const owned = ownProcess(t, running);
  const hung = await observeHungAttempt(root, 'roadmap', owned);
  await until(() => running.child.exitCode !== null, 8000);
  const stopped = await running.finished;
  assert.equal(stopped.status, 4, stopped.details);
  assert.match(stopped.data.blocker, /budget_exhausted|deadline/);
  assert.ok(Date.now() - startedAt < 9000, 'the run used the 20-second attempt timeout instead of its total budget');
  await until(() => !alive(hung.process.pid) && !alive(hung.grandchild), 3000);
  assert.equal(stopped.data.completed_phases.length, 0);
});

test('phase verification cannot mutate tracked code and still deliver the untested accepted revision', { timeout: 120000 }, t => {
  const root = fixture(t, 'speckit');
  put(root, '.mock/mutate-during-verification.mjs', `import { writeFileSync } from 'node:fs';
writeFileSync('src/add.mjs', 'export const add = () => 999;\\n');
`);
  setPhaseCheck(root, 0, [process.execPath, '.mock/mutate-during-verification.mjs']);
  commit(root);
  const origin = git(root, ['rev-parse', 'HEAD']);
  const result = cli(root, ['run', '--milestone', 'M001', '--only', 'P001']);
  assert.notEqual(result.status, 0, result.details);
  assert.match(failure(result), /verification_mutated_tree|verification.*mutat/);
  assert.ok(result.data && !result.data.completed_phases.includes('P001'), result.details);
  assert.equal(git(root, ['rev-parse', 'HEAD']), origin);
  assert.equal(existsSync(join(root, 'src/add.mjs')), false);
  assert.equal(git(root, ['status', '--porcelain']), '');
});

test('an unrelated passing audit cannot establish native requirement coverage', { timeout: 120000 }, t => {
  const root = fixture(t, 'speckit');
  driver(root, `if (input.kind === 'audit') {
  writeFileSync(process.env.SPEC_AUTONOMOUS_RESULT, JSON.stringify({
    schema_version: 1, run_id: input.run_id, task_id: input.task_id, attempt_id: input.attempt_id,
    status: 'candidate', summary: 'Unrelated audit fixture', blockers: [], milestone: null, plan: null,
    audit: [{ requirement: 'Unrelated checkout decoration', evidence: 'README.md', passed: true }]
  }));
} else {
  await import(${JSON.stringify(mockAgent)});
}`);
  commit(root);
  const origin = git(root, ['rev-parse', 'HEAD']);
  const result = cli(root, ['run', '--milestone', 'M001', '--only', 'P001']);
  assert.notEqual(result.status, 0, result.details);
  assert.match(failure(result), /audit|coverage|requirement|repair_budget/);
  assert.ok(result.data && !result.data.completed_phases.includes('P001'), result.details);
  assert.ok(result.data.attempts.some((a: any) => a.kind === 'audit'), 'must reach the syntactically valid audit result');
  assert.equal(git(root, ['rev-parse', 'HEAD']), origin);
});
