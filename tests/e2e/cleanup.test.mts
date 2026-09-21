import test from 'node:test';
import assert from 'node:assert/strict';
import {existsSync,writeFileSync,readFileSync} from 'node:fs';
import {fixture,cli,git,join} from './helpers.mts';

test('run.cleanup CLI previews terminal resources, rejects changed state and applies a reviewed cleanup',{timeout:120000},t=>{
  const root=fixture(t,'speckit');
  const done=cli(root,['run','--milestone','M001','--only','1']);
  assert.equal(done.status,0,done.details);
  const call=(args: any)=>cli(root,['tools','call','run.cleanup','--input',JSON.stringify({...args,run_id:done.data.id,view:'full'})]);
  const before=git(root,['show-ref']);
  const preview=call({delete_branches:true});
  assert.equal(preview.status,0,preview.details);
  assert.equal(preview.data.applied,false);
  assert.ok(preview.data.worktrees.length>0);
  assert.deepEqual(preview.data.removed,[]);
  assert.equal(git(root,['show-ref']),before);
  assert.ok(preview.data.worktrees.every((w: any)=>existsSync(w.path)));

  const dirty=preview.data.worktrees[0];
  writeFileSync(join(dirty.path,'operator-note.txt'),'retain this local note');
  const stale=call({delete_branches:true,apply:true,plan_hash:preview.data.plan_hash});
  assert.notEqual(stale.status,0,stale.details);
  assert.equal(stale.output.error.code,'source_drift');
  assert.ok(preview.data.worktrees.every((w: any)=>existsSync(w.path)));

  const fresh=call({delete_branches:true});
  assert.equal(fresh.status,0,fresh.details);
  assert.ok(fresh.data.retained_details.some((r: any)=>r.path===dirty.path&&r.reason==='dirty'));
  const applied=call({delete_branches:true,apply:true,plan_hash:fresh.data.plan_hash});
  assert.equal(applied.status,0,applied.details);
  assert.ok(applied.data.removed.length>0);
  assert.ok(applied.data.removed.every((path: any)=>!existsSync(path)));
  assert.equal(readFileSync(join(dirty.path,'operator-note.txt'),'utf8'),'retain this local note');
  assert.ok(existsSync(done.data.integration));
  assert.ok(applied.data.evidence_retained);
  const report=cli(root,['report',done.data.id]);
  assert.equal(report.status,0,report.details);
  assert.ok(report.data.events.length>0);
});
