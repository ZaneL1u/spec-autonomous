import test from 'node:test';
import assert from 'node:assert/strict';
import {spawn,spawnSync} from 'node:child_process';
import {writeFileSync,readFileSync,existsSync,mkdirSync} from 'node:fs';
import {join} from 'node:path';
import {fixture,git,workspace} from './helpers.mjs';
import {raw,binary,cleanEnv} from '../mock-host.mjs';
function connect(t,root){
 const child=spawn(binary,['--path',root,'mcp'],{env:cleanEnv(),stdio:['pipe','pipe','pipe']});let sequence=0,buffer='',stderr='';const pending=new Map();
 child.stderr.on('data',b=>stderr+=b);child.stdout.on('data',b=>{buffer+=b;let newline;while((newline=buffer.indexOf('\n'))>=0){const line=buffer.slice(0,newline);buffer=buffer.slice(newline+1);const response=JSON.parse(line);const promise=pending.get(response.id);if(promise){pending.delete(response.id);promise.resolve(response);}}});
 child.on('exit',code=>{for(const p of pending.values())p.reject(Error(`MCP closed ${code}: ${stderr}`));pending.clear();});
 t.after(()=>{if(child.exitCode===null)child.kill('SIGKILL');});
 const request=(method,params={})=>new Promise((resolve,reject)=>{const id=++sequence;pending.set(id,{resolve,reject});child.stdin.write(JSON.stringify({jsonrpc:'2.0',id,method,params})+'\n');});
 const notify=(method,params={})=>child.stdin.write(JSON.stringify({jsonrpc:'2.0',method,params})+'\n');
 return {child,request,notify,call:async(name,arguments_={})=>{const r=await request('tools/call',{name,arguments:arguments_});if(r.error)throw Error(JSON.stringify(r.error));return r.result;},init:async()=>{const r=await request('initialize',{protocolVersion:'2025-11-25',capabilities:{},clientInfo:{name:'test-host',version:'1'}});assert.equal(r.result.protocolVersion,'2025-11-25');notify('notifications/initialized');return r.result;},close:async()=>{child.stdin.end();await new Promise(resolve=>child.once('exit',resolve));assert.equal(child.exitCode,0,stderr);}};
}
test('MCP exposes eight complete tools and one granular dispatcher, with explicit protocol errors',{timeout:30000},async t=>{
 const root=fixture(t,'speckit');const client=connect(t,root);
 assert.equal((await client.request('tools/list')).error.code,-32002);const init=await client.init();assert.ok(!init.capabilities.sampling);
 const list=await client.request('tools/list');assert.equal(list.result.tools.length,9);assert.ok(list.result.tools.some(t=>t.name==='sa_tools'));assert.ok(!list.result.tools.some(t=>t.name==='sa_frontmatter_patch'));
 const catalog=await client.call('sa_tools',{operation:'list'});assert.ok(catalog.structuredContent.data.capabilities.length>=50);
 const before=git(root,['status','--porcelain']);const progress=await client.call('sa_progress');assert.equal(progress.isError,false);const direct=raw(root,['progress']);assert.equal(progress.structuredContent.data.repository_id,direct.data.repository_id);assert.equal(git(root,['status','--porcelain']),before);
 assert.equal((await client.call('sa_progress',{typo:true})).isError,true);assert.equal((await client.request('tools/call',{name:'unknown_tool'})).error.code,-32602);
 assert.equal((await client.request('resources/read',{uri:'spec-autonomous://runs/../../outside'})).error.code,-32602);
 const resource=await client.request('resources/read',{uri:'spec-autonomous://capabilities'});assert.ok(JSON.parse(resource.result.contents[0].text).capabilities.length>=50);
 await client.close();
});
test('MCP prepares work without starting a trap runner and accepts an externally produced receipt',{timeout:45000},async t=>{
 const root=fixture(t,'speckit');const config=join(root,'.spec-autonomous/config.toml');const marker=join(root,'agent-was-started');const trap=join(root,'.mock/trap.mjs');writeFileSync(trap,`import {writeFileSync} from 'node:fs';writeFileSync(${JSON.stringify(marker)},'bad');`);writeFileSync(config,readFileSync(config,'utf8').replace(/^command = .*$/m,`command = ${JSON.stringify([process.execPath,trap])}`));git(root,['add','--all']);git(root,['commit','-qm','test: trap legacy agent launch']);
 const client=connect(t,root);await client.init();const prepared=await client.call('sa_prepare',{milestone_id:'M001'});assert.equal(prepared.isError,false,prepared.content?.[0]?.text);const run=prepared.structuredContent.data;assert.equal(run.starts_agents,false);assert.ok(!existsSync(marker));
 const request=run.work[0],host={host_id:'mcp-test-host',session_id:request.request_id,fresh_context:true};
 const context=await client.request('resources/read',{uri:`spec-autonomous://work/${run.id}/${request.request_id}`});assert.equal(JSON.parse(context.result.contents[0].text).attempt_id,request.request_id);
 const claimed=await client.call('sa_tools',{operation:'call',capability:'work.claim',arguments:{run_id:run.id,request_id:request.request_id,token:request.token,host}});assert.equal(claimed.isError,false);
 const worker=spawnSync(process.execPath,[join(workspace,'tests/mock-agent.mjs')],{cwd:request.project,env:cleanEnv({SPEC_AUTONOMOUS_INPUT:request.input_path,SPEC_AUTONOMOUS_RESULT:request.result_path}),encoding:'utf8'});assert.equal(worker.status,0,worker.stderr);
 const result=JSON.parse(readFileSync(request.result_path));const applied=await client.call('sa_apply_result',{token:request.token,host,result});assert.equal(applied.isError,false,applied.content?.[0]?.text);assert.equal(applied.structuredContent.data.status,'awaiting_host');assert.ok(!existsSync(marker));
 const repeated=await client.call('sa_apply_result',{token:request.token,host,result});assert.equal(repeated.structuredContent.data.receipt_replayed,true);
 await client.close();
});
for(const agent of ['codex','claude'])test(`${agent}: opt-in MCP binding preserves other servers and uninstall removes only owned entries`,t=>{
 const root=fixture(t,'speckit');let path,original;
 if(agent==='codex'){path='.codex/config.toml';original='# User settings\nmodel = "user-choice"\n\n[mcp_servers.other]\ncommand="cat"\n';}else{path='.mcp.json';original=JSON.stringify({custom:true,mcpServers:{other:{command:'cat'}}});}
 mkdirSync(join(root,path,'..'),{recursive:true});writeFileSync(join(root,path),original);
 const installed=raw(root,['init','--agent',agent,'--mcp']);assert.equal(installed.status,0,installed.details);const body=readFileSync(join(root,path),'utf8');assert.ok(body.includes('spec_autonomous'));if(agent==='codex')assert.ok(body.startsWith(original));else assert.deepEqual(JSON.parse(body).mcpServers.other,{command:'cat'});
 const uninstalled=raw(root,['skills','uninstall']);assert.equal(uninstalled.status,0,uninstalled.details);const after=readFileSync(join(root,path),'utf8');assert.ok(!after.includes('spec_autonomous'));assert.ok(after.includes('other'));
});

test('an existing foreign MCP entry is preflighted before any skill or config writes',t=>{
 const root=fixture(t,'speckit');const path=join(root,'.mcp.json');const original=JSON.stringify({mcpServers:{spec_autonomous:{command:'foreign-tool'}}});writeFileSync(path,original);
 const config=readFileSync(join(root,'.spec-autonomous/config.toml'));const installed=raw(root,['init','--agent','claude','--mcp']);assert.notEqual(installed.status,0);assert.match(installed.details,/mcp_conflict/);
 assert.equal(readFileSync(path,'utf8'),original);assert.ok(!existsSync(join(root,'.claude/commands/autonomous.md')));assert.deepEqual(readFileSync(join(root,'.spec-autonomous/config.toml')),config);
});
