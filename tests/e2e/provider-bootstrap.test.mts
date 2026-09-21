import test from 'node:test';
import assert from 'node:assert/strict';
import { spawn, spawnSync } from 'node:child_process';
import { mkdtempSync, mkdirSync, writeFileSync, readFileSync, existsSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { delimiter, join } from 'node:path';
import { binary, workspace, fixture, git, cleanEnv } from './helpers.mts';
const launcher = process.env.SPEC_AUTONOMOUS_TEST_LAUNCHER || join(workspace, 'packages/cli/bin/spec-autonomous.mjs');

function setup(t: any, options: {fail?:boolean} = {}) {
  const root = fixture(t, 'openspec');
  const parent = mkdtempSync(join(tmpdir(), 'sa npm provider fixture '));
  t.after(() => rmSync(parent, { recursive: true, force: true }));
  const config = join(root, '.spec-autonomous/config.toml');
  writeFileSync(config, readFileSync(config, 'utf8').replace(/^openspec_command = .*\n/m, ''));
  git(root, ['add', '--all']); git(root, ['commit', '-qm', 'test: missing native tool']);
  const installedEntry = `import { spawnSync } from 'node:child_process';
if (process.argv[2] === '--echo') { console.log(JSON.stringify(process.argv.slice(3))); process.exit(7); }
const result = spawnSync(${JSON.stringify(process.execPath)}, [${JSON.stringify(join(workspace, 'node_modules/@fission-ai/openspec/bin/openspec.js'))}, ...process.argv.slice(2)], {stdio:'inherit',env:{...process.env,OPENSPEC_TELEMETRY:'0'}}); process.exit(result.status ?? 1);`;
  const npm = join(parent, 'npm-cli.js'), marker = join(parent, 'installs');
  writeFileSync(npm, `const fs=require('node:fs'),path=require('node:path');
fs.appendFileSync(${JSON.stringify(marker)},'install\\n');
if (${Boolean(options.fail)}) process.exit(23);
const target=path.join(process.cwd(),'node_modules/@fission-ai/openspec/bin');fs.mkdirSync(target,{recursive:true});
fs.writeFileSync(path.join(target,'openspec.js'),${JSON.stringify(installedEntry)});`);
  const home = join(parent, 'providers');
  const path = (process.env.PATH || '').split(delimiter).filter(directory =>
    !['openspec', 'openspec.cmd', 'openspec.exe'].some(name => existsSync(join(directory, name)))
  ).join(delimiter);
  const env = cleanEnv({ PATH: path, SPEC_AUTONOMOUS_BINARY: binary, SPEC_AUTONOMOUS_PROVIDER_HOME: home, npm_execpath: npm, SPEC_AUTONOMOUS_OFFLINE: '0' });
  return { root, parent, home, marker, env };
}
function cli(f: any, args: any) {
  const r = spawnSync(process.execPath, [launcher, '--path', f.root, ...args], { env: f.env, encoding: 'utf8', timeout: 30_000 });
  if (r.error) throw r.error;
  return r;
}
function connect(t: any, f: any) {
  const child = spawn(process.execPath, [launcher, '--path', f.root, 'mcp'], { env: f.env, stdio: ['pipe', 'pipe', 'pipe'] });
  let id = 0, buffer = '', stderr = ''; const pending = new Map();
  child.stderr.on('data', b => stderr += b);
  child.stdout.on('data', b => {
    buffer += b; let index;
    while ((index = buffer.indexOf('\n')) >= 0) {
      const line = buffer.slice(0, index); buffer = buffer.slice(index + 1);
      let response; try { response = JSON.parse(line); } catch { for (const p of pending.values()) p.reject(new Error(`MCP stdout polluted: ${line}`)); return; }
      pending.get(response.id)?.resolve(response); pending.delete(response.id);
    }
  });
  child.on('exit', code => { for (const p of pending.values()) p.reject(new Error(`MCP exited ${code}: ${stderr}`)); pending.clear(); });
  t.after(() => child.kill('SIGTERM'));
  const request = (method: any, params = {}) => new Promise<any>((resolve, reject) => {
    const sequence = ++id; pending.set(sequence, { resolve, reject }); child.stdin.write(JSON.stringify({ jsonrpc: '2.0', id: sequence, method, params }) + '\n');
  });
  return { request, child, call: (name: any, args: any) => request('tools/call', { name, arguments: args }), init: async () => {
    await request('initialize', { protocolVersion: '2025-11-25', capabilities: {}, clientInfo: { name: 'provider-test', version: '1' } });
    child.stdin.write(JSON.stringify({ jsonrpc: '2.0', method: 'notifications/initialized' }) + '\n');
  } };
}
test('npm init automatically installs missing OpenSpec, preserves specs and forwards native exec argv', { timeout: 30_000 }, t => {
  const f = setup(t), spec = join(f.root, 'openspec/config.yaml'), original = readFileSync(spec);
  const result = cli(f, ['init', '--agent', 'codex', '--json']);
  assert.equal(result.status, 0, result.stderr + result.stdout);
  assert.equal(JSON.parse(result.stdout).data.framework, 'openspec');
  assert.deepEqual(readFileSync(spec), original); assert.equal(readFileSync(f.marker, 'utf8').trim().split('\n').length, 1);
  const exec = cli(f, ['providers', 'exec', 'openspec', '--', '--echo', 'a b', '$(literal)', '--help']);
  assert.equal(exec.status, 7, exec.stderr); assert.deepEqual(JSON.parse(exec.stdout), ['a b', '$(literal)', '--help']);
  const again = cli(f, ['providers', 'ensure', 'openspec', '--json']);
  assert.equal(again.status, 0, again.stderr); assert.equal(JSON.parse(again.stdout).data.source, 'managed');
  assert.equal(readFileSync(f.marker, 'utf8').trim().split('\n').length, 1);
});
test('passive commands and malformed provider arguments cannot start an installation', t => {
  const f = setup(t); f.env.SPEC_AUTONOMOUS_OFFLINE = '1';
  for (const args of [['progress', '--json'], ['doctor', '--json'], ['tools', 'list', '--json'], ['providers', 'status', '--json']]) {
    const r = cli(f, args); assert.equal(r.status, 0, r.stderr + r.stdout); JSON.parse(r.stdout);
  }
  const typo = cli(f, ['providers', 'ensure', 'openspec', '--typo', '--json']);
  assert.equal(typo.status, 2); assert.match(typo.stdout, /invalid_arguments/);
  const missing = cli(f, ['providers', 'ensure', 'openspec', '--json']);
  assert.equal(missing.status, 1); assert.match(missing.stdout, /provider_offline/);
  assert.equal(existsSync(f.home), false); assert.equal(existsSync(f.marker), false);
});
test('ambiguous frameworks fail before download; explicit init selects an existing provider', t => {
  const f = setup(t); mkdirSync(join(f.root, '.specify/templates'), { recursive: true });
  const blocked = cli(f, ['init', '--agent', 'codex', '--json']);
  assert.equal(blocked.status, 1); assert.match(blocked.stdout, /selection_required/); assert.equal(existsSync(f.marker), false);
  const selected = cli(f, ['init', '--provider', 'openspec', '--agent', 'codex', '--json']);
  assert.equal(selected.status, 0, selected.stderr + selected.stdout); assert.equal(JSON.parse(selected.stdout).data.framework, 'openspec');
});
test('MCP adds provider tools without stdout pollution and lazily installs before native prepare', { timeout: 45_000 }, async t => {
  const f = setup(t), mcp = connect(t, f);
  assert.equal((await mcp.call('sa_providers', { operation: 'ensure', provider: 'openspec' })).error.code, -32002);
  assert.equal(existsSync(f.marker), false);
  await mcp.init();
  const tools = await mcp.request('tools/list'); assert.equal(tools.result.tools.length, 10);
  assert.ok(tools.result.tools.some((tool: any) => tool.name === 'sa_providers'));
  await mcp.call('sa_progress', {}); await mcp.call('sa_doctor', {});
  const status = await mcp.call('sa_providers', { operation: 'status' });
  assert.equal(status.result.structuredContent.data.providers.find((p: any) => p.provider === 'openspec').source, 'missing');
  assert.equal(existsSync(f.marker), false);
  const prepared = await mcp.call('sa_prepare', { milestone_id: 'M001' });
  assert.equal(prepared.result.isError, false, JSON.stringify(prepared));
  assert.equal(prepared.result.structuredContent.data.starts_agents, false);
  assert.equal(existsSync(f.marker), true);
  const ready = await mcp.call('sa_providers', { operation: 'ensure', provider: 'openspec' });
  assert.equal(ready.result.structuredContent.data.ready, true);
  assert.equal(readFileSync(f.marker, 'utf8').trim().split('\n').length, 1);
});
test('MCP install failure is a tool error and leaves no core run or ready receipt', { timeout: 30_000 }, async t => {
  const f = setup(t, { fail: true }), mcp = connect(t, f); await mcp.init();
  const result = await mcp.call('sa_prepare', { milestone_id: 'M001' });
  assert.equal(result.result.isError, true); assert.match(result.result.content[0].text, /provider_install_failed/);
  assert.equal(existsSync(join(f.home, 'openspec/current.json')), false);
  const progress = await mcp.call('sa_progress', {}); assert.equal(progress.result.isError, false);
});
