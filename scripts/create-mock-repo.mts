import {mkdirSync,writeFileSync,existsSync,readFileSync} from 'node:fs';
import {resolve,join,dirname} from 'node:path';
import {fileURLToPath} from 'node:url';
import {spawnSync} from 'node:child_process';
import {parseArgs} from 'node:util';

type Framework='openspec'|'speckit';
interface Check {argv:string[];cwd:string}
interface Task {name:string;path:string;test:string;content:string;broken:string;reads?:string[]}
interface Phase {id:string;label:string;slug:string;title:string;depends_on:string[];verification:Check[];tasks:Task[]}
interface MockOptions {goalOnly?:boolean;failOnce?:string;sleepMs?:number;phaseRepair?:boolean;milestoneRepair?:boolean}

export const workspace=fileURLToPath(new URL('../',import.meta.url));
export function createMock(output:string,framework:Framework='openspec',{goalOnly=false,failOnce='',sleepMs=80,phaseRepair=false,milestoneRepair=false}:MockOptions={}){
  if(!['openspec','speckit'].includes(framework))throw new Error('Unsupported fixture provider');
  if(existsSync(output))throw new Error(`Refusing to overwrite ${output}`);
  mkdirSync(output,{recursive:true});
  const write=(path:string,body:string)=>{mkdirSync(dirname(join(output,path)),{recursive:true});writeFileSync(join(output,path),body);};
  const run=(args:string[])=>{const r=spawnSync('git',args,{cwd:output,encoding:'utf8'});if(r.status!==0)throw new Error(r.stderr);};
  const node=process.execPath;
  const check=(...tests:string[]):Check=>({argv:[node,'--test',...tests],cwd:'.'});
  const add:Task={name:'add',path:'src/add.mjs',test:'tests/add.test.mjs',content:'export const add = (a, b) => a + b;\n',broken:'export const add = (a, b) => a - b;\n'};
  const multiply:Task={name:'multiply',path:'src/multiply.mjs',test:'tests/multiply.test.mjs',content:'export const multiply = (a, b) => a * b;\n',broken:'export const multiply = (a, b) => a + b;\n'};
  const service:Task={name:'service',path:'src/service.mjs',test:'tests/service.test.mjs',reads:['src/add.mjs','src/multiply.mjs'],content:"import { add } from './add.mjs';\nimport { multiply } from './multiply.mjs';\nexport const calculate = (a, b) => ({sum:add(a,b),product:multiply(a,b)});\n",broken:'export const calculate = () => ({});\n'};
  const repair:Task={name:'version',path:'src/version.mjs',test:'checks/version.mjs',content:'export const version = 1;\n',broken:'export const version = 0;\n'};
  const phases:Phase[]=[{id:'P001',label:'1',slug:'arithmetic',title:'Arithmetic primitives',depends_on:[],verification:[check(add.test,multiply.test)],tasks:[add,multiply]},{id:'P002',label:'2',slug:'calculator',title:'Calculator service',depends_on:['P001'],verification:[check(service.test)],tasks:[service]}];
  if(phaseRepair)phases[0]!.verification.push(check(repair.test));
  const milestoneVerification=[check(...phases.flatMap(p=>p.tasks.map(t=>t.test))),...(milestoneRepair?[check(repair.test)]:[])];
  const scenario={phases,repair,fail_once:failOnce,sleep_ms:sleepMs,milestone_verification:milestoneVerification};
  write('.mock/scenario.json',JSON.stringify(scenario,null,2));
  write('README.md','# Local Spec Autonomous test repository\n\nGenerated mock SDD project. Its worker is deterministic; the host still runs actual Node assertions and Git integrations.\n');
  write('AGENTS.md','# Mock project\n\nImplement only the assigned native task. Use Node test contracts. Do not change .mock configuration, tests or source specifications.\n');
  write('.gitignore','node_modules/\n.spec-autonomous/*\n!.spec-autonomous/config.toml\n!.spec-autonomous/milestones/\n!.spec-autonomous/plans/\n');
  for(const [task,body] of [[add,"assert.equal(add(2,3),5); assert.equal(add(-4,3),-1);"],[multiply,"assert.equal(multiply(2,3),6); assert.equal(multiply(-4,3),-12);"],[service,"assert.deepEqual(calculate(2,3),{sum:5,product:6});"]] as [Task,string][]){
    const fn=task.name==='service'?'calculate':task.name;
    write(task.test,`import assert from 'node:assert/strict';\nimport {${fn}} from '../${task.path}';\n${body}\n`);
  }
  // Version test remains outside automatic discovery until convergence requests it.
  write('checks/version.mjs',"import assert from 'node:assert/strict'; import {version} from '../src/version.mjs'; assert.equal(version,1);\n");
  scenario.repair.test='checks/version.mjs';write('.mock/scenario.json',JSON.stringify(scenario,null,2));
  const runner=join(workspace,'tests/mock-agent.mts');
  const openspec=join(workspace,'node_modules/@fission-ai/openspec/bin/openspec.js');
  const q=JSON.stringify;
  const renderChecks=(checks:Check[])=>`[${checks.map(c=>`{argv = ${JSON.stringify(c.argv)}, cwd = ${q(c.cwd)}}`).join(", ")}]`;
  write('.spec-autonomous/config.toml',`schema_version = 1\n[execution]\nmax_workers = 2\nmax_attempts = 3\nattempt_timeout_seconds = 20\nrun_timeout_seconds = 180\nmax_repair_rounds = 2\n[host]\nmax_concurrency = 2\n[runner]\nprofile = "command"\ncommand = [${q(node)}, ${q(runner)}]\nfresh_session = true\n[provider]\nopenspec_command = [${q(node)}, ${q(openspec)}]\n`);
  if(framework==='openspec'){
    write('openspec/config.yaml','schema: spec-driven\n');
    mkdirSync(join(output,'openspec/specs'),{recursive:true});mkdirSync(join(output,'openspec/changes/archive'),{recursive:true});
  }else{
    write('.specify/memory/constitution.md','# Constitution\n\nUse tests as executable contracts. Preserve source specifications.\n');
    for(const stage of ['constitution','specify','plan','tasks','converge']){
      const source=join(workspace,'tests/fixtures/speckit-native',`${stage}.md`);
      write(`.specify/templates/commands/${stage}.md`,readFileSync(source,'utf8'));
    }
  }
  if(!goalOnly){
    const phases_=phases.map(p=>({...p,source:{kind:framework==='openspec'?'openspec-change':'speckit-feature',selector:framework==='openspec'?p.slug:`specs/${p.slug}`}}));
    // Use Bun's TOML serializer only in tooling? Instead render this small fixture
    // declaration directly so Node alone can create test repositories.
    let manifest=`schema_version = 1\nid = "M001"\ngoal = "Build a tested arithmetic service"\nframework = "${framework}"\nrevision = 1\nverification = ${renderChecks(milestoneVerification)}\n`;
    for(const p of phases_){manifest+=`\n[[phases]]\nid = ${q(p.id)}\nlabel = ${q(p.label)}\ntitle = ${q(p.title)}\ndepends_on = ${JSON.stringify(p.depends_on)}\nsource = {kind = ${q(p.source.kind)}, selector = ${q(p.source.selector)}}\nverification = ${renderChecks(p.verification)}\n`;}
    write('.spec-autonomous/milestones/M001/milestone.toml',manifest);
  }
  run(['init','-q','-b','main']);run(['config','user.name','Mock Fixture']);run(['config','user.email','mock@example.invalid']);run(['add','--all']);run(['commit','-q','-m','test: initialize local mock SDD repository']);
  return {root:output,framework,goalOnly,milestoneId:goalOnly?null:'M001'};
}
if(process.argv[1]&&resolve(process.argv[1])===fileURLToPath(import.meta.url)){
  const {values}=parseArgs({options:{output:{type:'string'},framework:{type:'string',default:'openspec'},'goal-only':{type:'boolean'},'fail-once':{type:'string',default:''}}});
  const output=resolve(values.output??`.artifacts/mock-repositories/${values.framework}`);
  console.log(JSON.stringify(createMock(output,values.framework as Framework,{goalOnly:values['goal-only'],failOnce:values['fail-once']}),null,2));
}
