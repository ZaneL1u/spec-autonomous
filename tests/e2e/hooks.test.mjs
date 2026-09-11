import test from 'node:test';
import assert from 'node:assert/strict';
import {mkdirSync,writeFileSync,readFileSync} from 'node:fs';
import {fixture,cli,git,configure,join} from './helpers.mjs';

for(const idempotent of [false,true])test(`interrupted ${idempotent?'idempotent':'non-idempotent'} native hook is reconciled without duplicate effects`,{timeout:150000},t=>{
  const root=fixture(t,'speckit');
  mkdirSync(join(root,'hooks'),{recursive:true});
  writeFileSync(join(root,'hooks/count.mjs'),`import {existsSync,readFileSync,writeFileSync} from 'node:fs';\nconst file='hook-count.json';const state=existsSync(file)?JSON.parse(readFileSync(file,'utf8')):{count:0,keys:[]};const key=process.env.SPEC_AUTONOMOUS_HOOK_KEY;if(!${idempotent}||!state.keys.includes(key)){state.count++;state.keys.push(key);writeFileSync(file,JSON.stringify(state));}\n`);
  writeFileSync(join(root,'.specify/extensions.yml'),'hooks:\n  after_implement:\n    - command: mock.count\n      optional: false\n');
  configure(root,`\n[hooks."mock.count"]\nargv = [${JSON.stringify(process.execPath)}, "hooks/count.mjs"]\nidempotent = ${idempotent}\n`);
  const first=cli(root,['run','--milestone','M001','--only','1'],{env:{SPEC_AUTONOMOUS_TEST_FAILPOINT:'after_hook'}});
  assert.equal(first.status,86,first.details);
  const saved=cli(root,['status']).data;
  const statePath=join(saved.integration,'hook-count.json');
  assert.equal(JSON.parse(readFileSync(statePath,'utf8')).count,1);
  let resumed=cli(root,['resume',saved.id]);
  if(!idempotent){
    assert.equal(resumed.status,4,resumed.details);assert.match(resumed.data.blocker,/hook_outcome_unknown/);
    assert.equal(JSON.parse(readFileSync(statePath,'utf8')).count,1);
    const key=Object.keys(resumed.data.hook_results).find(k=>resumed.data.hook_results[k]==='intent');
    const resolved=cli(root,['resolve-hook',saved.id,'--key',key,'--outcome','completed','--evidence','Verified the committed local counter has exactly one effect']);
    assert.equal(resolved.status,0,resolved.details);
    resumed=cli(root,['resume',saved.id]);
  }
  assert.equal(resumed.status,0,resumed.details);assert.equal(resumed.data.status,'scope_completed');
  assert.equal(JSON.parse(readFileSync(join(root,'hook-count.json'),'utf8')).count,1);
  assert.equal(git(root,['status','--porcelain']),'');
});

test('plan mode never executes implementation hooks',{timeout:120000},t=>{
  const root=fixture(t,'speckit');
  mkdirSync(join(root,'hooks'),{recursive:true});
  writeFileSync(join(root,'hooks/marker.mjs'),"import {writeFileSync} from 'node:fs';writeFileSync('implementation-hook-ran.txt','unexpected');\n");
  writeFileSync(join(root,'.specify/extensions.yml'),'hooks:\n  before_implement:\n    - command: mock.marker\n      optional: false\n');
  configure(root,`\n[hooks."mock.marker"]\nargv = [${JSON.stringify(process.execPath)}, "hooks/marker.mjs"]\nidempotent = false\n`);
  const planned=cli(root,['plan','--milestone','M001']);
  assert.equal(planned.status,0,planned.details);assert.equal(planned.data.status,'plan_ready');
  assert.equal(planned.data.attempts.filter(a=>a.kind==='hook'||a.kind==='implement').length,0);
  assert.equal(git(root,['ls-files','implementation-hook-ran.txt']),'');
});
