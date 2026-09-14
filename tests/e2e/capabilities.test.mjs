import test from 'node:test';
import assert from 'node:assert/strict';
import {mkdirSync,writeFileSync,readFileSync,existsSync} from 'node:fs';
import {fixture,cli,git,join} from './helpers.mjs';
import {raw} from '../mock-host.mjs';
const call=(root,name,args={})=>raw(root,['tools','call',name,'--input',JSON.stringify(args)]);
const put=(root,file,body)=>{mkdirSync(join(root,file,'..'),{recursive:true});writeFileSync(join(root,file),body);};
const commit=root=>{git(root,['add','--all']);git(root,['commit','-qm','test: capabilities fixture']);};

test('default catalog is limited while all granular tools have schemas and reject unknown fields',t=>{
 const root=fixture(t,'speckit');const primary=raw(root,['tools','list']);assert.equal(primary.status,0,primary.details);assert.equal(primary.data.capabilities.length,8);
 const all=raw(root,['tools','list','--all','--limit','200']);assert.equal(all.status,0,all.details);const caps=all.data.capabilities;assert.ok(caps.length>=50);assert.equal(new Set(caps.map(c=>c.id)).size,caps.length);
 for(const cap of caps){assert.equal(cap.input_schema.additionalProperties,false);assert.ok(cap.output_schema);}
 const before=git(root,['status','--porcelain']);const bad=call(root,'document.scaffold',{file:'bad.md',kind:'summary',unexpected:true});assert.notEqual(bad.status,0);assert.equal(git(root,['status','--porcelain']),before);assert.ok(!existsSync(join(root,'bad.md')));
});
test('frontmatter CAS preserves CRLF body and unknown metadata; stale patches cannot overwrite',t=>{
 const root=fixture(t,'speckit');const body='\r\n# Notes\r\nKeep this exact body.\r\n';put(root,'docs/note.md','---\r\nstatus: draft\r\ncustom:\r\n  enabled: true\r\n---\r\n'+body);
 const read=call(root,'document.inspect',{file:'docs/note.md'});assert.equal(read.status,0,read.details);
 const patch=call(root,'frontmatter.patch',{file:'docs/note.md',expected_hash:read.data.source_hash,patch:{status:'reviewed'}});assert.equal(patch.status,0,patch.details);
 assert.ok(readFileSync(join(root,'docs/note.md'),'utf8').endsWith(body));const after=call(root,'frontmatter.get',{file:'docs/note.md'});assert.equal(after.data.frontmatter.custom.enabled,true);
 const bytes=readFileSync(join(root,'docs/note.md'));assert.notEqual(call(root,'frontmatter.patch',{file:'docs/note.md',expected_hash:read.data.source_hash,patch:{status:'stale'}}).status,0);assert.deepEqual(readFileSync(join(root,'docs/note.md')),bytes);
 assert.notEqual(call(root,'document.inspect',{file:'../outside.md'}).status,0);assert.notEqual(call(root,'document.inspect',{file:'.git/config'}).status,0);
});
test('roadmap updates validate dependencies and preserve stable labels',t=>{
 const root=fixture(t,'speckit');let read=call(root,'roadmap.get',{milestone_id:'M001'});assert.equal(read.status,0,read.details);
 const phase={id:'P003',label:'3',title:'Documentation',depends_on:['P002'],source:{kind:'speckit-feature',selector:'specs/documentation'},verification:[]};
 const add=call(root,'roadmap.add',{milestone_id:'M001',expected_hash:read.data.source_hash,phase});assert.equal(add.status,0,add.details);assert.equal(add.data.milestone.revision,2);
 const remove=call(root,'roadmap.remove',{milestone_id:'M001',expected_hash:add.data.source_hash,phase_id:'P002'});assert.notEqual(remove.status,0);read=call(root,'roadmap.get',{milestone_id:'M001'});assert.equal(read.data.milestone.phases.length,3);
 assert.notEqual(call(root,'roadmap.select',{milestone_id:'M001',only:'3'}).status,0);
});
test('worktree tools verify ownership, clean state and source/destination revisions',t=>{
 const root=fixture(t,'speckit');const target=git(root,['rev-parse','HEAD']);const made=call(root,'worktree.create',{name:'manual-unit'});assert.equal(made.status,0,made.details);
 put(made.data.path,'notes.txt','reviewable work');git(made.data.path,['add','notes.txt']);git(made.data.path,['commit','-qm','test: workspace change']);
 const head=git(made.data.path,['rev-parse','HEAD']);const diff=call(root,'worktree.diff',{worktree:made.data.path,base:target});assert.equal(diff.status,0,diff.details);assert.ok(diff.data.changed_files.includes('notes.txt'));
 const merge=call(root,'worktree.merge',{worktree_id:made.data.id,expected_head:head,expected_target:target});assert.equal(merge.status,0,merge.details);assert.equal(readFileSync(join(root,'notes.txt'),'utf8'),'reviewable work');
 put(made.data.path,'keep.txt','dirty');assert.notEqual(call(root,'worktree.remove',{worktree_id:made.data.id,expected_head:head}).status,0);assert.ok(existsSync(join(made.data.path,'keep.txt')));
 const external=join(root,'..','external');git(root,['worktree','add','-b','external',external]);assert.notEqual(call(root,'worktree.remove',{worktree_id:external,expected_head:head}).status,0);assert.ok(existsSync(external));
});
test('scoped Git commit leaves unrelated staged changes outside the commit',t=>{
 const root=fixture(t,'speckit');const head=git(root,['rev-parse','HEAD']);put(root,'a.txt','A');put(root,'b.txt','B');git(root,['add','b.txt']);
 const done=call(root,'git.commit',{expected_head:head,message:'test: scoped commit',files:['a.txt']});assert.equal(done.status,0,done.details);assert.equal(git(root,['show','HEAD:a.txt']),'A');assert.equal(git(root,['diff','--cached','--name-only']),'b.txt');
});
for(const framework of ['speckit','openspec'])test(`${framework}: native archive previews are read-only and milestone apply is idempotent`,{timeout:180000},t=>{
 const root=fixture(t,framework);const done=cli(root,['run','--milestone','M001']);assert.equal(done.status,0,done.details);
 const before=git(root,['rev-parse','HEAD']);const preview=raw(root,['archive','--milestone','M001']);assert.equal(preview.status,0,preview.details);assert.equal(preview.data.sources.length,2);assert.equal(git(root,['rev-parse','HEAD']),before);assert.equal(git(root,['status','--porcelain']),'');
 const archived=raw(root,['archive','--milestone','M001','--apply','--plan-hash',preview.data.plan_hash]);assert.equal(archived.status,0,archived.details);assert.equal(archived.data.status,'archived');
 for(const source of preview.data.sources){assert.ok(!existsSync(join(root,source.source)));assert.ok(existsSync(join(root,source.destination)));}
 assert.ok(existsSync(join(root,preview.data.destination,'milestone.toml')));assert.equal(git(root,['status','--porcelain']),'');
 const head=git(root,['rev-parse','HEAD']);const replay=raw(root,['archive','--apply','--plan-hash',preview.data.plan_hash]);assert.equal(replay.status,0,replay.details);assert.equal(replay.data.replayed,true);assert.equal(git(root,['rev-parse','HEAD']),head);
});
test('archive refuses active work and stale preview without changing the source', {timeout:120000},t=>{
 const root=fixture(t,'speckit');const started=raw(root,['prepare','--milestone','M001']);assert.equal(started.status,0,started.details);
 const blocked=raw(root,['archive','--feature','specs/arithmetic']);assert.notEqual(blocked.status,0);
 const req=started.data.work[0];raw(root,['cancel',started.data.id]);assert.equal(call(root,'work.revoke',{run_id:started.data.id,request_id:req.request_id,token:req.token,host_stopped:true,reason:'never dispatched'}).status,0);
 const done=cli(root,['run','--milestone','M001','--only','1']);assert.equal(done.status,0,done.details);
 const preview=raw(root,['archive','--feature','specs/arithmetic']);assert.equal(preview.status,0,preview.details);
 put(root,'extra.txt','new baseline');commit(root);const head=git(root,['rev-parse','HEAD']);const stale=raw(root,['archive','--feature','specs/arithmetic','--apply','--plan-hash',preview.data.plan_hash]);assert.notEqual(stale.status,0);assert.ok(existsSync(join(root,'specs/arithmetic/tasks.md')));assert.equal(git(root,['rev-parse','HEAD']),head);
});

test('doctor and previewed repairs preserve user configuration and back up the original bytes',t=>{
 const root=fixture(t,'speckit');const original=readFileSync(join(root,'.spec-autonomous/config.toml'));
 const doctor=raw(root,['doctor']);assert.equal(doctor.status,0,doctor.details);assert.ok(doctor.data.diagnostics.some(d=>d.includes('legacy_runner_ignored')));assert.deepEqual(readFileSync(join(root,'.spec-autonomous/config.toml')),original);
 const preview=call(root,'repair',{kind:'legacy-config'});assert.equal(preview.status,0,preview.details);
 const stale=call(root,'repair',{kind:'legacy-config',apply:true,expected_hash:'not-current'});assert.notEqual(stale.status,0);assert.deepEqual(readFileSync(join(root,'.spec-autonomous/config.toml')),original);
 const fixed=call(root,'repair',{kind:'legacy-config',apply:true,expected_hash:preview.data.expected_hash});assert.equal(fixed.status,0,fixed.details);assert.ok(!readFileSync(join(root,'.spec-autonomous/config.toml'),'utf8').includes('[runner]'));
 const backup=join(root,'.git/spec-autonomous/backups',`config-${preview.data.expected_hash}.toml`);assert.deepEqual(readFileSync(backup),original);
});
