#!/usr/bin/env node
// A deterministic test double for the documented worker protocol, not a model.
// It writes real code, while the production host runs real tests and Git operations.
import {readFileSync,writeFileSync,mkdirSync,existsSync} from 'node:fs';
import {dirname,join} from 'node:path';
import {spawn} from 'node:child_process';
import {createHash} from 'node:crypto';

let input=JSON.parse(readFileSync(process.env.SPEC_AUTONOMOUS_INPUT,'utf8'));
if(input.snapshot?.metadata?.context_reference){const ref=input.snapshot.metadata.context_reference;const bytes=readFileSync(ref.path);if(createHash('sha256').update(bytes).digest('hex')!==ref.sha256)throw Error('Context reference hash mismatch');input=JSON.parse(bytes);}
const scenario=JSON.parse(readFileSync('.mock/scenario.json','utf8'));
const output={schema_version:1,run_id:input.run_id,task_id:input.task_id,attempt_id:input.attempt_id,status:'candidate',summary:`Mock ${input.kind}`,blockers:[],milestone:null,plan:null,audit:[]};
const write=(path,body)=>{mkdirSync(dirname(path),{recursive:true});writeFileSync(path,body);};
const phases=scenario.phases.map((p)=>({...p,source:{kind:input.framework==='openspec'?'openspec-change':'speckit-feature',selector:input.framework==='openspec'?p.slug:`specs/${p.slug}`}}));
const phase=phases.find(p=>p.source.selector===input.snapshot?.selector);
if(process.env.MOCK_HANG==='1'){
  const child=spawn(process.execPath,['-e','setInterval(()=>{},1000)'],{stdio:'ignore'});
  write(join(dirname(process.env.SPEC_AUTONOMOUS_INPUT),'grandchild.pid'),String(child.pid));
  await new Promise(()=>{});
}
if(input.kind==='roadmap'){
  output.milestone={schema_version:1,id:input.milestone.id,goal:input.goal,framework:input.framework,revision:1,phases:phases.map(({slug,tasks,...p})=>p),verification:[{argv:[process.execPath,'--test',...phases.flatMap(p=>p.tasks.map(t=>t.test))],cwd:'.'}]};
}else if(input.kind==='native-planning'){
  const stage=input.snapshot.next_action.artifact;
  const source=input.snapshot.source_dir;
  let path=input.snapshot.next_action.outputs[0];
  let content='';
  if(stage==='proposal')content=`## Why\n\n${input.goal}\n\n## What Changes\n\n- Implement ${phase.title}.\n\n## Capabilities\n\n### New Capabilities\n\n- arithmetic: Verified arithmetic operations.\n\n### Modified Capabilities\n\nNone.\n\n## Impact\n\nLocal JavaScript modules and tests.\n`;
  else if(stage==='specs'){
    path=`${source}/specs/arithmetic/spec.md`;
    content=`## Purpose\n\nProvide correct and independently verifiable arithmetic behavior for the local mock project used to exercise the orchestration pipeline.\n\n## ADDED Requirements\n\n### Requirement: Arithmetic correctness\nThe system SHALL implement the operations assigned to this phase correctly.\n\n#### Scenario: Arithmetic checks\n- **WHEN** the phase test suite runs\n- **THEN** every arithmetic assertion passes\n`;
  }else if(stage==='design'||stage==='plan')content=`# ${phase.title}\n\n## Context\n\nUse independent JavaScript modules and Node test contracts.\n\n## Goals / Non-Goals\n\nImplement phase scope only.\n\n## Decisions\n\nSeparate modules, verify each with its dedicated test.\n\n## Risks / Trade-offs\n\nDo not break prior arithmetic operations.\n`;
  else if(stage==='tasks')content=`## 1. Implementation\n\n${phase.tasks.map((t,i)=>`- [ ] ${input.framework==='openspec'?`1.${i+1}`:`T${String(i+1).padStart(3,'0')}`} [P] [US1] Implement ${t.name} in ${t.path}`).join('\n')}\n`;
  else if(stage==='specify')content=`# Feature Specification: ${phase.title}\n\n## User Scenarios & Testing\n\nUsers receive correct arithmetic results. All listed phase tests must pass.\n\n## Requirements\n\n${phase.tasks.map(t=>`- Implement ${t.name} in ${t.path}`).join('\n')}\n`;
  else if(stage==='constitution')content='# Constitution\n\nUse tests as executable contracts; preserve native specifications.\n';
  else throw new Error(`Unsupported mock planning stage: ${stage}`);
  if(stage==='tasks'&&process.env.MOCK_SPLIT_SOURCE==='1')content=`## 1. Implementation\n\n- [ ] ${input.framework==='openspec'?'1.1':'T001'} Implement ${phase.tasks.map(t=>t.path).join(' and ')}\n`;
  write(path,content);
  if(input.framework==='speckit'&&process.env.MOCK_NATIVE_SIDECARS==='1'){
    if(stage==='specify'){
      write('.specify/feature.json',JSON.stringify({feature_directory:join(process.cwd(),source)}));
      write(`${source}/checklists/requirements.md`,'# Native checklist\n\n- [x] Specification checked\n');
    } else if(stage==='plan'){
      for(const file of ['research.md','data-model.md','quickstart.md','contracts/api.md'])write(`${source}/${file}`,`# Native ${file}\n\nPreserve arithmetic requirements.\n`);
    }
  }
}else if(input.kind==='plan-tasks'){
  const pending=input.snapshot.tasks.filter(t=>!t.done);
  const tasks=pending.map((source,i)=>{
    const target=phase.tasks.find(t=>source.description.includes(t.path))??scenario.repair;
    return {id:`work-${target.name}`,description:source.description,source_ids:[source.id],depends_on:source.parallel?[]:pending.slice(0,i).map(s=>`work-${(phase.tasks.find(t=>s.description.includes(t.path))??scenario.repair).name}`),reads:target.reads??[],writes:[target.path],verification:[{argv:[process.execPath,'--test',target.test],cwd:'.'}]};
  });
  const split=process.env.MOCK_SPLIT_SOURCE==='1'?phase.tasks.map(target=>({id:`work-${target.name}`,description:`Implement ${target.name}`,source_ids:[pending[0].id],depends_on:[],reads:target.reads??[],writes:[target.path],verification:[{argv:[process.execPath,'--test',target.test],cwd:'.'}]})):tasks;
  output.plan={schema_version:1,phase_id:phase.id,source_hash:input.snapshot.source_hash,tasks:split};
}else if(input.kind==='implement'){
  const target=phase.tasks.find(t=>input.task.writes.includes(t.path))??scenario.repair;
  const delay=process.env.MOCK_SLOW_TASK===target.name?5000:Number(process.env.MOCK_SLEEP_MS??scenario.sleep_ms??60);
  await new Promise(r=>setTimeout(r,delay));
  const fail=process.env.MOCK_FAIL_ALWAYS===target.name||(process.env.MOCK_FAIL_ONCE??scenario.fail_once??'')===target.name&&!input.failure;
  write(target.path,fail?target.broken:target.content);
  if(process.env.MOCK_SCOPE_ESCAPE==='1')write('outside-scope.txt','unexpected change');
}else if(input.kind==='audit'){
  if(process.env.MOCK_REQUIRE_PROVENANCE==='1'){
    const ref=input.snapshot.metadata.execution_evidence;
    if(!ref)throw Error('Audit needs execution provenance');
    const bytes=readFileSync(ref.path);if(createHash('sha256').update(bytes).digest('hex')!==ref.sha256)throw Error('Evidence digest mismatch');
    const evidence=JSON.parse(bytes);if(evidence.run_id!==input.run_id||evidence.accepted_head!==input.base_commit)throw Error('Wrong evidence binding');
    if(!evidence.attempts.some(a=>a.kind==='native-planning')||!evidence.attempts.some(a=>a.kind==='implement'&&a.status==='integrated'))throw Error('Missing real attempt history');
    for(const a of evidence.attempts){for(const key of ['input','result','execution_log']){const r=a[key];if(r.availability!=='recorded'||createHash('sha256').update(readFileSync(r.path)).digest('hex')!==r.sha256)throw Error('Invalid attempt artifact');}}
    if(!evidence.verification.some(v=>v.exit_code===0&&v.tree_unchanged))throw Error('Missing host verification');
  }
  const targets=input.snapshot.metadata.scope==='milestone'?phases.flatMap(p=>p.tasks):phase.tasks;
  const passed=targets.every(task=>existsSync(task.path)&&readFileSync(task.path,'utf8')===task.content)&&!(process.env.MOCK_AUDIT_GAP==='1'&&phase.id==='P001'&&!existsSync(scenario.repair.path));
  output.audit=input.snapshot.metadata.acceptance.map(ref=>({requirement:ref.id,evidence:targets.map(t=>t.path).join(', '),passed}));
}else if(input.kind==='converge'){
  const path=input.snapshot.tracking_file;
  const before=readFileSync(path,'utf8');const round=(before.match(/Convergence/g)??[]).length;
  const prefix=input.framework==='openspec'?`9.${round+1}`:`T${1000+round}`;
  write(path,before+`\n## Phase 9: Convergence\n\n- [ ] ${prefix} Implement ${scenario.repair.name} in ${scenario.repair.path}\n`);
}else throw new Error(`Unknown mock unit kind: ${input.kind}`);
if(process.env.MOCK_FORGE_ID==='1')output.attempt_id='forged';
write(process.env.SPEC_AUTONOMOUS_RESULT,JSON.stringify(output));
