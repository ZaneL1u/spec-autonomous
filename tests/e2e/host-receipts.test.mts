import test from 'node:test';
import assert from 'node:assert/strict';
import {readFileSync,existsSync} from 'node:fs';
import {spawnSync} from 'node:child_process';
import {fixture,cli,git,join,workspace} from './helpers.mts';
import {raw,cleanEnv} from '../mock-host.mts';
for(const point of ['before_receipt_file','after_receipt_file'])test(`receipt restart reconciles ${point} without another semantic worker`,{timeout:30000},t=>{
 const root=fixture(t,'speckit');const first=raw(root,['prepare','--milestone','M001']);assert.equal(first.status,0,first.details);const request=first.data.work[0];
 const owner={host_id:'receipt-test',session_id:request.request_id,fresh_context:true};
 const worker=spawnSync(process.execPath,[join(workspace,'tests/mock-agent.mts')],{cwd:request.project,env:cleanEnv({SPEC_AUTONOMOUS_INPUT:request.input_path,SPEC_AUTONOMOUS_RESULT:request.result_path}),encoding:'utf8'});assert.equal(worker.status,0,worker.stderr);
 const argv=['apply-result','--result',request.result_path,'--token',request.token,'--host-id',owner.host_id,'--session-id',owner.session_id,'--fresh-context'];
 const failed=raw(root,argv,{SPEC_AUTONOMOUS_TEST_FAILPOINT:point});assert.equal(failed.status,86,failed.details);
 const saved=raw(root,['status',first.data.id]).data;assert.equal(saved.attempts.find((a: any)=>a.id===request.request_id).status,'receiving');
 const recovered=raw(root,argv);assert.equal(recovered.status,0,recovered.details);assert.equal(recovered.data.receipt_replayed,true);assert.equal(recovered.data.status,'awaiting_host');
 const status=raw(root,['status',first.data.id]).data;assert.equal(status.attempts.filter((a: any)=>a.task_id===request.task_id).length,1);assert.equal(status.attempts.find((a: any)=>a.id===request.request_id).status,'accepted');
});
for(const point of ['after_archive_candidate','after_archive_advance'])test(`archive restart reconciles ${point} without duplicate source moves`,{timeout:120000},t=>{
 const root=fixture(t,'speckit');const done=cli(root,['run','--milestone','M001','--only','1']);assert.equal(done.status,0,done.details);
 const before=raw(root,['archive','--feature','specs/arithmetic']);assert.equal(before.status,0,before.details);
 const args=['archive','--feature','specs/arithmetic','--apply','--plan-hash',before.data.plan_hash];
 const failed=raw(root,args,{SPEC_AUTONOMOUS_TEST_FAILPOINT:point});assert.equal(failed.status,86,failed.details);
 const recovered=raw(root,args);assert.equal(recovered.status,0,recovered.details);assert.equal(recovered.data.status,'archived');assert.ok(!existsSync(join(root,'specs/arithmetic')));assert.ok(existsSync(join(root,before.data.destination,'tasks.md')));assert.equal(git(root,['status','--porcelain']),'');
 const head=git(root,['rev-parse','HEAD']);assert.equal(raw(root,args).status,0);assert.equal(git(root,['rev-parse','HEAD']),head);
});
