// Explicit native-tool acceptance. Tools may be downloaded into the provider cache.
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { mkdirSync, mkdtempSync, readFileSync, existsSync, writeFileSync } from 'node:fs';
import { join, resolve } from 'node:path';
import { tmpdir } from 'node:os';
import { fileURLToPath } from 'node:url';
if (!process.argv.includes('--run')) throw new Error('Use --run to initialize isolated projects with real native tools.');
const workspace=fileURLToPath(new URL('../',import.meta.url));
const launcher=process.env.SPEC_AUTONOMOUS_TEST_LAUNCHER || join(workspace,'packages/cli/bin/spec-autonomous.mjs');
const runtime=process.env.SPEC_AUTONOMOUS_JS_RUNTIME || process.execPath;
const sandbox=mkdtempSync(join(tmpdir(),'sa real init '));
const env:NodeJS.ProcessEnv={...process.env,OPENSPEC_TELEMETRY:'0',DO_NOT_TRACK:'1',SPEC_AUTONOMOUS_LANG:'zh-CN'};
for(const key of ['NODE_TEST_CONTEXT','NODE_CHANNEL_FD','SPECIFY_FEATURE','SPECIFY_FEATURE_DIRECTORY','SPECIFY_INIT_DIR'])delete env[key];
const results:any[]=[];
for(const [provider,agent] of [['openspec','codex'],['speckit','claude']] as const){
 const root=join(sandbox,`project-${provider}`);mkdirSync(root);
 const run=(args:string[])=>{const r=spawnSync(runtime,[launcher,'--path',root,...args],{env,encoding:'utf8',timeout:180000,maxBuffer:8*1024*1024});if(r.error)throw r.error;assert.equal(r.status,0,r.stderr+r.stdout);return (JSON.parse(r.stdout) as any).data;};
 const initialized=run(['init','--provider',provider,'--agent',agent,'--mcp','--non-interactive','--json']);
 for(const path of ['.git','.spec-autonomous/config.toml','.spec-autonomous/milestones','.spec-autonomous/plans','.spec-autonomous/archives',agent==='codex'?'.agents/skills/autonomous/SKILL.md':'.claude/commands/autonomous.md',agent==='codex'?'.codex/config.toml':'.mcp.json',provider==='openspec'?'openspec/config.yaml':'.specify/memory/constitution.md'])assert.ok(existsSync(join(root,path)),path);
 const config=join(root,'.spec-autonomous/config.toml');writeFileSync(config,readFileSync(config,'utf8')+'\n# preserved user setting\n');const before=readFileSync(config);
 const repeated=run(['init','--provider',provider,'--agent',agent,'--mcp','--non-interactive','--json']);assert.deepEqual(readFileSync(config),before);
 const status=run(['providers','status',provider,'--json']);assert.ok(status.providers[0].ready);
 results.push({provider,agent,root,initialized,repeated,tool:status.providers[0]});console.log(`PASS ${provider}/${agent}: empty directory, real native init, Git, config, Skills, MCP and repeat preservation`);
}
const report={platform:`${process.platform}-${process.arch}`,runtime,launcher,sandbox,results};
const output=resolve(process.env.SPEC_AUTONOMOUS_INIT_REPORT || join(workspace,'.artifacts/interactive-init-real.json'));writeFileSync(output,JSON.stringify(report,null,2)+'\n');console.log(`Report: ${output}`);
