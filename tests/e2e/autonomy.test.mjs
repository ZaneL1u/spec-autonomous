import test from 'node:test';
import assert from 'node:assert/strict';
import {existsSync,readFileSync} from 'node:fs';
import {fixture,cli,start,until,git,configure,join} from './helpers.mjs';

test('OpenSpec: native planning, parallel fresh workers, repair and full verified delivery', {timeout:120000}, t=>{
  const root=fixture(t,'openspec',{failOnce:'add',sleepMs:300});
  const before=git(root,['rev-parse','HEAD']);
  const r=cli(root,['run','--milestone','M001']);
  assert.equal(r.status,0,r.details);assert.equal(r.data.status,'completed');
  assert.deepEqual(r.data.completed_phases,['P001','P002']);
  assert.equal(r.data.completed_tasks.length,3);assert.notEqual(git(root,['rev-parse','HEAD']),before);
  assert.equal(git(root,['status','--porcelain']),'');
  assert.ok(readFileSync(join(root,'src/add.mjs'),'utf8').includes('a + b'));
  const attempts=r.data.attempts;
  assert.equal(new Set(attempts.map(a=>a.id)).size,attempts.length);
  assert.ok(attempts.some(a=>a.kind==='implement'&&a.status==='failed'));
  const writes=attempts.filter(a=>a.kind==='implement');
  assert.equal(new Set(writes.map(a=>a.worktree)).size,writes.length);
  assert.ok(writes.some((a,i)=>writes.some((b,j)=>i!==j&&Date.parse(a.started_at)<Date.parse(b.finished_at)&&Date.parse(b.started_at)<Date.parse(a.finished_at))));
  for(const p of ['arithmetic','calculator'])assert.ok(!readFileSync(join(root,`openspec/changes/${p}/tasks.md`),'utf8').includes('- [ ]'));
  assert.ok(r.data.evidence.every(e=>e.revision&&e.log_hash&&existsSync(e.log)));
  const report=cli(root,['report',r.data.id]);assert.equal(report.status,0,report.details);assert.ok(report.data.events.some(e=>e.kind==='task_retry'));
});

for(const framework of ['openspec','speckit'])test(`${framework}: a goal creates native artifacts and roadmap before execution`, {timeout:120000}, t=>{
  const root=fixture(t,framework,{goalOnly:true});
  const r=cli(root,['milestone','new','Build a tested arithmetic service','--id','MGOAL','--mode','autonomous']);
  assert.equal(r.status,0,r.details);assert.equal(r.data.status,'completed');
  assert.ok(existsSync(join(root,'.spec-autonomous/milestones/MGOAL/milestone.toml')));
  assert.ok(existsSync(join(root,'.spec-autonomous/milestones/MGOAL/ROADMAP.md')));
  assert.ok(existsSync(join(root,framework==='speckit'?'specs/arithmetic/spec.md':'openspec/changes/arithmetic/specs/arithmetic/spec.md')));
  assert.ok(existsSync(join(root,framework==='speckit'?'specs/calculator/tasks.md':'openspec/changes/calculator/tasks.md')));
  assert.equal(r.data.attempts[0].kind,'roadmap');
  assert.ok(r.data.attempts.some(a=>a.kind==='native-planning'));
});

test('range preflight refuses incomplete outside dependency without leaving a worktree',t=>{
  const root=fixture(t);const inventory=git(root,['worktree','list','--porcelain']);const refs=git(root,['show-ref']);
  const r=cli(root,['run','--milestone','M001','--from','2','--to','2']);
  assert.notEqual(r.status,0);assert.match(r.details,/prerequisite_outside_range/);
  assert.equal(git(root,['worktree','list','--porcelain']),inventory);assert.equal(git(root,['show-ref']),refs);
});

test('only stops at scope completion and a later bounded run consumes verified prerequisites',{timeout:120000},t=>{
  const root=fixture(t,'speckit');
  const first=cli(root,['run','--milestone','M001','--only','1']);
  assert.equal(first.status,0,first.details);assert.equal(first.data.status,'scope_completed');
  assert.ok(!existsSync(join(root,'src/service.mjs')));assert.ok(!existsSync(join(root,'specs/calculator/tasks.md')));
  const second=cli(root,['run','--milestone','M001','--from','2','--to','2']);
  assert.equal(second.status,0,second.details);assert.equal(second.data.status,'scope_completed');
  assert.ok(existsSync(join(root,'src/service.mjs')));assert.ok(second.data.completed_phases.includes('P001'));
});

for(const point of ['after_candidate_patch','after_source_writeback','before_intent_finalize','after_candidate_commit','after_git_advance','after_origin_advance'])test(`crash recovery reconciles ${point} without duplicating accepted work`,{timeout:150000},t=>{
  const root=fixture(t,'speckit');
  const crashed=cli(root,['run','--milestone','M001','--max-workers','1'],{env:{SPEC_AUTONOMOUS_TEST_FAILPOINT:point}});
  assert.equal(crashed.status,86,crashed.details);
  const status=cli(root,['status']);assert.equal(status.status,0,status.details);
  const result=cli(root,['resume',status.data.id]);
  assert.equal(result.status,0,result.details);assert.equal(result.data.status,'completed');
  assert.equal(new Set(result.data.completed_tasks).size,3);
  const accepted=result.data.intents.filter(i=>i.state==='accepted');
  assert.equal(new Set(accepted.map(i=>i.task_id)).size,accepted.length);
  assert.equal(git(root,['status','--porcelain']),'');
});

test('out-of-scope changes are rejected and never delivered',{timeout:120000},t=>{
  const root=fixture(t,'speckit');const before=git(root,['rev-parse','HEAD']);
  configure(root,'\n[runner.environment]\nMOCK_SCOPE_ESCAPE = "1"\n');
  const configured=git(root,['rev-parse','HEAD']);assert.notEqual(before,configured);
  const r=cli(root,['run','--milestone','M001']);
  assert.equal(r.status,4,r.details);assert.match(r.data.blocker,/scope_violation|no_progress/);
  assert.equal(git(root,['rev-parse','HEAD']),configured);assert.ok(!existsSync(join(root,'outside-scope.txt')));
  assert.equal(r.data.completed_tasks.length,0);
});

test('progress observes concurrent workers from a linked worktree and pause stops them',{timeout:120000},async t=>{
  const root=fixture(t,'speckit',{sleepMs:3000});
  const running=start(root,['run','--milestone','M001']);
  t.after(()=>{if(running.child.exitCode===null)running.child.kill('SIGKILL');});
  const progress=await until(()=>{const r=cli(root,['progress','--all-worktrees']);return r.status===0&&r.data.active_workers>=2?r.data:null;},40000);
  const worker=progress.worktrees.find(w=>w.kind==='managed-worker'&&w.status==='claimed');
  assert.ok(worker);
  const nested=cli(worker.path,['progress','--all-worktrees']);assert.equal(nested.status,0,nested.details);
  assert.equal(nested.data.worktrees.length,progress.worktrees.length);
  const pause=cli(root,['pause',worker.run_id]);assert.equal(pause.status,0,pause.details);
  const ended=await running.finished;assert.equal(ended.status,4,ended.details);assert.match(ended.data.blocker,/paused/);
  const stopped=cli(root,['progress','--all-worktrees']);assert.equal(stopped.data.active_workers,0);
});
