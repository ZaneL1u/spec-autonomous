#!/usr/bin/env node
import { cp, mkdtemp, rm, mkdir, writeFile, readFile, readdir } from 'node:fs/promises';
import { existsSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';
const root = fileURLToPath(new URL('../', import.meta.url));
const source = join(root, 'playground');
const temp = await mkdtemp(join(tmpdir(), 'spec-autonomous-dogfood-'));
const project = join(temp, 'todo-project');
await cp(source, project, { recursive: true });
await mkdir(join(project, '.mock'), { recursive: true });
const env = { ...process.env, SPEC_AUTONOMOUS_LANG: 'en', OPENSPEC_TELEMETRY: '0', DO_NOT_TRACK: '1' };
const binary = join(root, 'target/debug/spec-autonomous');
const launcher = join(root, 'packages/cli/bin/spec-autonomous.mjs');
env.SPEC_AUTONOMOUS_BINARY = binary;
function run(args, options = {}) {
  const result = spawnSync(process.execPath, [launcher, '--path', project, ...args], { env: { ...env, SPEC_AUTONOMOUS_TEST_BINARY: binary }, encoding: 'utf8', timeout: 30000, ...options });
  if (result.error) throw result.error;
  if (result.status !== 0) throw new Error(`${args.join(' ')} failed (${result.status})\n${result.stderr}\n${result.stdout}`);
  return result.stdout.trim() ? JSON.parse(result.stdout) : {};
}
try {
  // Empty-project initialization path, using OpenSpec's native initializer.
  run(['init', '--provider', 'openspec', '--agent', 'codex', '--mcp', '--non-interactive', '--json']);
  const git = spawnSync('git', ['-C', project, 'add', '--all']);
  if (git.status !== 0) throw new Error(git.stderr);
  const commit = spawnSync('git', ['-C', project, '-c', 'user.name=Spec Autonomous Dogfood', '-c', 'user.email=dogfood@example.invalid', 'commit', '-qm', 'dogfood: initialize OpenSpec project']);
  if (commit.status !== 0) throw new Error(commit.stderr);
  const config = join(project, '.spec-autonomous/config.toml');
  const configText = await readFile(config, 'utf8');
  const openspecCommand = `[${JSON.stringify(process.execPath)}, ${JSON.stringify(join(root, 'node_modules/@fission-ai/openspec/bin/openspec.js'))}]`;
  await writeFile(config, configText.replace(/openspec_command\s*=\s*\[[^\n]*\]/, `openspec_command = ${openspecCommand}`));
  const configCommit = spawnSync('git', ['-C', project, 'add', '.spec-autonomous/config.toml']);
  if (configCommit.status !== 0) throw new Error(configCommit.stderr);
  const configCommitResult = spawnSync('git', ['-C', project, '-c', 'user.name=Spec Autonomous Dogfood', '-c', 'user.email=dogfood@example.invalid', 'commit', '-qm', 'dogfood: configure OpenSpec command']);
  if (configCommitResult.status !== 0) throw new Error(configCommitResult.stderr);
  const host = spawnSync(process.execPath, [join(root, 'tests/mock-host.mjs'), '--path', project, 'prepare', '--change', 'todo-list', '--mode', 'autonomous'], { env: { ...env, SPEC_AUTONOMOUS_TEST_BINARY: binary }, encoding: 'utf8', timeout: 120000, maxBuffer: 16 * 1024 * 1024 });
  if (host.error || host.status !== 0) throw new Error(`mock host failed\n${host.stderr}\n${host.stdout}`);
  const lines = host.stdout.trim().split('\n').filter(Boolean).map(line => { try { return JSON.parse(line); } catch { return null; } }).filter(Boolean);
  const terminal = lines.at(-1)?.data ?? lines.at(-1);
  const runId = terminal.id;
  const todo = join(project, 'openspec/changes/todo-list');
  const progress = run(['progress', '--all-worktrees', '--json']);
  const cleanup = run(['tools', 'call', 'run.cleanup', '--input', JSON.stringify({ run_id: runId, delete_branches: false, delete_integration: false }), '--json']);
  const report = { project, run_id: runId, status: terminal.status, initialized: existsSync(join(project, '.spec-autonomous/config.toml')), framework: 'openspec', worktrees: progress.data.worktrees?.length ?? 0, cleanup_plan_hash: cleanup.data.plan_hash, native_change: existsSync(todo) };
  await mkdir(join(root, '.artifacts'), { recursive: true });
  await writeFile(join(root, '.artifacts/dogfood-report.json'), JSON.stringify(report, null, 2) + '\n');
  console.log(JSON.stringify(report, null, 2));
} finally { await rm(temp, { recursive: true, force: true }); }
