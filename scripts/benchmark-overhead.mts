// Runs deterministic fixture hosts against explicitly selected binaries only.
// No consumer repository, installed executable or global configuration is changed.
import {spawnSync} from 'node:child_process';
import {mkdtempSync,rmSync,readFileSync,writeFileSync,mkdirSync} from 'node:fs';
import {tmpdir} from 'node:os';
import {join,resolve,dirname} from 'node:path';
import {parseArgs} from 'node:util';
import {createMock,workspace} from './create-mock-repo.mts';
const {values}=parseArgs({options:{baseline:{type:'string'},candidate:{type:'string'},output:{type:'string'}}});
if(!values.baseline||!values.candidate)throw Error('Supply --baseline and --candidate absolute binary paths');
const output=resolve(values.output??'.artifacts/overhead-benchmark.json');
const results=[];
for(const [label,executable] of [['baseline',values.baseline],['candidate',values.candidate]]){
 const parent=mkdtempSync(join(tmpdir(),'sa-overhead-benchmark-'));
 try{
  const root=join(parent,'project');createMock(root,'speckit',{sleepMs:0});
  const config=join(root,'.spec-autonomous/config.toml');writeFileSync(config,readFileSync(config,'utf8').replace('max_workers = 2','max_workers = 1').replace('max_concurrency = 2','max_concurrency = 1'));
  const committed=spawnSync('git',['-C',root,'commit','-qam','benchmark: single worker'],{encoding:'utf8'});if(committed.status!==0)throw Error(committed.stderr);
  const env={...process.env,SPEC_AUTONOMOUS_TEST_BINARY:resolve(executable!)};
  const started=Date.now();
  const run=spawnSync(process.execPath,[join(workspace,'tests/mock-host.mts'),'--path',root,'prepare','--milestone','M001','--json'],{env,encoding:'utf8',timeout:180000,maxBuffer:32*1024*1024});
  if(run.status!==0)throw Error(`${label}: ${run.stderr}\n${run.stdout.slice(-5000)}`);
  const state=JSON.parse(run.stdout.trim().split('\n').at(-1)!).data;if(state.status!=='completed')throw Error(`${label}: ${state.status}`);
  const inventory=spawnSync('git',['-C',root,'worktree','list','--porcelain'],{encoding:'utf8'});if(inventory.status!==0)throw Error(inventory.stderr);
  results.push({label,wall_ms:Date.now()-started,binary:resolve(executable!),status:state.status,verified_tasks:state.completed_tasks.length,attempts:state.attempts.length,verification_checks:state.evidence.length,verification_reuses:state.verification_reuses??0,remaining_worktrees:inventory.stdout.split('\n').filter(l=>l.startsWith('worktree ')).length,attempt_checkouts:new Set(state.attempts.map((a:any)=>a.worktree)).size,intent_checkouts:new Set(state.intents.map((i:any)=>i.worktree)).size});
 }finally{rmSync(parent,{recursive:true,force:true});}
}
mkdirSync(dirname(output),{recursive:true});writeFileSync(output,JSON.stringify({scenario:'same two-phase/three-task Spec Kit fixture, one worker, real Git and Node assertions, deterministic test agents',results},null,2)+'\n');
console.log(JSON.stringify({output,results},null,2));
