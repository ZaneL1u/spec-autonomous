import test from 'node:test';
import assert from 'node:assert/strict';
import { mkdtempSync, mkdirSync, rmSync } from 'node:fs';
import { join } from 'node:path';
import { tmpdir } from 'node:os';
import { resolveInitOptions } from '../lib/init-options.mjs';
function fixture(t, report = {}) {
 const root = mkdtempSync(join(tmpdir(), 'sa init options ')); t.after(() => rmSync(root, { recursive: true, force: true }));
 return { root, io: { detection: async () => ({ root, detected: [], selected: null, warnings: [], ...report }), env: {}, input: {isTTY:true}, output:{isTTY:true} } };
}
test('missing values are prompted once and explicit values survive', async t => {
 const {io} = fixture(t); const questions=[];
 const options=await resolveInitOptions({lang:'zh-CN'}, {...io,select:async config=>{questions.push(config);return ['speckit','claude',false][questions.length-1];}});
 assert.equal(options.provider,'speckit');assert.equal(options.agent,'claude');assert.equal(options.mcp,false);assert.equal(questions.length,3);
 assert.match(questions[0].message,/选择规范框架/);assert.match(questions[0].theme.style.keysHelpTip([]),/确认/);
 const complete=await resolveInitOptions({provider:'openspec',agent:'codex'}, {...io,select:()=>assert.fail('complete args must not prompt')});assert.equal(complete.mcp,false);
 const partial=await resolveInitOptions({provider:'openspec',mcp:false}, {...io,select:async ()=> 'claude'});assert.equal(partial.agent,'claude');assert.equal(partial.mcp,false);
});
test('yes selects documented defaults and never guesses ambiguous frameworks', async t=>{
 const {io}=fixture(t);const options=await resolveInitOptions({yes:true}, {...io,select:()=>assert.fail()});assert.equal(options.provider,'openspec');assert.equal(options.agent,'codex');assert.equal(options.mcp,true);
 const ambiguous=fixture(t,{detected:[{framework:'openspec'},{framework:'speckit'}]});await assert.rejects(resolveInitOptions({yes:true},ambiguous.io),/provider_selection_required/);
 const incomplete=fixture(t,{warnings:['incomplete']});await assert.rejects(resolveInitOptions({yes:true},incomplete.io),/provider_selection_required/);
});
test('detected provider and host are reused and CI/structured modes never prompt',async t=>{
 const {root,io}=fixture(t,{detected:[{framework:'speckit'}],selected:'speckit'});mkdirSync(join(root,'.claude'));
 const result=await resolveInitOptions({}, {...io,select:()=>assert.fail()});assert.equal(result.agent,'claude');assert.equal(result.provider,'speckit');
 const empty=fixture(t);
 for(const [options,extra] of [[{nonInteractive:true},{}],[{json:true},{}],[{format:'toml'},{}],[{}, {env:{CI:'1'}}],[{}, {input:{isTTY:false}}]]) await assert.rejects(resolveInitOptions(options,{...empty.io,...extra,select:()=>assert.fail()}),/provider_selection_required/);
 await assert.rejects(resolveInitOptions({interactive:true,json:true},empty.io),/init_mode_conflict/);
 await assert.rejects(resolveInitOptions({interactive:true},{...empty.io,input:{isTTY:false}}),/init_terminal_required/);
});
test('cancellation propagates before any initializer can be invoked',async t=>{
 const {io}=fixture(t);const cancelled=Object.assign(new Error('cancelled'),{name:'ExitPromptError'});
 await assert.rejects(resolveInitOptions({}, {...io,select:async()=>{throw cancelled;}}),e=>e===cancelled);
});
