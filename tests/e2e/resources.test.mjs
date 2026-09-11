import test from 'node:test';
import assert from 'node:assert/strict';
import {readFileSync,writeFileSync,mkdirSync,existsSync} from 'node:fs';
import {createHash} from 'node:crypto';
import {fixture,cli,edit,git,join,start,until} from './helpers.mjs';

test('large native context is referenced intact while worker packets remain bounded',{timeout:120000},t=>{
  const root=fixture(t,'speckit');const first=cli(root,['plan','--milestone','M001']);assert.equal(first.status,0,first.details);
  edit(root,'specs/arithmetic/spec.md',text=>text+'\n<!-- '+ 'retained native context '.repeat(12000)+' critical-end-rule:never_modify_tests -->\n');
  edit(root,'.spec-autonomous/config.toml',text=>text.replace('max_workers = 2','max_context_bytes = 16384\nmax_workers = 2'));
  git(root,['add','--all']);git(root,['commit','-qm','test: large native specification']);
  const planned=cli(root,['plan','--milestone','M001']);assert.equal(planned.status,0,planned.details);
  const attempt=planned.data.attempts.find(a=>a.kind==='plan-tasks');
  const directory=join(root,'.git/spec-autonomous/runs',planned.data.id,'attempts',attempt.id);
  const input=readFileSync(join(directory,'input.json'));const prompt=readFileSync(join(directory,'prompt.md'));
  assert.ok(input.length<=16384);assert.ok(prompt.length<=16384);
  const ref=JSON.parse(input).snapshot.metadata.context_reference;
  const full=readFileSync(ref.path);assert.ok(full.length>16384);
  assert.equal(createHash('sha256').update(full).digest('hex'),ref.sha256);
  assert.ok(full.toString().includes('critical-end-rule:never_modify_tests'));
  const progress=cli(root,['progress']);const run=progress.data.runs.find(r=>r.run_id===planned.data.id);
  assert.equal(run.native_progress.find(p=>p.phase_id==='P001').native_total,2);
});

test('large task lists use fresh bounded planner batches with complete global coverage',{timeout:120000},t=>{
  const root=fixture(t,'speckit');
  const path=join(root,'.mock/scenario.json');const scenario=JSON.parse(readFileSync(path,'utf8'));
  scenario.phases=scenario.phases.slice(0,1);
  scenario.phases[0].tasks=Array.from({length:40},(_,i)=>({name:`item${i}`,path:`src/item${i}.mjs`,test:`checks/item${i}.mjs`,content:`export const value=${i};\n`,broken:'export const value=-1;\n'}));
  writeFileSync(path,JSON.stringify(scenario));
  for(const item of scenario.phases[0].tasks)writeFileSync(join(root,item.test),`import assert from 'node:assert/strict'; import {value} from '../${item.path}'; assert.equal(value,${item.name.slice(4)});\n`);
  edit(root,'.spec-autonomous/config.toml',text=>text.replace('max_workers = 2','max_planner_tasks = 8\nmax_workers = 2'));
  edit(root,'.spec-autonomous/milestones/M001/milestone.toml',text=>text.slice(0,text.lastIndexOf('[[phases]]')));
  git(root,['add','--all']);git(root,['commit','-qm','test: forty native tasks']);
  const planned=cli(root,['plan','--milestone','M001']);assert.equal(planned.status,0,planned.details);
  const plan=planned.data.plans.P001;assert.equal(plan.tasks.length,40);
  assert.equal(new Set(plan.tasks.map(t=>t.id)).size,40);
  assert.equal(new Set(plan.tasks.flatMap(t=>t.source_ids)).size,40);
  const calls=planned.data.attempts.filter(a=>a.kind==='plan-tasks');assert.equal(calls.length,5);
  for(const a of calls){const input=JSON.parse(readFileSync(join(root,'.git/spec-autonomous/runs',planned.data.id,'attempts',a.id,'input.json')));assert.ok(input.snapshot.tasks.length<=8);}
  assert.ok(plan.tasks.slice(8).every(t=>t.depends_on.length>0));
  const progress=cli(root,['progress']);const run=progress.data.runs.find(r=>r.run_id===planned.data.id);
  assert.equal(run.native_progress.find(p=>p.phase_id==='P001').native_total,40);
  assert.equal(run.native_progress.find(p=>p.phase_id==='P001').native_checked,0);
});

test('explicit cleanup preserves dirty worktrees, integration, external worktrees and branch refs',{timeout:120000},t=>{
  const root=fixture(t,'speckit');const done=cli(root,['run','--milestone','M001','--only','1']);assert.equal(done.status,0,done.details);
  const dirty=done.data.attempts.find(a=>a.kind==='implement');writeFileSync(join(dirty.worktree,'keep-uncommitted.txt'),'keep me');
  const external=join(root,'..','external');git(root,['worktree','add','-b','user-work',external,'HEAD']);
  const refs=git(root,['show-ref']);
  const cleaned=cli(root,['cleanup',done.data.id]);assert.equal(cleaned.status,0,cleaned.details);
  assert.ok(cleaned.data.removed.length>0);assert.ok(cleaned.data.retained.includes(dirty.worktree));
  assert.ok(existsSync(join(dirty.worktree,'keep-uncommitted.txt')));assert.ok(existsSync(done.data.integration));assert.ok(existsSync(external));
  assert.equal(git(root,['show-ref']),refs);assert.ok(cleaned.data.branch_refs_retained);assert.ok(cleaned.data.evidence_retained);
});

test('native Spec Kit planning integrates sidecar artifacts and preserves the original feature pointer',{timeout:120000},t=>{
  const root=fixture(t,'speckit');
  const pointer='{\n  "feature_directory":"specs/user-active", "custom":"preserve"\n}\n';
  writeFileSync(join(root,'.specify/feature.json'),pointer);
  edit(root,'.spec-autonomous/config.toml',text=>text+'\n[runner.environment]\nMOCK_NATIVE_SIDECARS="1"\n');
  git(root,['add','--all']);git(root,['commit','-qm','test: native planning sidecars']);
  const done=cli(root,['run','--milestone','M001','--only','1']);assert.equal(done.status,0,done.details);
  assert.equal(readFileSync(join(root,'.specify/feature.json'),'utf8'),pointer);
  for(const file of ['spec.md','plan.md','tasks.md','research.md','data-model.md','quickstart.md','contracts/api.md','checklists/requirements.md'])assert.ok(existsSync(join(root,'specs/arithmetic',file)),file);
});


test('a decomposed native parent is checked only after every child is verified and integrated',{timeout:120000},async t=>{
  const root=fixture(t,'speckit');
  edit(root,'.spec-autonomous/config.toml',text=>text+'\n[runner.environment]\nMOCK_SPLIT_SOURCE="1"\nMOCK_SLOW_TASK="multiply"\n');
  git(root,['add','--all']);git(root,['commit','-qm','test: split a native source task']);
  const running=start(root,['run','--milestone','M001','--only','1']);
  t.after(()=>{if(running.child.exitCode===null)running.child.kill('SIGKILL');});
  const partial=await until(()=>{const r=cli(root,['status']);return r.status===0&&r.data.completed_tasks.length===1?r.data:null;},30000);
  const source=join(partial.integration,'specs/arithmetic/tasks.md');
  assert.ok(readFileSync(source,'utf8').includes('- [ ] T001'));
  assert.equal(partial.completed_phases.length,0);
  const done=await running.finished;assert.equal(done.status,0,done.details);
  assert.equal(done.data.completed_tasks.length,2);
  const text=readFileSync(join(root,'specs/arithmetic/tasks.md'),'utf8');
  assert.equal((text.match(/- \[x\]/g)??[]).length,1);assert.ok(!text.includes('- [ ]'));
});


test('audits receive immutable execution provenance bound to the actual accepted revision',{timeout:120000},t=>{
  const root=fixture(t,'speckit');
  edit(root,'.spec-autonomous/config.toml',text=>text+'\n[runner.environment]\nMOCK_REQUIRE_PROVENANCE="1"\nPRIVATE_CONFIG_VALUE="must-not-enter-evidence"\n');
  git(root,['add','--all']);git(root,['commit','-qm','test: require execution provenance']);
  const done=cli(root,['run','--milestone','M001']);assert.equal(done.status,0,done.details);
  for(const attempt of done.data.attempts.filter(a=>a.kind==='audit')){
    const dir=join(root,'.git/spec-autonomous/runs',done.data.id,'attempts',attempt.id);
    const input=JSON.parse(readFileSync(join(dir,'input.json')));const ref=input.snapshot.metadata.execution_evidence;
    const bytes=readFileSync(ref.path);assert.equal(createHash('sha256').update(bytes).digest('hex'),ref.sha256);
    const evidence=JSON.parse(bytes);assert.equal(evidence.accepted_head,attempt.base_commit);
    assert.equal(evidence.runner_profile,'command');assert.equal(evidence.native_session_ids_observed,0);
    assert.ok(!bytes.toString().includes('must-not-enter-evidence'));
    assert.equal(new Set(evidence.attempts.map(a=>a.attempt_id)).size,evidence.attempts.length);
  }
});
