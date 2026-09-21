import test from 'node:test';
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';
import { join } from 'node:path';
const root = fileURLToPath(new URL('../..', import.meta.url));
test('checked-in OpenSpec playground dogfoods init, auto receipts, progress and cleanup preview', { timeout: 120000 }, () => {
  const result = spawnSync(process.execPath, [join(root, 'scripts/dogfood.mts')], { cwd: root, encoding: 'utf8', timeout: 120000, maxBuffer: 16 * 1024 * 1024, env: { ...process.env, OPENSPEC_TELEMETRY: '0' } });
  assert.equal(result.status, 0, result.stderr + result.stdout);
  const report = JSON.parse(result.stdout);
  assert.equal(report.status, 'completed');
  assert.equal(report.initialized, true);
  assert.equal(report.framework, 'openspec');
  assert.ok(report.worktrees >= 1);
  assert.match(report.cleanup_plan_hash, /^[a-f0-9]{64}$/);
  assert.equal(report.native_change, true); assert.equal(report.discussion_cards, 0); assert.equal(report.discussion_applied, 0);
});
