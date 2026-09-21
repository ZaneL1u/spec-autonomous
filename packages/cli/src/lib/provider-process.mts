import { spawn } from 'node:child_process';
import { existsSync, realpathSync } from 'node:fs';
import { delimiter, dirname, join, basename } from 'node:path';
import { errorCode } from './errors.mts';

export interface ProcessResult {
  code: number;
  stdout: string;
  stderr: string;
}

export interface RunOptions {
  cwd?: string;
  env?: NodeJS.ProcessEnv;
  timeout?: number;
  output?: 'capture' | 'log';
  onChild?: (pid: number | null) => void;
}

export type RunProcess = (argv: string[], options?: RunOptions) => Promise<ProcessResult>;

export function findExecutable(name: string, env: NodeJS.ProcessEnv = process.env): string | null {
  const path = env.PATH ?? env.Path ?? '';
  for (const directory of path.split(delimiter).filter(Boolean)) {
    for (const suffix of process.platform === 'win32' ? ['.exe', '.cmd', ''] : ['']) {
      const file = join(directory, name + suffix);
      if (existsSync(file)) return file;
    }
  }
  return null;
}

export function nodeCommand(env: NodeJS.ProcessEnv = process.env): string {
  if (!process.versions.bun) return process.execPath;
  const node = findExecutable('node', env);
  if (!node) throw new Error('runtime_missing: OpenSpec requires Node.js 22+ on PATH');
  return node;
}

export function npmArgv(env: NodeJS.ProcessEnv = process.env): string[] {
  const node = nodeCommand(env);
  const candidates = [
    ...(basename(env.npm_execpath ?? '') === 'npm-cli.js' ? [env.npm_execpath ?? ''] : []),
    join(dirname(node), 'node_modules/npm/bin/npm-cli.js'),
    join(dirname(node), '../lib/node_modules/npm/bin/npm-cli.js'),
  ];
  const npm = findExecutable('npm', env);
  if (npm && !npm.endsWith('.cmd')) candidates.push(realpathSync(npm));
  const entry = candidates.find(existsSync);
  if (!entry) throw new Error('runtime_missing: use a Node.js installation with npm, or run with Bun');
  return [node, entry];
}

// No shell interpolation. Each installation has a finite deadline and owns its
// subprocess group; cancellation never targets host-owned agent processes.
export function runProcess(argv: string[], { cwd, env = process.env, timeout = 300_000, output = 'capture', onChild = () => {} }: RunOptions = {}): Promise<ProcessResult> {
  return new Promise((resolve, reject) => {
    const clean: NodeJS.ProcessEnv = { ...env };
    for (const key of ['NODE_TEST_CONTEXT', 'NODE_TEST_WORKER_ID', 'NODE_CHANNEL_FD', 'NODE_CHANNEL_SERIALIZATION_MODE', 'NODE_UNIQUE_ID']) delete clean[key];
    const child = spawn(argv[0], argv.slice(1), { cwd, env: clean, shell: false, detached: process.platform !== 'win32', stdio: ['ignore', 'pipe', 'pipe'] });
    let stdout = '', stderr = '';
    let failure: Error | undefined, escalation: NodeJS.Timeout | undefined;
    const stop = (signal: NodeJS.Signals) => {
      try {
        if (process.platform === 'win32' || child.pid === undefined) child.kill(signal);
        else process.kill(-child.pid, signal);
      } catch (error) { if (errorCode(error) !== 'ESRCH') failure ??= error as Error; }
    };
    const cancel = (signal: NodeJS.Signals) => {
      failure ??= new Error(`provider_cancelled: ${signal}`);
      stop(signal);
      escalation ??= setTimeout(() => stop('SIGKILL'), 1000);
    };
    const interrupt = () => cancel('SIGINT'), terminate = () => cancel('SIGTERM');
    process.on('SIGINT', interrupt); process.on('SIGTERM', terminate);
    const timer = setTimeout(() => { failure = new Error('provider_timeout: command exceeded its deadline'); cancel('SIGTERM'); }, timeout);
    const collect = (stream: 'stdout' | 'stderr', chunk: Buffer) => {
      if (output === 'log') process.stderr.write(chunk);
      if (stream === 'stdout') stdout += chunk; else stderr += chunk;
      if (stdout.length + stderr.length > 4 * 1024 * 1024) {
        failure = new Error('provider_output_too_large'); cancel('SIGTERM');
      }
    };
    child.stdout!.on('data', (b: Buffer) => collect('stdout', b)); child.stderr!.on('data', (b: Buffer) => collect('stderr', b));
    child.once('spawn', () => { try { onChild(child.pid ?? null); } catch (e) { failure = e as Error; cancel('SIGTERM'); } });
    child.once('error', error => { failure = error; });
    child.once('close', (code, signal) => {
      clearTimeout(timer); clearTimeout(escalation);
      process.off('SIGINT', interrupt); process.off('SIGTERM', terminate);
      try { onChild(null); } catch (e) { failure ??= e as Error; }
      if (failure) reject(failure);
      else resolve({ code: code ?? (signal === 'SIGINT' ? 130 : signal === 'SIGTERM' ? 143 : 1), stdout, stderr });
    });
  });
}
