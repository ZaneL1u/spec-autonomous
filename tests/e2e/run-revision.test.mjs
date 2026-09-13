import test from 'node:test';
import assert from 'node:assert/strict';
import {readFileSync, writeFileSync} from 'node:fs';
import {spawnSync} from 'node:child_process';
import {fixture, edit, git, join, workspace} from './helpers.mjs';
import {raw, drive, cleanEnv} from '../mock-host.mjs';

function execute(root, request, changeResult = () => {}) {
  const owner = {host_id: 'revision-e2e', session_id: request.request_id, fresh_context: true};
  const claim = raw(root, ['claim', request.run_id, request.request_id, '--token', request.token,
    '--host-id', owner.host_id, '--session-id', owner.session_id, '--fresh-context']);
  assert.equal(claim.status, 0, claim.details);
  const worker = spawnSync(process.execPath, [join(workspace, 'tests/mock-agent.mjs')], {
    cwd: request.project,
    env: cleanEnv({SPEC_AUTONOMOUS_INPUT: request.input_path, SPEC_AUTONOMOUS_RESULT: request.result_path}),
    encoding: 'utf8', timeout: 10000,
  });
  assert.equal(worker.status, 0, worker.stderr || String(worker.error));
  const result = JSON.parse(readFileSync(request.result_path, 'utf8'));
  changeResult(result);
  writeFileSync(request.result_path, JSON.stringify(result));
  return raw(root, ['apply-result', '--result', request.result_path, '--token', request.token,
    '--host-id', owner.host_id, '--session-id', owner.session_id, '--fresh-context', '--view', 'full']);
}

test('a failed verification command can be revised and delivered in the same run without redoing accepted work', {timeout: 120000}, async t => {
  const root = fixture(t, 'speckit', {sleepMs: 0});
  edit(root, '.spec-autonomous/config.toml', text => text.replace('max_attempts = 3', 'max_attempts = 1'));
  git(root, ['add', '--all']);
  git(root, ['commit', '-qm', 'test: exhaust one bad check before reviewing its correction']);
  const initialHead = git(root, ['rev-parse', 'HEAD']);
  let current = raw(root, ['prepare', '--milestone', 'M001', '--only', '1', '--view', 'full']);
  assert.equal(current.status, 0, current.details);
  const runId = current.data.id;
  let originalChecks;
  let targetId;
  let acceptedId;
  // A runtime wrapper reproduces the Node directory-argument failure after
  // planning, exercising recovery independently of static argv diagnostics.
  const badCheck = {argv: [process.execPath, '-e',
    'const r=require("node:child_process").spawnSync(process.execPath,["--test","tests/"],{stdio:"inherit"});process.exit(r.status ?? 1)'], cwd: '.'};
  for (let round = 0; round < 16 && current.data?.work?.[0]?.kind !== 'implement'; round++) {
    assert.equal(current.status, 0, current.details);
    assert.equal(current.data.status, 'awaiting_host', current.details);
    const request = current.data.work[0];
    current = execute(root, request, result => {
      if (!result.plan) return;
      assert.equal(result.plan.tasks.length, 2);
      const [first, second] = result.plan.tasks;
      acceptedId = first.id;
      targetId = second.id;
      originalChecks = structuredClone(second.verification);
      second.depends_on = [first.id];
      second.verification = [badCheck];
    });
  }
  assert.ok(originalChecks, 'native planning returned an execution plan');
  assert.equal(current.data.work[0].task_id, acceptedId);
  current = execute(root, current.data.work[0]);
  assert.equal(current.status, 0, current.details);
  assert.ok(current.data.completed_tasks.includes(`P001/${acceptedId}`));
  assert.equal(current.data.work[0].task_id, targetId);
  const failedRequest = current.data.work[0];
  execute(root, failedRequest);
  const stopped = raw(root, ['status', runId]);
  assert.equal(stopped.status, 0, stopped.details);
  assert.ok(['paused', 'needs_input'].includes(stopped.data.status), stopped.details);
  const failedAttempt = stopped.data.attempts.find(a => a.id === failedRequest.request_id);
  assert.equal(failedAttempt.status, 'failed');
  assert.match(failedAttempt.error, /verification_failed/);
  const failure = stopped.data.evidence.find(e => e.exit_code !== 0 && e.argv.includes(badCheck.argv[2]));
  assert.ok(failure, 'the coordinator executed and recorded the failing command');
  assert.match(failedAttempt.error, /MODULE_NOT_FOUND|Cannot find module/);
  const acceptedAttempts = stopped.data.attempts.filter(a => a.kind === 'implement' && a.task_id === acceptedId).map(a => a.id);
  const checkpoint = stopped.data.accepted_head;
  const correction = {run_id: runId, reason: 'Use the existing multiply test file: the native arithmetic requirement and assertions are unchanged.',
    task_checks: [{phase_id: 'P001', task_id: targetId, checks: originalChecks}]};
  const call = args => raw(root, ['tools', 'call', 'run.revise', '--input', JSON.stringify(args), '--view', 'full']);
  const preview = call(correction);
  assert.equal(preview.status, 0, preview.details);
  assert.equal(preview.data.can_apply, true);
  assert.ok(preview.data.retained_verified_tasks.includes(`P001/${acceptedId}`));
  assert.equal(raw(root, ['status', runId]).data.accepted_head, checkpoint);
  const appliedArgs = {...correction, apply: true, plan_hash: preview.data.plan_hash};
  const applied = call(appliedArgs);
  assert.equal(applied.status, 0, applied.details);
  assert.equal(applied.data.applied, true);
  assert.equal(raw(root, ['status', runId]).data.accepted_head, checkpoint, 'revision does not commit code or advance Git');
  assert.equal(git(root, ['rev-parse', 'HEAD']), initialHead);
  const delivered = await drive(root, ['prepare', '--run-id', runId, '--view', 'full'], {maxRounds: 16});
  assert.equal(delivered.status, 0, delivered.details);
  assert.equal(delivered.data.id, runId);
  assert.equal(delivered.data.status, 'scope_completed');
  assert.deepEqual(delivered.data.attempts.filter(a => a.kind === 'implement' && a.task_id === acceptedId).map(a => a.id), acceptedAttempts);
  assert.ok(delivered.data.attempts.some(a => a.id === failedRequest.request_id && a.status === 'failed'));
  assert.ok(delivered.data.attempts.some(a => a.task_id === targetId && a.status === 'integrated'));
  assert.ok(delivered.data.evidence.some(e => e.log === failure.log && e.exit_code !== 0));
  assert.ok(delivered.data.completed_tasks.includes(`P001/${acceptedId}`));
  assert.ok(delivered.data.completed_tasks.includes(`P001/${targetId}`));
  assert.equal(git(root, ['status', '--porcelain']), '');
  assert.equal(readFileSync(join(root, 'src/multiply.mjs'), 'utf8'), 'export const multiply = (a, b) => a * b;\n');
  const replay = call(appliedArgs);
  assert.equal(replay.status, 0, replay.details);
  assert.equal(replay.data.replayed, true, 'the reviewed correction remains replayable after delivery');
});
