import { message } from './locale.mts';
import { mkdir, readFile, writeFile, rename, rm, stat, chmod } from 'node:fs/promises';
import { existsSync, realpathSync } from 'node:fs';
import { createHash, randomUUID } from 'node:crypto';
import { homedir } from 'node:os';
import { dirname, join, resolve } from 'node:path';
import { setTimeout as delay } from 'node:timers/promises';
import { platformFor, type Platform } from './platform.mts';
import { runProcess, findExecutable, nodeCommand, npmArgv, type ProcessResult, type RunOptions } from './provider-process.mts';
import { versions, uvHashes } from './provider-versions.mts';
import { errorCode, errorMessage } from './errors.mts';

export type ProviderName = 'openspec' | 'speckit';
export type ProviderSource = 'project' | 'path' | 'configured' | 'managed' | 'missing';

export interface ProbeResult {
  ready: boolean;
  version?: string;
  reason?: string;
}

export interface ProviderStatus extends ProbeResult {
  provider: ProviderName;
  source: ProviderSource;
  command?: string[];
}

export interface StatusOptions {
  root?: string;
  managed?: boolean;
  command?: string[];
}

export interface ProviderManager {
  home: string;
  env: NodeJS.ProcessEnv;
  run: (argv: string[], options?: RunOptions) => Promise<ProcessResult>;
  status: (name: ProviderName, options?: StatusOptions) => Promise<ProviderStatus>;
  ensure: (name: ProviderName, options?: StatusOptions) => Promise<ProviderStatus>;
  managed: (name: ProviderName) => Promise<ProviderStatus | null>;
}

export interface ProviderManagerOptions {
  env?: NodeJS.ProcessEnv;
  home?: string;
  run?: (argv: string[], options?: RunOptions) => Promise<ProcessResult>;
  find?: (name: string) => string | null;
  platform?: () => Platform;
  download?: typeof downloadVerified;
  log?: (text: string) => void;
}

interface LockOwner {
  pid: number;
  child: number | null;
}

export const providerNames: ProviderName[] = ['openspec', 'speckit'];
export function providerName(value: unknown): ProviderName {
  if (!providerNames.includes(value as ProviderName)) throw new Error('provider_selection_required: choose openspec or speckit');
  return value as ProviderName;
}
export function providerHome(env: NodeJS.ProcessEnv = process.env): string {
  const base = process.platform === 'win32' ? env.LOCALAPPDATA || join(homedir(), 'AppData/Local')
    : process.platform === 'darwin' ? join(homedir(), 'Library/Application Support') : env.XDG_DATA_HOME || join(homedir(), '.local/share');
  return resolve(env.SPEC_AUTONOMOUS_PROVIDER_HOME || join(base, 'spec-autonomous/providers'));
}
function alive(pid: number | null | undefined): boolean {
  if (!Number.isSafeInteger(pid) || (pid as number) < 1) return false;
  try { process.kill(pid as number, 0); return true; } catch (e) { return errorCode(e) !== 'ESRCH'; }
}
export async function withInstallLock<T>(directory: string, action: (onChild: (pid: number | null) => void) => Promise<T>, { timeout = 310_000 }: { timeout?: number } = {}): Promise<T> {
  await mkdir(dirname(directory), { recursive: true });
  const started = Date.now();
  while (true) {
    try { await mkdir(directory); break; }
    catch (error) {
      if (errorCode(error) !== 'EEXIST') throw error;
      let owner: LockOwner | undefined;
      try { owner = JSON.parse(await readFile(join(directory, 'owner.json'), 'utf8')) as LockOwner; } catch {}
      const observed = await stat(directory).catch(() => null);
      const age = Date.now() - (observed?.mtimeMs ?? Date.now());
      if (owner && !alive(owner.pid) && !alive(owner.child) || !owner && age > 30_000) {
        // A reaper inside this generation serializes stale-owner checks. Always
        // re-read after acquisition: the earlier observation may be obsolete.
        const reaper = join(directory, 'reaping');
        try {
          await mkdir(reaper);
          await writeFile(join(reaper, 'owner.json'), JSON.stringify({ pid: process.pid }));
          let latest: LockOwner | undefined;
          try { latest = JSON.parse(await readFile(join(directory, 'owner.json'), 'utf8')) as LockOwner; } catch {}
          const current = await stat(directory);
          if (latest && !alive(latest.pid) && !alive(latest.child) || !latest && age > 30_000 && current.ino === observed?.ino && current.dev === observed?.dev) await rm(directory, { recursive: true, force: true });
          else await rm(reaper, { recursive: true, force: true });
        } catch (e) {
          const code = errorCode(e);
          if (!['ENOENT', 'EEXIST'].includes(code ?? '')) throw e;
          if (code === 'EEXIST') {
            let reaperOwner: { pid: number } | undefined;
            try { reaperOwner = JSON.parse(await readFile(join(reaper, 'owner.json'), 'utf8')) as { pid: number }; } catch {}
            const reaperAge = Date.now() - (await stat(reaper).catch(() => ({ mtimeMs: Date.now() }))).mtimeMs;
            if (reaperOwner && !alive(reaperOwner.pid) || !reaperOwner && reaperAge > 30_000) await rm(reaper, { recursive: true, force: true });
          }
        }
        if (Date.now() - started >= timeout) throw new Error('provider_busy: lock recovery is still in progress; retry later');
        await delay(100);
        continue;
      }
      if (Date.now() - started >= timeout) throw new Error('provider_busy: another installer still owns the lock; retry later');
      await delay(100);
    }
  }
  const ownerFile = join(directory, 'owner.json');
  const owner: LockOwner = { pid: process.pid, child: null };
  // Synchronous child bookkeeping closes the spawn-to-lock-update gap within
  // this process. An orphan installer keeps its generation unavailable.
  const { writeFileSync, renameSync } = await import('node:fs');
  const onChild = (pid: number | null) => {
    owner.child = pid;
    writeFileSync(`${ownerFile}.tmp`, JSON.stringify(owner));
    renameSync(`${ownerFile}.tmp`, ownerFile);
  };
  try { onChild(null); return await action(onChild); }
  finally { await rm(directory, { recursive: true, force: true }); }
}

export async function downloadVerified(url: string, destination: string, expected: string, fetcher: typeof fetch = fetch): Promise<void> {
  const response = await fetcher(url, { signal: AbortSignal.timeout(120_000) });
  if (!response.ok) throw new Error(`provider_download_failed: HTTP ${response.status}`);
  const chunks: Uint8Array[] = []; let size = 0;
  for await (const chunk of response.body!) {
    size += chunk.length;
    if (size > 128 * 1024 * 1024) throw new Error('provider_download_too_large');
    chunks.push(chunk);
  }
  const bytes = Buffer.concat(chunks);
  if (createHash('sha256').update(bytes).digest('hex') !== expected) throw new Error('provider_integrity_mismatch: uv archive SHA256');
  await writeFile(destination, bytes, { flag: 'wx' });
}

export function createProviderManager({ env = process.env, home = providerHome(env), run = runProcess,
  find = name => findExecutable(name, env), platform = platformFor, download = downloadVerified, log = text => { process.stderr.write(`[providers] ${text}\n`); } }: ProviderManagerOptions = {}): ProviderManager {
  const childEnv: NodeJS.ProcessEnv = { ...env, OPENSPEC_TELEMETRY: '0', DO_NOT_TRACK: '1', UV_NO_PROGRESS: '1' };
  const commandFor = (name: ProviderName, dir: string): string[] => name === 'openspec'
    ? [nodeCommand(env), join(dir, 'node_modules/@fission-ai/openspec/bin/openspec.js')]
    : [join(dir, 'bin', process.platform === 'win32' ? 'specify.exe' : 'specify')];
  async function probe(name: ProviderName, command: string[], root?: string): Promise<ProbeResult> {
    try {
      const result = await run([...command, name === 'speckit' ? 'version' : '--version'], { cwd: root, env: childEnv, timeout: 15_000 });
      const match = `${result.stdout}\n${result.stderr}`.match(/\b(\d+\.\d+\.\d+(?:[-+.][\w.]+)?)\b/);
      return result.code === 0 && match ? { ready: true, version: match[1] } : { ready: false, reason: 'version_probe_failed' };
    } catch (error) {
      if (errorMessage(error).startsWith('provider_cancelled:')) throw error;
      return { ready: false, reason: errorMessage(error) };
    }
  }
  function external(name: ProviderName, root: string): { command: string[]; source: ProviderSource } | null {
    if (name === 'openspec') {
      const local = join(root, 'node_modules/@fission-ai/openspec/bin/openspec.js');
      if (existsSync(local)) return { command: [nodeCommand(env), local], source: 'project' };
    }
    const executable = find(name === 'speckit' ? 'specify' : 'openspec');
    if (!executable) return null;
    if (executable.endsWith('.cmd')) {
      const entry = join(dirname(executable), 'node_modules/@fission-ai/openspec/bin/openspec.js');
      if (name === 'openspec' && existsSync(entry)) return { command: [nodeCommand(env), entry], source: 'path' };
      throw new Error('provider_command_unsupported: use a native executable or --managed for an isolated installation');
    }
    const file = realpathSync(executable);
    return { command: name === 'openspec' && /\.[cm]?js$/.test(file) ? [nodeCommand(env), file] : [executable], source: 'path' };
  }
  async function managed(name: ProviderName): Promise<ProviderStatus | null> {
    try {
      const receipt = JSON.parse(await readFile(join(home, name, 'current.json'), 'utf8')) as { version?: string; generation?: string };
      if (receipt.version !== versions[name] || !/^[a-f\d-]{36}$/.test(receipt.generation ?? '')) return null;
      const command = commandFor(name, join(home, name, receipt.generation!));
      if (!existsSync(command.at(-1)!)) return null;
      const health = await probe(name, command);
      if (!health.ready || health.version !== versions[name]) return null;
      return { provider: name, source: 'managed', command, ...health };
    } catch (error) { if (['ENOENT', 'ENOTDIR'].includes(errorCode(error) ?? '') || error instanceof SyntaxError) return null; throw error; }
  }
  async function status(name: ProviderName, { root = process.cwd(), managed: forceManaged = false, command }: StatusOptions = {}): Promise<ProviderStatus> {
    providerName(name);
    if (!forceManaged) {
      const existing = command ? { command, source: 'configured' as const } : external(name, resolve(root));
      if (existing) return { provider: name, ...existing, ...await probe(name, existing.command, root) };
    }
    return await managed(name) || { provider: name, ready: false, source: 'missing', version: versions[name] };
  }
  async function checked(argv: string[], options: RunOptions = {}): Promise<ProcessResult> {
    const result = await run(argv, { env: childEnv, output: 'log', ...options });
    if (result.code !== 0) throw new Error(`provider_install_failed: command exited ${result.code}; see stderr for installer diagnostics`);
    return result;
  }
  async function installUv(onChild: (pid: number | null) => void): Promise<string> {
    const p = platform();
    const directory = join(home, `uv-${versions.uv}-${p.key}`);
    const executable = join(directory, process.platform === 'win32' ? 'uv.exe' : 'uv');
    if (existsSync(executable)) {
      const check = await run([executable, '--version'], { env: childEnv, timeout: 15_000 });
      if (check.code === 0 && check.stdout.includes(`uv ${versions.uv}`)) return executable;
      throw new Error('provider_uv_invalid: cached uv failed validation');
    }
    const stage = join(home, `uv-download-${randomUUID()}`);
    await mkdir(stage, { recursive: true });
    try {
      const stem = `uv-${p.target}`;
      const asset = `${stem}.${p.os === 'win32' ? 'zip' : 'tar.gz'}`;
      const archive = join(stage, asset);
      await download(`https://github.com/astral-sh/uv/releases/download/${versions.uv}/${asset}`, archive, uvHashes[p.key]);
      await checked(['tar', '-xf', archive, '-C', stage], { cwd: stage, onChild });
      const extracted = p.os === 'win32' ? join(stage, 'uv.exe') : join(stage, stem, 'uv');
      await chmod(extracted, 0o755);
      const check = await run([extracted, '--version'], { env: childEnv, timeout: 15_000 });
      if (check.code !== 0 || !check.stdout.includes(`uv ${versions.uv}`)) throw new Error('provider_uv_invalid: downloaded executable failed validation');
      await mkdir(directory, { recursive: true });
      await rename(extracted, executable);
      return executable;
    } finally { await rm(stage, { recursive: true, force: true }); }
  }
  async function ensure(name: ProviderName, options: StatusOptions = {}): Promise<ProviderStatus> {
    providerName(name);
    const before = await status(name, options);
    if (before.ready) return before;
    if (before.source !== 'missing') throw new Error(`provider_unusable: ${name} at ${before.command?.join(' ')}; ${before.reason}. Use providers ensure ${name} --managed to install an isolated version.`);
    if (env.SPEC_AUTONOMOUS_OFFLINE === '1') throw new Error(`provider_offline: ${name} is missing; connect once and run providers ensure ${name}`);
    return withInstallLock(join(home, '.install-lock'), async onChild => {
      const current = await status(name, options);
      if (current.ready) return current;
      if (current.source !== 'missing') throw new Error(`provider_unusable: ${name} changed while waiting for installation`);
      const generation = randomUUID(), directory = join(home, name, generation);
      await mkdir(directory, { recursive: true });
      log(message('log.install', undefined, { provider: name, version: versions[name], directory }));
      if (name === 'openspec') {
        await writeFile(join(directory, 'package.json'), '{"private":true}\n');
        const argv = process.versions.bun
          ? [process.execPath, 'add', '--ignore-scripts', '--exact', `@fission-ai/openspec@${versions.openspec}`]
          : [...npmArgv(env), 'install', '--ignore-scripts', '--no-audit', '--no-fund', '--save-exact', `@fission-ai/openspec@${versions.openspec}`];
        await checked(argv, { cwd: directory, onChild });
      } else {
        let uv = find('uv');
        if (uv) {
          const check = await run([uv, '--version'], { env: childEnv, timeout: 15_000 });
          if (check.code !== 0) throw new Error('provider_uv_invalid: existing uv failed validation');
        } else uv = await installUv(onChild);
        await checked([uv, 'tool', 'install', '--python', '>=3.11', `specify-cli==${versions.speckit}`], {
          cwd: directory, onChild, env: { ...childEnv, UV_TOOL_DIR: join(directory, 'tools'), UV_TOOL_BIN_DIR: join(directory, 'bin'),
            UV_PYTHON_INSTALL_DIR: join(home, 'python'), UV_CACHE_DIR: join(home, 'uv-cache'), UV_PYTHON_DOWNLOADS: 'automatic', UV_NO_MODIFY_PATH: '1' },
        });
      }
      const command = commandFor(name, directory), health = await probe(name, command);
      if (!health.ready || health.version !== versions[name]) throw new Error(`provider_validation_failed: ${name} ${health.reason ?? health.version}`);
      const receipt = join(home, name, 'current.json'), pending = `${receipt}.${generation}.tmp`;
      await writeFile(pending, JSON.stringify({ schema_version: 1, provider: name, version: versions[name], generation, verified_at: new Date().toISOString() }) + '\n');
      await rename(pending, receipt);
      return { provider: name, ready: true, source: 'managed', command, version: health.version };
    });
  }
  return { home, status, ensure, managed, run, env: childEnv };
}
