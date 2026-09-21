#!/usr/bin/env node
// Portable test driver: Bun/npm only select this script; neither owns the tests.
import {spawnSync} from 'node:child_process';
import {readdirSync} from 'node:fs';
import {fileURLToPath} from 'node:url';
const root=fileURLToPath(new URL('../',import.meta.url));
const files=(dir:string)=>readdirSync(new URL(`../${dir}/`,import.meta.url)).filter(f=>f.endsWith('.test.mts')).sort().map(f=>`${dir}/${f}`);
const env:NodeJS.ProcessEnv={...process.env,OPENSPEC_TELEMETRY:'0',DO_NOT_TRACK:'1'};
for(const key of ['NODE_TEST_CONTEXT','NODE_TEST_WORKER_ID','NODE_CHANNEL_FD','NODE_CHANNEL_SERIALIZATION_MODE','NODE_UNIQUE_ID'])delete env[key];
const steps:[string,string[]][]=[
  [process.execPath,['node_modules/typescript/bin/tsc','--noEmit']],
  // The committed packages/cli/{bin,lib} output must match its TypeScript
  // sources; CI additionally fails when this build leaves the tree dirty.
  [process.execPath,['node_modules/tsdown/dist/run.mjs']],
  ['cargo',['fmt','--all','--','--check']],
  ['cargo',['clippy','--workspace','--all-targets','--locked','--','-D','warnings']],
  ['cargo',['test','--workspace','--locked']],
  ['cargo',['test','-p','spec-autonomous-core','--locked','--test','provider_contracts','real_openspec_plans_then_accepts_skipped_specs_and_custom_tracking_artifact','--','--ignored','--exact']],
  [process.execPath,['--test',...files('packages/cli/test'),...files('scripts/test')]],
  ['cargo',['build','-p','spec-autonomous-cli','--locked']],
  [process.execPath,['--test','--test-concurrency=2',...files('tests/e2e')]],
  [process.execPath,['node_modules/@fission-ai/openspec/bin/openspec.js','validate','--all','--strict','--no-interactive']],
];
for(const [command,args] of steps){
  console.log(`\nRunning ${command} ${args.join(' ')}`);
  const result=spawnSync(command,args,{cwd:root,env,stdio:'inherit',shell:false});
  if(result.error){console.error(result.error.message);process.exit(1);}
  if(result.status!==0)process.exit(result.status??1);
}
