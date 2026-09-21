import test from 'node:test';
import assert from 'node:assert/strict';
import {readFileSync,existsSync} from 'node:fs';
import {fixture,cli,start,until,git,edit,join} from './helpers.mts';

for(const scope of ['phase','milestone'])test(`${scope} verification failures enter bounded source repair and revalidation`,{timeout:150000},t=>{
  const root=fixture(t,'speckit',{phaseRepair:scope==='phase',milestoneRepair:scope==='milestone'});
  const r=cli(root,['run','--milestone','M001']);
  assert.equal(r.status,0,r.details);assert.equal(r.data.status,'completed');
  assert.equal(r.data.repair_rounds,1);assert.ok(existsSync(join(root,'src/version.mjs')));
  const checks=r.data.evidence.filter((e: any)=>e.argv.includes('checks/version.mjs'));
  assert.ok(checks.some((e: any)=>e.exit_code!==0),'a real missing implementation failed verification');
  assert.ok(checks.some((e: any)=>e.exit_code===0&&e.tree_unchanged),'the repaired implementation passed');
  assert.ok(checks.some((e: any)=>readFileSync(e.log,'utf8').includes('tests')),'Node assertions actually executed');
  assert.ok(r.data.attempts.some((a: any)=>a.kind==='converge'&&a.status==='accepted'));
  assert.equal(git(root,['status','--porcelain']),'');
});

test('a supplied plan cannot bypass dependency validation or create a worktree on rejection',{timeout:120000},t=>{
  const root=fixture(t,'speckit');
  const planned=cli(root,['plan','--milestone','M001']);assert.equal(planned.status,0,planned.details);
  const file='.spec-autonomous/plans/M001-P001.toml';
  edit(root,file,text=>text.replace('depends_on = []','depends_on = ["missing-dependency"]'));
  git(root,['add','--all']);git(root,['commit','-qm','test: invalid external plan']);
  const before=git(root,['worktree','list','--porcelain']);
  const r=cli(root,['run','--plan',join(root,file)]);
  assert.notEqual(r.status,0);assert.match(r.details,/invalid_plan/);
  assert.equal(git(root,['worktree','list','--porcelain']),before);
});

test('source drift can be committed and reconciled on resume without losing native edits',{timeout:180000},async t=>{
  const root=fixture(t,'speckit',{sleepMs:1200});
  const planned=cli(root,['plan','--milestone','M001']);assert.equal(planned.status,0,planned.details);
  const running=start(root,['run','--milestone','M001']);
  t.after(()=>{if(running.child.exitCode===null)running.child.kill('SIGKILL');});
  await until(()=>{const p=cli(root,['progress']);return p.data?.active_workers>=2;},40000);
  edit(root,'specs/arithmetic/spec.md',text=>text+'\nNative edit: preserve the established arithmetic contracts.\n');
  git(root,['add','--all']);git(root,['commit','-qm','test: user refines native spec']);
  const stopped=await running.finished;assert.equal(stopped.status,4,stopped.details);assert.match(stopped.data.blocker,/source_drift/);
  const result=cli(root,['resume',stopped.data.id,'--extend-seconds','60']);
  assert.equal(result.status,0,result.details);assert.equal(result.data.status,'completed');
  assert.ok(readFileSync(join(root,'specs/arithmetic/spec.md'),'utf8').includes('Native edit: preserve'));
  assert.equal(git(root,['status','--porcelain']),'');
  const report=cli(root,['report',result.data.id]);assert.ok(report.data.events.some((e: any)=>e.kind==='source_reconciled'));
});
