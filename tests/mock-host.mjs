#!/usr/bin/env node
// Independent external host used by tests. The product CLI never imports this
// file, executes this process, or starts its deterministic semantic worker.
import {spawn,spawnSync} from 'node:child_process';
import {readFileSync,writeFileSync,openSync,closeSync} from 'node:fs';
import {dirname,join,resolve} from 'node:path';
import {fileURLToPath} from 'node:url';
import {performance} from 'node:perf_hooks';
export const workspace=fileURLToPath(new URL('../',import.meta.url));
export const binary=process.env.SPEC_AUTONOMOUS_TEST_BINARY??join(workspace,'target/debug',process.platform==='win32'?'spec-autonomous.exe':'spec-autonomous');
export function cleanEnv(extra={}){const env={...process.env,...extra};for(const key of ['NODE_TEST_CONTEXT','NODE_TEST_WORKER_ID','NODE_CHANNEL_FD','NODE_CHANNEL_SERIALIZATION_MODE','NODE_UNIQUE_ID'])delete env[key];return env;}
export function parseOutput(text){try{return JSON.parse(text)}catch{}return text.trim().split('\n').filter(Boolean).map(line=>JSON.parse(line)).at(-1)}
export function raw(root,args,env={}){
 const r=spawnSync(binary,['--path',root,...args,'--json'],{encoding:'utf8',timeout:120000,maxBuffer:16*1024*1024,env:cleanEnv(env)});
 if(r.error)throw r.error;let output;try{output=parseOutput(r.stdout)}catch{}
 return {...r,output,data:output?.data,details:`exit=${r.status}\n${r.stderr}\n${r.stdout.slice(-6000)}`};
}
export async function drive(root,args,{env={},onSnapshot=()=>{},maxRounds=300}={}){
 let current=raw(root,args,env);const jobs=new Map();let abort=false;
 const kill=child=>{if(!child?.pid)return;try{if(process.platform==='win32')spawnSync('taskkill',['/PID',String(child.pid),'/T','/F'],{stdio:'ignore'});else process.kill(-child.pid,'SIGKILL');}catch{child.kill('SIGKILL')}};
 const stop=()=>{abort=true;for(const {child} of jobs.values())kill(child);};
 process.once('SIGTERM',stop);process.once('SIGINT',stop);
 try{
  for(let round=0;round<maxRounds;round++){
   onSnapshot(current);
   if(current.status!==0||current.data?.status!=='awaiting_host'){if(current.data?.id){const full=raw(root,['status',current.data.id],env);if(full.data){current.output={schema_version:1,data:full.data};current.data=full.data;current.stdout=JSON.stringify(current.output);}}return current;}
   for(const request of current.data.work??[]){
    if(jobs.has(request.request_id))continue;
    if(request.status==='receiving'||request.status==='submitted'){jobs.set(request.request_id,{child:null,request,promise:Promise.resolve({request,owner:request.owner,code:0})});continue;}
    if(request.status==='claimed')throw Error('Cannot take over a claimed session; stop/revoke the old host work explicitly');
    const owner={host_id:'mock-host',session_id:`session-${request.request_id}`,fresh_context:true};
    const claimed=raw(root,['claim',request.run_id,request.request_id,'--token',request.token,'--host-id',owner.host_id,'--session-id',owner.session_id,'--fresh-context'],env);
    if(claimed.status!==0)throw Error(claimed.details);
    let input=JSON.parse(readFileSync(request.input_path));if(input.snapshot?.metadata?.context_reference)input=JSON.parse(readFileSync(input.snapshot.metadata.context_reference.path));
    const workerEnv=cleanEnv({...env,SPEC_AUTONOMOUS_INPUT:request.input_path,SPEC_AUTONOMOUS_RESULT:request.result_path,SPEC_AUTONOMOUS_ATTEMPT:request.request_id});
    // Fixture controls belong to this test host, never the product launcher.
    const toml=readFileSync(join(root,'.spec-autonomous/config.toml'),'utf8');
    for(const section of ['runner.environment','mock.environment']){
      const match=toml.match(new RegExp(`\\[${section.replace('.','\\.')}\\]([\\s\\S]*?)(?=\\n\\[|$)`));
      for(const row of (match?.[1]??'').split('\n')){const m=/^\s*([A-Z][A-Z0-9_]*)\s*=\s*(".*")\s*$/.exec(row);if(m)workerEnv[m[1]]=JSON.parse(m[2]);}
    }
    if(input.framework==='speckit'){workerEnv.SPECIFY_INIT_DIR=request.project;workerEnv.SPECIFY_FEATURE_DIRECTORY=join(request.project,input.snapshot?.selector??'');workerEnv.SPECIFY_FEATURE=input.snapshot?.selector?.split('/').at(-1)??'';}
    const dir=dirname(request.input_path);const start=performance.now();
    const stdout=openSync(join(dir,'stdout.log'),'w'),stderr=openSync(join(dir,'stderr.log'),'w');
    const configured=/^command = (\[.*\])$/m.exec(toml);const argv=configured?JSON.parse(configured[1]):[process.execPath,join(workspace,'tests/mock-agent.mjs')];
    const child=spawn(argv[0],argv.slice(1),{cwd:request.project,env:workerEnv,detached:process.platform!=='win32',stdio:['ignore',stdout,stderr]});closeSync(stdout);closeSync(stderr);writeFileSync(join(dir,'host-process.json'),JSON.stringify({pid:child.pid,host_session_id:owner.session_id}));
    const timer=setTimeout(()=>kill(child),Math.max(1,Math.min((request.limits?.attempt_timeout_seconds??20)*1000,request.limits?.run_remaining_ms??120000)));
    const promise=new Promise((resolve,reject)=>{child.once('error',reject);child.once('exit',(code,signal)=>{clearTimeout(timer);kill(child);resolve({request,owner,code,signal,duration:performance.now()-start});});});
    jobs.set(request.request_id,{child,promise,request});
   }
   if(jobs.size===0)throw Error(`No host work available: ${current.details}`);
   const ended=await Promise.race([...jobs.values()].map(j=>j.promise).concat(new Promise(r=>setTimeout(()=>r({poll:true}),100))));
   if(ended.poll){const next=raw(root,['next','--run-id',current.data.id],env);if(['paused','cancelled','needs_input'].includes(next.data?.status)){const final=raw(root,['status',current.data.id],env);final.status=next.data.status==='cancelled'?130:4;return final;}if(next.data?.run_remaining_ms===0){current=raw(root,['resume',current.data.id],env);continue;}for(const {request} of jobs.values())raw(root,['tools','call','work.heartbeat','--input',JSON.stringify({run_id:request.run_id,request_id:request.request_id,token:request.token})],env);continue;}
   jobs.delete(ended.request.request_id);
   if(abort){raw(root,['pause',ended.request.run_id]);return raw(root,['status',ended.request.run_id]);}
   if(ended.code!==0){
    let input=JSON.parse(readFileSync(ended.request.input_path));
    writeFileSync(ended.request.result_path,JSON.stringify({schema_version:1,run_id:input.run_id,task_id:input.task_id,attempt_id:input.attempt_id,status:'failed',summary:`external mock exited ${ended.code}`,blockers:[],milestone:null,plan:null,audit:[]}));
   }
   current=raw(root,['apply-result','--result',ended.request.result_path,'--token',ended.request.token,'--host-id',ended.owner.host_id,'--session-id',ended.owner.session_id,'--fresh-context'],env);
  }
  throw Error('Host fixture round limit exceeded');
 }finally{stop();await Promise.allSettled([...jobs.values()].map(j=>j.promise));for(const {request} of jobs.values())raw(root,['tools','call','work.revoke','--input',JSON.stringify({run_id:request.run_id,request_id:request.request_id,token:request.token,host_stopped:true,reason:'External test host stopped the child'})]);process.off('SIGTERM',stop);process.off('SIGINT',stop);}
}
if(process.argv[1]&&resolve(process.argv[1])===fileURLToPath(import.meta.url)){
 const index=process.argv.indexOf('--path');if(index<0)throw Error('mock-host requires --path');const root=resolve(process.argv[index+1]);const args=process.argv.slice(index+2).filter(x=>x!=='--json');
 const result=await drive(root,args,{onSnapshot:r=>{if(r.data)console.log(JSON.stringify({event:'host_snapshot',data:{run_id:r.data.id,status:r.data.status,stage:r.data.stage}}));}});
 console.log(JSON.stringify(result.output??{error:{message:result.details}}));process.exitCode=result.status??1;
}
