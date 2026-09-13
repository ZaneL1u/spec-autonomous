import test from 'node:test';
import assert from 'node:assert/strict';
import { spawnSync, spawn } from 'node:child_process';
import { mkdtempSync, mkdirSync, writeFileSync, readFileSync, readdirSync, rmSync, existsSync, chmodSync } from 'node:fs';
import { join, delimiter } from 'node:path';
import { tmpdir } from 'node:os';
import { binary, workspace, cleanEnv } from './helpers.mjs';
const runtime=process.env.SPEC_AUTONOMOUS_JS_RUNTIME || process.execPath;
const launcher=process.env.SPEC_AUTONOMOUS_TEST_LAUNCHER || join(workspace,'packages/cli/bin/spec-autonomous.mjs');
function fixture(t){
 const base=mkdtempSync(join(tmpdir(),'sa empty init ')),root=join(base,'project'),tools=join(base,'tools'),marker=join(base,'called');mkdirSync(root);mkdirSync(tools);
 t.after(()=>rmSync(base,{recursive:true,force:true}));
 // OpenSpec is real; Spec Kit's small offline fixture checks bootstrap routing.
 const scripts={openspec:`import {spawnSync} from 'node:child_process';const r=spawnSync(${JSON.stringify(process.execPath)},[${JSON.stringify(join(workspace,'node_modules/@fission-ai/openspec/bin/openspec.js'))},...process.argv.slice(2)],{stdio:'inherit'});process.exit(r.status??1);`,
 specify:`import fs from 'node:fs';if(process.argv[2]==='version'){console.log('1.0.6');process.exit(0);}if(process.argv[2]!=='init'||!process.argv.includes('--non-interactive'))process.exit(4);fs.appendFileSync(${JSON.stringify(marker)},'init\\n');fs.mkdirSync('.specify/memory',{recursive:true});fs.writeFileSync('.specify/memory/constitution.md','# Constitution\\n');`};
 for(const [name,body] of Object.entries(scripts)){const file=join(tools,name);writeFileSync(file,`#!${process.execPath}\n${body}`);chmodSync(file,0o755);}
 const env=cleanEnv({PATH:`${tools}${delimiter}${process.env.PATH}`,SPEC_AUTONOMOUS_BINARY:binary,SPEC_AUTONOMOUS_PROVIDER_HOME:join(base,'providers'),SPEC_AUTONOMOUS_LANG:'zh-CN',SPEC_AUTONOMOUS_OFFLINE:'1'});delete env.CI;
 const cli=args=>spawnSync(runtime,[launcher,'--path',root,...args],{env,encoding:'utf8',timeout:30000});
 return{base,root,tools,env,cli,marker};
}
for(const provider of ['openspec','speckit'])test(`empty ${provider} init creates Git and complete project structure; repeat preserves config`,{skip:process.platform==='win32'},t=>{
 const f=fixture(t);const result=f.cli(['init','--provider',provider,'--agent','codex','--non-interactive','--json']);assert.equal(result.status,0,result.stderr+result.stdout);
 const data=JSON.parse(result.stdout).data;assert.equal(data.framework,provider);assert.equal(data.agent,'codex');
 for(const path of ['.git','.spec-autonomous/config.toml','.spec-autonomous/skills-installed.toml','.spec-autonomous/milestones','.spec-autonomous/plans','.spec-autonomous/archives','.agents/skills/autonomous/SKILL.md',provider==='openspec'?'openspec/config.yaml':'.specify/memory/constitution.md'])assert.ok(existsSync(join(f.root,path)),path);
 const config=join(f.root,'.spec-autonomous/config.toml');writeFileSync(config,readFileSync(config,'utf8')+'\n# user configuration\n');const before=readFileSync(config);const second=f.cli(['init','--non-interactive','--json']);assert.equal(second.status,0,second.stderr+second.stdout);assert.deepEqual(readFileSync(config),before);
 assert.notEqual(spawnSync('git',['rev-parse','HEAD'],{cwd:f.root}).status,0,'init must not create a commit');
});
test('conflicts and missing noninteractive options leave an empty/new project unchanged',{skip:process.platform==='win32'},t=>{
 const f=fixture(t);
 const missing=f.cli(['init','--non-interactive','--json']);assert.equal(JSON.parse(missing.stdout).error.code,'provider_selection_required');assert.deepEqual(readdirSync(f.root),[]);
 mkdirSync(join(f.root,'.agents/skills/autonomous'),{recursive:true});const file=join(f.root,'.agents/skills/autonomous/SKILL.md');writeFileSync(file,'user-owned');
 const conflict=f.cli(['init','--provider','openspec','--agent','codex','--json']);assert.notEqual(conflict.status,0);assert.equal(JSON.parse(conflict.stdout).error.code,'skill_conflict');assert.equal(readFileSync(file,'utf8'),'user-owned');assert.equal(existsSync(join(f.root,'openspec')),false);assert.equal(existsSync(join(f.root,'.git')),false);assert.equal(existsSync(join(f.base,'providers')),false);
});
test('yes and no-mcp use defaults and emit a readable localized completion summary',{skip:process.platform==='win32'},t=>{
 const f=fixture(t);const result=f.cli(['init','--yes','--no-mcp']);assert.equal(result.status,0,result.stderr+result.stdout);assert.match(result.stdout,/项目初始化完成/);assert.match(result.stdout,/\.spec-autonomous\/config.toml/);assert.match(result.stdout,/\$autonomous/);assert.equal(existsSync(join(f.root,'.codex/config.toml')),false);
});
// A POSIX PTY exercises Inquirer raw keyboard input, rather than mocking prompts.
for(const cancel of [false,true,"eof"])test(`terminal wizard ${cancel?`${cancel === 'eof' ? 'EOF' : 'Ctrl-C'} cancels without writes`:'accepts keyboard choices and initializes Claude with MCP'}`,{skip:process.platform==='win32' || spawnSync('python3',['--version']).status!==0,timeout:30000},async t=>{
 const f=fixture(t);let output='',step=0;
 const child=spawn('python3',[join(workspace,'tests/pty-driver.py'),runtime,launcher,'--path',f.root,'init'],{env:f.env,stdio:['pipe','pipe','pipe']});t.after(()=>{if(child.exitCode===null)child.kill('SIGKILL');});
 child.stdout.on('data',buffer=>{
  output+=buffer.toString();
  if(step===0&&output.includes('选择规范框架')){step++;child.stdin.write(cancel?(cancel === 'eof'?'\x04':'\x03'):'\x1b[B\r');}
  else if(step===1&&output.includes('选择使用的编程助手')){step++;child.stdin.write('\x1b[B\r');}
  else if(step===2&&output.includes('是否配置项目 MCP')){step++;child.stdin.write('\r');}
 });child.stderr.on('data',b=>output+=b);
 const code=await new Promise((resolve,reject)=>{child.on('error',reject);child.on('close',resolve);});
 if(cancel){assert.equal(code,130,output);assert.match(output,/已取消初始化/);assert.deepEqual(readdirSync(f.root),[]);assert.equal(existsSync(join(f.base,'providers')),false);}
 else{assert.equal(code,0,output);assert.match(output,/项目初始化完成/);assert.equal(readFileSync(f.marker,'utf8').trim(),'init');assert.ok(existsSync(join(f.root,'.claude/commands/autonomous.md')));assert.ok(existsSync(join(f.root,'.mcp.json')));}
});
