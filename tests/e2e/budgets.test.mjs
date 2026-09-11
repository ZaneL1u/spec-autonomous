import test from 'node:test';
import assert from 'node:assert/strict';
import {fixture,cli,edit,git} from './helpers.mjs';

test('no-progress stops identical failures while independent work completes and resume retains the stop',{timeout:120000},t=>{
  const root=fixture(t,'speckit');
  edit(root,'.spec-autonomous/config.toml',text=>text.replace('max_attempts = 3','max_attempts = 8')+'\n[runner.environment]\nMOCK_FAIL_ALWAYS="add"\n');
  git(root,['add','--all']);git(root,['commit','-qm','test: persistent identical worker failure']);
  const first=cli(root,['run','--milestone','M001','--only','1']);
  assert.notEqual(first.status,0,first.details);assert.match(first.data.blocker,/no_progress/);
  assert.equal(first.data.attempts.filter(a=>a.task_id==='work-add'&&a.kind==='implement').length,2);
  assert.ok(first.data.completed_tasks.some(k=>k.includes('work-multiply')));
  const resumed=cli(root,['resume',first.data.id,'--extend-seconds','60']);
  assert.notEqual(resumed.status,0,resumed.details);
  assert.equal(resumed.data.attempts.filter(a=>a.task_id==='work-add'&&a.kind==='implement').length,2);
  assert.equal(resumed.data.completed_phases.length,0);
});
