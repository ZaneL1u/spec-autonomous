import { mkdtemp, mkdir, readdir, lstat, readFile, copyFile, rm, chmod } from 'node:fs/promises';
import { existsSync } from 'node:fs';
import { constants } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { createProviderManager, providerName, providerNames } from './providers.mjs';
import { detectLocale } from './locale.mjs';
import { nodeCommand } from './provider-process.mjs';

export function bridgeEnvironment(env = process.env) {
  return { ...env, SPEC_AUTONOMOUS_OPENSPEC_BRIDGE: JSON.stringify([
    nodeCommand(env), fileURLToPath(new URL('../bin/provider-bridge.mjs', import.meta.url)),
  ]) };
}
export function createProviderContext(binary, options = {}, manager = createProviderManager({ env: { ...process.env, SPEC_AUTONOMOUS_LANG: detectLocale({ explicit: options.lang }) } })) {
  const path = resolve(options.path || process.cwd());
  const { provider, framework } = options;
  if (provider && framework && framework !== 'auto' && provider !== framework) throw new Error('provider_selection_conflict: --provider and --framework disagree');
  const explicit = provider || framework;
  if (explicit && explicit !== 'auto') providerName(explicit);
  async function detection() {
    const result = await manager.run([binary, '--path', path, 'detect', '--json'], { env: manager.env, timeout: 15_000 });
    if (result.code !== 0) throw new Error(`provider_detection_failed: ${result.stderr || result.stdout}`);
    return JSON.parse(result.stdout);
  }
  async function select(requested, allowMissing = false) {
    const report = await detection();
    const selected = (requested !== 'auto' ? requested : undefined) || (explicit !== 'auto' ? explicit : undefined) || report.selected;
    providerName(selected);
    if (!allowMissing && !report.detected.some(d => d.framework === selected)) throw new Error('provider_not_initialized: run init --provider openspec|speckit --agent codex|claude');
    return { provider: selected, root: report.root, report };
  }
  async function ensureSelected(requested, options = {}) {
    const selection = await select(requested, options.allowMissing);
    return manager.ensure(selection.provider, { root: selection.root, managed: options.managed,
      command: selection.report.provider_commands?.[selection.provider] });
  }
  async function ensureSource(source = {}) {
    let requested = source.framework;
    const runId = source.run_id || source.result?.run_id;
    // Continuation uses the ledger's provider even if another SDD framework was
    // added later. Ask the core for this fact instead of parsing SQLite in JS.
    if (runId) {
      const status = await manager.run([binary, '--path', path, 'status', runId, '--json'], { env: manager.env, timeout: 15_000 });
      if (status.code !== 0) throw new Error(`provider_source_failed: ${status.stderr || status.stdout}`);
      const original = JSON.parse(status.stdout).data.milestone.framework;
      if (requested && requested !== 'auto' && requested !== original) throw new Error('provider_selection_conflict: preserve the run provider');
      requested = original;
    }
    return ensureSelected(requested);
  }
  return { binary, args: options.nativeArgs || [], options, manager, path, explicit, detection, select, ensureSelected, ensureSource };
}

export function nativeAction(parsed) {
  let action = parsed.command;
  while (action.arguments?.command) action = action.arguments.command;
  return action;
}
export async function cliSource(parsed) {
  const args = nativeAction(parsed).arguments || {};
  const input = args.input;
  if (input) {
    const source = JSON.parse(input.startsWith('@') ? await readFile(input.slice(1), 'utf8') : input);
    // Native CLI globals override source JSON. Receipt operations instead use
    // the run's ledger provider and do not accept a source-framework override.
    if (parsed.framework && !source.result) source.framework = parsed.framework;
    return source;
  }
  const result = args.result;
  if (result) return { result: JSON.parse(await readFile(result, 'utf8')) };
  return { run_id: args.run_id, framework: parsed.framework };
}

// Native initialization runs in an empty staging repository. Check every target
// before the first write; never use the upstream force/overwrite switches.
export async function mergeScaffold(source, target) {
  const entries = [];
  async function scan(directory, relative = '') {
    for (const entry of await readdir(directory, { withFileTypes: true })) {
      if (entry.name === '.git') continue;
      const local = join(relative, entry.name), from = join(source, local), to = join(target, local);
      const src = await lstat(from), dst = await lstat(to).catch(e => { if (e.code !== 'ENOENT') throw e; return null; });
      if (src.isSymbolicLink() || dst?.isSymbolicLink()) throw new Error(`provider_init_conflict: symlink ${local}`);
      if (src.isDirectory()) {
        if (dst && !dst.isDirectory()) throw new Error(`provider_init_conflict: ${local}`);
        entries.push({ from, to, directory: true, exists: Boolean(dst) });
        await scan(from, local);
      } else if (src.isFile()) {
        if (dst && (!dst.isFile() || !Buffer.from(await readFile(from)).equals(await readFile(to)))) throw new Error(`provider_init_conflict: ${local}`);
        entries.push({ from, to, mode: src.mode, exists: Boolean(dst) });
      } else throw new Error(`provider_init_conflict: unsupported file ${local}`);
    }
  }
  await scan(source);
  const created = [];
  try {
    for (const entry of entries) {
      if (entry.exists) continue;
      if (entry.directory) await mkdir(entry.to);
      else { await copyFile(entry.from, entry.to, constants.COPYFILE_EXCL); await chmod(entry.to, entry.mode); }
      created.push(entry);
    }
  } catch (error) {
    // Only roll back files whose exact contents remain ours. Never recursively
    // remove a generated directory that another process may have written into.
    for (const entry of created.reverse()) {
      if (entry.directory) { const { rmdir } = await import('node:fs/promises'); await rmdir(entry.to).catch(() => {}); }
      else if (Buffer.from(await readFile(entry.to)).equals(await readFile(entry.from))) await rm(entry.to);
    }
    throw error;
  }
  return created.map(e => e.to);
}

export async function initializeProvider(context) {
  const { manager, options } = context;
  const selection = await context.select(undefined, true);
  const hasProvider = selection.report.detected.some(d => d.framework === selection.provider);
  let agent = options.agent;
  if (!agent) {
    const codex = existsSync(join(selection.root, '.agents')), claude = existsSync(join(selection.root, '.claude'));
    if (codex !== claude) agent = codex ? 'codex' : 'claude';
  }
  if (!['codex', 'claude'].includes(agent)) throw new Error('host_selection_required: choose --agent codex|claude');
  if (!hasProvider && (selection.report.detected.length || selection.report.warnings.length)) throw new Error('provider_init_conflict: preserve existing or incomplete SDD setup; initialize the desired native framework explicitly');
  const ready = await manager.ensure(selection.provider, { root: selection.root, command: selection.report.provider_commands?.[selection.provider] });
  const ensureGit = async () => {
    const env = { ...manager.env };
    for (const key of ['GIT_DIR', 'GIT_WORK_TREE', 'GIT_COMMON_DIR', 'GIT_INDEX_FILE']) delete env[key];
    const existing = await manager.run(['git', 'rev-parse', '--git-dir'], { cwd: selection.root, env });
    if (existing.code === 0) return;
    if (existsSync(join(selection.root, '.git'))) throw new Error('init_git_failed: existing Git metadata is unusable');
    const initialized = await manager.run(['git', 'init', '-q'], { cwd: selection.root, env });
    if (initialized.code !== 0) throw new Error('init_git_failed: cannot initialize Git repository');
  };
  if (hasProvider) { await ensureGit(); return ready; }
  const stage = await mkdtemp(join(tmpdir(), 'spec-autonomous-native-init-'));
  try {
    const initEnv = { ...manager.env };
    for (const key of ['GIT_DIR', 'GIT_WORK_TREE', 'GIT_COMMON_DIR', 'GIT_INDEX_FILE', 'SPECIFY_INIT_DIR', 'SPECIFY_FEATURE_DIRECTORY', 'SPECIFY_FEATURE']) delete initEnv[key];
    // Keep native root discovery and Git side effects inside the staging folder.
    if (selection.provider === 'openspec') {
      const git = await manager.run(['git', 'init', '-q', stage], { env: initEnv });
      if (git.code !== 0) throw new Error('provider_init_failed: Git is required to initialize the native framework');
    }
    const argv = selection.provider === 'openspec' ? ['init', '--tools', agent]
      : ['init', '--here', '--integration', agent, '--script', process.platform === 'win32' ? 'ps' : 'sh', '--ignore-agent-tools', '--non-interactive'];
    const result = await manager.run([...ready.command, ...argv], { cwd: stage, env: initEnv, output: 'log' });
    if (result.code !== 0) throw new Error(`provider_init_failed: native initializer exited ${result.code}`);
    await mergeScaffold(stage, selection.root);
    await ensureGit();
    return ready;
  } finally { await rm(stage, { recursive: true, force: true }); }
}

export async function providerOperation(context, { operation = 'status', provider, managed = false } = {}) {
  if (!['status', 'ensure'].includes(operation) || typeof managed !== 'boolean') throw new Error('invalid_arguments: provider operation must be status or ensure');
  if (provider !== undefined) providerName(provider);
  if (operation === 'ensure') return context.ensureSelected(provider, { allowMissing: true, managed });
  const report = await context.detection();
  return { root: report.root, selected: provider || report.selected, detected: report.detected, home: context.manager.home,
    providers: await Promise.all((provider ? [provider] : providerNames).map(name => context.manager.status(name, { root: report.root, managed, command: report.provider_commands?.[name] }))) };
}

export const providerTool = {
  name: 'sa_providers', description: 'Inspect native OpenSpec / Spec Kit CLI readiness, or install missing selected tools and runtime prerequisites in a user-owned directory.',
  inputSchema: { type: 'object', additionalProperties: false, properties: {
    operation: { type: 'string', enum: ['status', 'ensure'], default: 'status' },
    provider: { type: 'string', enum: providerNames }, managed: { type: 'boolean', default: false },
  } }, annotations: { readOnlyHint: false, destructiveHint: false, openWorldHint: true },
};

// These operations can ask the core to read or modify native OpenSpec artifacts.
// Unknown tools and passive state/progress operations are forwarded unchanged.
const nativeCapabilities = new Set(['inspect', 'prepare', 'apply-result', 'archive', 'plan', 'resume', 'milestone.new', 'run.revise',
  'task.complete', 'native.instructions', 'native.create', 'roadmap.import', 'verify.source', 'verify.plan', 'task.list', 'task.ready']);
export function needsProvider(command, capability) {
  if (nativeCapabilities.has(command)) return true;
  return command === 'milestone' || command === 'tools' && nativeCapabilities.has(capability);
}
export function mcpNeedsProvider(message) {
  if (message.method !== 'tools/call') return false;
  const { name, arguments: args = {} } = message.params || {};
  if (name === 'sa_tools') return args.operation === 'call' && nativeCapabilities.has(args.capability);
  return typeof name === 'string' && [...nativeCapabilities].some(id => `sa_${id.replace(/[.-]/g, '_')}` === name);
}
