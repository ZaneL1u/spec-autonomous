import {mkdtempSync,rmSync,readFileSync,writeFileSync} from 'node:fs';
import {tmpdir} from 'node:os';
import {join,resolve} from 'node:path';
import {spawn,spawnSync} from 'node:child_process';
import {createMock,workspace} from '../../scripts/create-mock-repo.mjs';

export const binary=process.env.SPEC_AUTONOMOUS_TEST_BINARY??join(workspace,'target','debug',process.platform==='win32'?'spec-autonomous.exe':'spec-autonomous');
export function fixture(t,framework='openspec',options={}){
  const parent=mkdtempSync(join(tmpdir(),'sa e2e '));
  t.after(()=>rmSync(parent,{recursive:true,force:true}));
  const root=join(parent,'project');createMock(root,framework,options);return root;
}
export function parseOutput(stdout){
  try{return JSON.parse(stdout);}catch{}
  const rows=stdout.trim().split('\n').filter(Boolean).map(line=>JSON.parse(line));
  return rows.at(-1);
}
export function cleanEnv(extra={}){const env={...process.env,...extra};for(const key of ["NODE_TEST_CONTEXT","NODE_TEST_WORKER_ID","NODE_CHANNEL_FD","NODE_CHANNEL_SERIALIZATION_MODE","NODE_UNIQUE_ID"])delete env[key];return env;}
export function cli(root,args,options={}){
  const passive=['run','autonomous','auto','prepare','plan','milestone','resume'].includes(args[0]);
  const program=passive?process.execPath:binary;const argv=passive?[join(workspace,'tests/mock-host.mjs'),'--path',root,...args,'--json']:['--path',root,...args,'--json'];
  const r=spawnSync(program,argv,{cwd:workspace,encoding:'utf8',timeout:options.timeout??120000,maxBuffer:16*1024*1024,env:cleanEnv(options.env)});
  if(r.error)throw r.error;
  let output;try{output=parseOutput(r.stdout);}catch{output=null;}
  if(args[0]==='report'&&output?.data?.pagination?.events?.next_offset!=null){let next=output.data.pagination.events.next_offset;while(next!=null){const page=spawnSync(binary,['--path',root,...args,'--offset',String(next),'--limit','200','--json'],{encoding:'utf8',env:cleanEnv(options.env)});if(page.status!==0)throw Error(page.stderr);const data=parseOutput(page.stdout).data;output.data.events.push(...data.events);next=data.pagination?.events?.next_offset;}}
  return {...r,output,data:output?.data,details:`exit=${r.status}\n${r.stderr}\n${r.stdout.slice(-6000)}`};
}
export function start(root,args,options={}){
  const child=spawn(process.execPath,[join(workspace,'tests/mock-host.mjs'),'--path',root,...args,'--json'],{cwd:workspace,env:cleanEnv(options.env),stdio:['ignore','pipe','pipe']});
  let stdout='',stderr='';child.stdout.on('data',b=>stdout+=b);child.stderr.on('data',b=>stderr+=b);
  const finished=new Promise((res,rej)=>{child.on('error',rej);child.on('exit',(status,signal)=>{let output;try{output=parseOutput(stdout);}catch{output=null;}res({status,signal,stdout,stderr,output,data:output?.data,details:`exit=${status}\n${stderr}\n${stdout.slice(-6000)}`});});});
  return {child,finished,output:()=>stdout};
}
export async function until(fn,timeout=20000){const end=Date.now()+timeout;while(Date.now()<end){const result=await fn();if(result)return result;await new Promise(r=>setTimeout(r,40));}throw Error('Timed out waiting for observable state');}
export function git(root,args){const r=spawnSync('git',args,{cwd:root,encoding:'utf8'});if(r.status!==0)throw Error(r.stderr);return r.stdout.trim();}
export function configure(root,append){const path=join(root,'.spec-autonomous/config.toml');writeFileSync(path,readFileSync(path,'utf8')+append);git(root,['add','--all']);git(root,['commit','-qm','test: configure scenario']);}
export function edit(root,path,transform){const file=join(root,path);writeFileSync(file,transform(readFileSync(file,'utf8')));}
export {workspace,createMock,join,resolve};
