import test from 'node:test';
import assert from 'node:assert/strict';
import {spawn,spawnSync} from 'node:child_process';
import {writeFileSync,readFileSync,existsSync} from 'node:fs';
import {fixture,git,join,until} from './helpers.mts';
import {raw,binary,cleanEnv} from '../mock-host.mts';
const alive=(pid: any)=>{const r=spawnSync('ps',['-p',String(pid),'-o','stat='],{encoding:'utf8'});return r.status===0&&r.stdout.trim()&&!r.stdout.trim().startsWith('Z');};
test('SIGTERM stops a CLI-owned verification process tree without merging its candidate',{timeout:30000,skip:process.platform==='win32'},async t=>{
 const root=fixture(t,'speckit');const marker=join(root,'..','verification-process.json');
 writeFileSync(join(root,'.mock/hang-check.mjs'),`import {writeFileSync} from 'node:fs';import {spawn} from 'node:child_process';const child=spawn(process.execPath,['-e','setInterval(()=>{},1000)'],{stdio:'ignore'});writeFileSync(${JSON.stringify(marker)},JSON.stringify({pid:process.pid,child:child.pid}));setInterval(()=>{},1000);`);
 const config=join(root,'.spec-autonomous/config.toml');writeFileSync(config,readFileSync(config,'utf8').replace('schema_version = 1',`schema_version = 2\nverification=[{argv=${JSON.stringify([process.execPath,'.mock/hang-check.mjs'])},cwd="."}]`));git(root,['add','--all']);git(root,['commit','-qm','test: cancellable verification']);
 const before=git(root,['rev-parse','HEAD']);const created=raw(root,['tools','call','worktree.create','--input','{"name":"signal-test"}']);assert.equal(created.status,0,created.details);
 writeFileSync(join(created.data.path,'change.txt'),'candidate');git(created.data.path,['add','change.txt']);git(created.data.path,['commit','-qm','test: candidate']);
 const input={worktree_id:created.data.id,expected_head:git(created.data.path,['rev-parse','HEAD']),expected_target:before};
 const cli=spawn(binary,['--path',root,'tools','call','worktree.merge','--input',JSON.stringify(input),'--json'],{env:cleanEnv(),stdio:['ignore','pipe','pipe']});
 let stdout='',stderr='';cli.stdout.on('data',b=>stdout+=b);cli.stderr.on('data',b=>stderr+=b);let children:any;
 t.after(()=>{if(cli.exitCode===null)cli.kill('SIGKILL');if(children){try{process.kill(-children.pid,'SIGKILL')}catch{}}});
 await until(()=>existsSync(marker),10000);children=JSON.parse(readFileSync(marker,'utf8'));assert.ok(alive(children.pid));
 cli.kill('SIGTERM');await until(()=>cli.exitCode!==null,8000);assert.equal(cli.exitCode,130,stdout+stderr);
 await until(()=>!alive(children.pid)&&!alive(children.child),3000);assert.equal(git(root,['rev-parse','HEAD']),before);
});
test('idle MCP exits on SIGTERM even when the input pipe remains open',{timeout:10000,skip:process.platform==='win32'},async t=>{
 const root=fixture(t,'speckit');const child=spawn(binary,['--path',root,'mcp'],{env:cleanEnv(),stdio:['pipe','pipe','pipe']});t.after(()=>{if(child.exitCode===null)child.kill('SIGKILL')});
 let output='';child.stdout.on('data',b=>output+=b);
 child.stdin.write(JSON.stringify({jsonrpc:'2.0',id:1,method:'initialize',params:{protocolVersion:'2025-11-25',capabilities:{},clientInfo:{name:'signal-test',version:'1'}}})+'\n');
 await until(()=>output.includes('protocolVersion'),5000);child.kill('SIGTERM');await until(()=>child.exitCode!==null,5000);assert.equal(child.exitCode,130);
});
