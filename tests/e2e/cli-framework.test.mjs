import test from 'node:test';
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { mkdtempSync, rmSync, existsSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { binary, workspace, cleanEnv } from './helpers.mjs';
import { runCli } from '../../packages/cli/lib/cli-program.mjs';
const launcher = process.env.SPEC_AUTONOMOUS_TEST_LAUNCHER || join(workspace, 'packages/cli/bin/spec-autonomous.mjs');

function setup(t) {
  const root = mkdtempSync(join(tmpdir(), 'sa cli grammar '));
  t.after(() => rmSync(root, { recursive: true, force: true }));
  const home = join(root, 'provider-home');
  const env = cleanEnv({ SPEC_AUTONOMOUS_BINARY: binary, SPEC_AUTONOMOUS_PROVIDER_HOME: home, SPEC_AUTONOMOUS_OFFLINE: '1' });
  return { root, home, env, cli: args => {
    const result = spawnSync(process.execPath, [launcher, ...args], { cwd: root, env, encoding: 'utf8', timeout: 15_000 });
    if (result.error) throw result.error;
    return result;
  } };
}
test('Commander provides root and nested help without provider or project writes', t => {
  const f = setup(t);
  const root = f.cli(['--help']); assert.equal(root.status, 0, root.stderr);
  assert.match(root.stdout, /providers/); assert.match(root.stdout, /--path/); assert.equal(root.stderr, '');
  assert.doesNotMatch(root.stdout, /cli-metadata/);
  const direct = f.cli(['providers', 'ensure', '--help']);
  const nested = f.cli(['help', 'providers', 'ensure']);
  assert.equal(direct.status, 0, direct.stderr); assert.equal(nested.status, 0, nested.stderr);
  assert.equal(direct.stdout, nested.stdout); assert.match(direct.stdout, /--managed/);
  assert.doesNotMatch(direct.stdout, /providers exec/);
  assert.equal(f.cli([]).status, 0);
  assert.equal(existsSync(f.home), false); assert.equal(existsSync(join(f.root, '.spec-autonomous')), false);
});
test('syntax errors, invalid enums and native conflicts are rejected before installation', t => {
  const f = setup(t);
  for (const args of [
    ['init', '--provider', 'openspec', '--agent', 'unsupported'],
    ['init', '--provider', 'openspec', '--agent', 'codex', '--typo'],
    ['init', '--provider', 'openspec', '--framework', 'speckit', '--agent', 'codex'],
    ['prepare', '--max-workers', 'oops'], ['prepare', '--change', 'a', '--feature', 'b'],
    ['providers', 'ensure', 'openspec', 'extra'], ['providers', 'ensure', 'unknown'],
    ['providers', 'ensure', 'openspec', '--framework', 'speckit'], ['perpare'],
    ['progress', '--json', '--format', 'toml'],
  ]) {
    const result = f.cli([...args, '--json']);
    assert.equal(result.status, 2, `${args}: ${result.stderr} ${result.stdout}`);
    assert.equal(JSON.parse(result.stdout).error.code, 'invalid_arguments');
    assert.equal(result.stderr, '', result.stderr);
  }
  assert.equal(existsSync(f.home), false); assert.equal(existsSync(join(f.root, 'openspec')), false);
});
test('global values work before and after nested provider commands, including equals notation', t => {
  const f = setup(t);
  for (const args of [
    ['--path', f.root, 'providers', 'status', 'openspec', '--json'],
    ['providers', '--path', f.root, 'status', 'openspec', '--json'],
    ['providers', 'status', 'openspec', `--path=${f.root}`, '--format=json'],
  ]) {
    const result = f.cli(args); assert.equal(result.status, 0, result.stderr + result.stdout);
    assert.equal(JSON.parse(result.stdout).data.providers[0].provider, 'openspec');
  }
  assert.equal(existsSync(f.home), false);
});
test('native aliases and argv are forwarded unchanged after Clap preflight', async () => {
  const argv = ['--path', 'path with spaces', 'auto', '--goal', 'a b $(literal) `text`', '--framework', 'openspec', '--json'];
  let forwarded, source;
  const code = await runCli(argv, { binary, context: () => ({ ensureSource: async value => { source = value; } }),
    forward: async args => { forwarded = args; return 7; }, stdout: () => assert.fail('no wrapper stdout'), stderr: message => assert.fail(message) });
  assert.equal(code, 7); assert.deepEqual(forwarded, [binary, ...argv]); assert.equal(source.framework, 'openspec');
});
test('provider exec preserves delimiter payload flags and shell text', async () => {
  let forwarded;
  const payload = ['--help', '--json', 'a b', '$(literal)', '--path', 'native path'];
  const code = await runCli(['providers', 'exec', 'openspec', '--managed', '--', ...payload], { binary,
    context: () => ({ path: workspace, manager: { env: {} }, ensureSelected: async (provider, options) => {
      assert.equal(provider, 'openspec'); assert.equal(options.managed, true); return { command: ['/mock/native-provider'] };
    } }), forward: async argv => { forwarded = argv; return 23; }, stdout: () => assert.fail('payload --help must not print wrapper help'), stderr: message => assert.fail(message) });
  assert.equal(code, 23); assert.deepEqual(forwarded, ['/mock/native-provider', ...payload]);
});
test('structured native input and init use parsed objects without rescanning argv', async t => {
  const f = setup(t); const file = join(f.root, 'input file.json'); writeFileSync(file, '{"framework":"speckit"}');
  let source, options;
  const common = { binary, stdout: () => {}, stderr: message => assert.fail(message), forward: async () => 0,
    context: (_binary, parsed) => { options = parsed; return { ensureSource: async value => { source = value; } }; } };
  assert.equal(await runCli(['tools', 'call', 'native.instructions', '--input', `@${file}`], common), 0);
  assert.equal(source.framework, 'speckit');
  assert.equal(await runCli(['--framework', 'openspec', 'tools', 'call', 'native.instructions', '--input', `@${file}`], common), 0);
  assert.equal(source.framework, 'openspec');
  assert.equal(await runCli(['init', '--path', f.root, '--agent', 'codex', '--framework', 'auto', '--provider', 'openspec', '--prefix', 'literal-prefix', '--mcp'], { ...common,
    initialize: async () => { assert.equal(options.agent, 'codex'); assert.equal(options.provider, 'openspec'); assert.equal(options.framework, 'openspec'); assert.equal(options.path, f.root); },
  }), 0);
});
test('Chinese environment and explicit English override localize only human text', t => {
  const f = setup(t);
  const run = args => spawnSync(process.execPath, [launcher, ...args], { cwd: f.root, env: { ...f.env, LC_ALL: 'zh_CN.UTF-8' }, encoding: 'utf8', timeout: 15_000 });
  const zh = run(['--help']); assert.equal(zh.status, 0, zh.stderr); assert.match(zh.stdout, /准备完整工作包/); assert.doesNotMatch(zh.stdout, /Deterministic SDD capabilities/);
  const en = run(['--lang', 'en-US', '--help']); assert.equal(en.status, 0, en.stderr); assert.match(en.stdout, /Prepare complete work packets/); assert.doesNotMatch(en.stdout, /准备完整工作包/);
  const zhError = run(['--lang', 'zh-CN', '--path', join(f.root, 'missing'), 'detect', '--json']);
  const enError = run(['--lang', 'en-US', '--path', join(f.root, 'missing'), 'detect', '--json']);
  assert.equal(zhError.status, 2); assert.equal(enError.status, 2);
  const zhJson = JSON.parse(zhError.stdout), enJson = JSON.parse(enError.stdout);
  assert.equal(zhJson.error.code, enJson.error.code); assert.notEqual(zhJson.error.message, enJson.error.message);
});
