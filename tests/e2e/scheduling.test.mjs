import test from 'node:test';
import assert from 'node:assert/strict';
import { existsSync, readFileSync, writeFileSync } from 'node:fs';
import { createHash } from 'node:crypto';
import { spawnSync } from 'node:child_process';
import { performance } from 'node:perf_hooks';
import { binary, cleanEnv, cli, edit, fixture, git, join } from './helpers.mjs';

const sleepMs = 600;

function sixIndependentTasks(root) {
  const scenarioPath = join(root, '.mock/scenario.json');
  const scenario = JSON.parse(readFileSync(scenarioPath, 'utf8'));
  // Keep add/multiply so the untouched second roadmap phase remains meaningful.
  // The four extra modules, like those two primitives, share no writable files.
  for (let n = 1; n <= 4; n++) {
    const name = `offset-${n}`;
    const path = `src/${name}.mjs`;
    const test = `tests/${name}.test.mjs`;
    scenario.phases[0].tasks.push({
      name,
      path,
      test,
      content: `export const offset = value => value + ${n};\n`,
      broken: `export const offset = value => value - ${n};\n`,
    });
    writeFileSync(join(root, test), `import assert from 'node:assert/strict';
import { offset } from '../${path}';
assert.equal(offset(7), ${7 + n});
assert.equal(offset(-3), ${-3 + n});
`);
  }
  const tasks = scenario.phases[0].tasks;
  const phaseCheck = { argv: [process.execPath, '--test', ...tasks.map(task => task.test)], cwd: '.' };
  scenario.phases[0].verification = [phaseCheck];
  scenario.milestone_verification = [{
    argv: [process.execPath, '--test', ...scenario.phases.flatMap(phase => phase.tasks.map(task => task.test))],
    cwd: '.',
  }];
  scenario.sleep_ms = sleepMs;
  scenario.fail_once = '';
  writeFileSync(scenarioPath, JSON.stringify(scenario, null, 2));
  edit(root, '.spec-autonomous/milestones/M001/milestone.toml', text => {
    const parts = text.split('\n[[phases]]');
    const render = check => `verification = [{argv = ${JSON.stringify(check.argv)}, cwd = "."}]`;
    parts[0] = parts[0].replace(/^verification = .*$/m, render(scenario.milestone_verification[0]));
    parts[1] = parts[1].replace(/^verification = .*$/m, render(phaseCheck));
    return parts.join('\n[[phases]]');
  });
  git(root, ['add', '--all']);
  git(root, ['commit', '-qm', 'test: six independent Spec Kit arithmetic tasks']);
  return tasks;
}

function concurrency(attempts) {
  const events = attempts.flatMap(attempt => {
    const start = Date.parse(attempt.started_at);
    const finish = Date.parse(attempt.finished_at);
    assert.ok(Number.isFinite(start) && Number.isFinite(finish), 'every implementation has durable start/end timestamps');
    assert.ok(finish > start, 'every implementation interval must be nonempty');
    return [{ at: start, delta: 1 }, { at: finish, delta: -1 }];
  });
  // Half-open intervals: a worker finishing at the next worker's start does not
  // count as overlap. These are coordinator-observed attempt intervals, not CPU time.
  events.sort((a, b) => a.at - b.at || a.delta - b.delta);
  let live = 0;
  let peak = 0;
  for (const event of events) {
    live += event.delta;
    assert.ok(live >= 0, 'attempt end cannot precede its start');
    peak = Math.max(peak, live);
  }
  assert.equal(live, 0);
  return peak;
}

test('six independent Spec Kit tasks respect worker limits and produce equivalent verified deliveries', { timeout: 180000 }, t => {
  const observations = [];
  let expectedTasks;
  for (const maxWorkers of [1, 3]) {
    const root = fixture(t, 'speckit', { sleepMs });
    const tasks = sixIndependentTasks(root);
    assert.equal(tasks.length, 6);
    assert.equal(new Set(tasks.map(task => task.path)).size, 6);
    if (expectedTasks) assert.deepEqual(tasks, expectedTasks, 'both runs must implement the same workload');
    expectedTasks = tasks;

    const before = git(root, ['rev-parse', 'HEAD']);
    const started = performance.now();
    const result = cli(root, ['run', '--milestone', 'M001', '--only', 'P001', '--max-workers', String(maxWorkers)]);
    const wallMs = Math.round(performance.now() - started);
    assert.equal(result.status, 0, result.details);
    assert.equal(result.data.status, 'scope_completed');
    assert.deepEqual(result.data.selected_phases, ['P001']);
    assert.deepEqual(result.data.completed_phases, ['P001']);
    assert.equal(result.data.completed_tasks.length, 6);
    assert.notEqual(git(root, ['rev-parse', 'HEAD']), before);
    assert.equal(git(root, ['rev-parse', 'HEAD']), result.data.accepted_head);
    assert.equal(git(root, ['status', '--porcelain']), '');
    assert.equal(existsSync(join(root, 'src/service.mjs')), false, 'the second phase stays outside the selected scope');
    for (const task of tasks) assert.equal(readFileSync(join(root, task.path), 'utf8'), task.content);

    const source = readFileSync(join(root, 'specs/arithmetic/tasks.md'), 'utf8');
    const checked = [...source.matchAll(/^- \[[xX]\] (T\d+) \[P\]/gm)];
    assert.equal(checked.length, 6, 'all six native parallel tasks are marked complete after integration');
    assert.equal(new Set(checked.map(match => match[1])).size, 6);
    assert.doesNotMatch(source, /^- \[ \]/m);

    const writes = result.data.attempts.filter(attempt => attempt.kind === 'implement');
    assert.equal(writes.length, 6, 'the successful workload needs one attempt per source task');
    assert.ok(writes.every(attempt => attempt.status === 'integrated'));
    assert.equal(new Set(writes.map(attempt => attempt.id)).size, 6);
    assert.equal(new Set(writes.map(attempt => attempt.worktree)).size, 6);
    const peak = concurrency(writes);
    assert.ok(peak <= maxWorkers, `observed ${peak} overlapping attempts for max_workers=${maxWorkers}`);
    if (maxWorkers === 1) assert.equal(peak, 1, 'serial execution must never overlap attempts');
    else assert.ok(peak >= 2, 'parallel execution must actually overlap independent attempts');

    // Prove the host ran real Node assertions, rather than accepting only mock
    // success claims or a nested test runner's empty output. Recheck delivered code.
    assert.ok(result.data.evidence.length >= 12);
    for (const evidence of result.data.evidence) {
      assert.equal(evidence.exit_code, 0);
      assert.equal(evidence.tree_unchanged, true);
      assert.ok(readFileSync(evidence.log).length > 0, 'host test evidence must contain executed test output');
    }
    const delivered = spawnSync(process.execPath, ['--test', ...tasks.map(task => task.test)], {
      cwd: root,
      env: cleanEnv(),
      encoding: 'utf8',
      timeout: 10000,
    });
    assert.equal(delivered.status, 0, delivered.stderr + delivered.stdout);
    assert.ok(delivered.stdout.length > 0);

    const observation = {
      max_workers: maxWorkers,
      task_count: tasks.length,
      mock_sleep_ms: sleepMs,
      wall_ms: wallMs,
      peak_overlapping_attempts: peak,
      status: result.data.status,
      run_id: result.data.id,
      accepted_head: result.data.accepted_head,
      host_verification_count: result.data.evidence.length,
      attempts: writes.map(attempt => ({
        task_id: attempt.task_id,
        started_at: attempt.started_at,
        finished_at: attempt.finished_at,
      })),
    };
    observations.push(observation);
    t.diagnostic(JSON.stringify({ scheduling_observation: observation }));
  }
  t.diagnostic(JSON.stringify({
    scheduling_comparison: {
      recorded_at: new Date().toISOString(),
      platform: `${process.platform}-${process.arch}`,
      node: process.version,
      binary,
      binary_sha256: createHash('sha256').update(readFileSync(binary)).digest('hex'),
      serial_wall_ms: observations[0].wall_ms,
      parallel_wall_ms: observations[1].wall_ms,
      observed_ratio: Number((observations[0].wall_ms / observations[1].wall_ms).toFixed(3)),
      speed_is_not_a_correctness_gate: true,
    },
  }));
});
