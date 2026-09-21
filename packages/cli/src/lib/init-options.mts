import { existsSync } from 'node:fs';
import { join } from 'node:path';
import { detectLocale, message, type Locale } from './locale.mts';
import type { DetectionReport } from './provider-cli.mts';

export interface SelectChoice<T> {
  value: T;
  name?: string;
}

export interface SelectConfig<T> {
  message: string;
  choices: readonly SelectChoice<T>[];
  default?: T;
  // Inquirer calls help renderers with its own arguments; accept and ignore them.
  theme?: { style?: { keysHelpTip?: (...args: unknown[]) => string } };
}

// Tests drive these prompts with plain `{ isTTY }` stubs, so only the terminal
// flag is required; the real streams still satisfy the shape.
export interface PromptIo {
  input?: { isTTY?: boolean } & Partial<NodeJS.ReadStream>;
  output?: { isTTY?: boolean } & Partial<NodeJS.WriteStream>;
}

export type SelectPrompt = (config: SelectConfig<any>, io?: PromptIo) => Promise<any>;

export interface InitOptions {
  lang?: Locale;
  json?: boolean;
  format?: string;
  interactive?: boolean;
  nonInteractive?: boolean;
  yes?: boolean;
  provider?: string;
  framework?: string;
  agent?: string;
  mcp?: boolean;
  path?: string;
  [key: string]: unknown;
}

export interface ResolveInitIo extends PromptIo {
  detection: () => Promise<DetectionReport>;
  env?: NodeJS.ProcessEnv;
  select?: SelectPrompt;
}

export async function selectInitOption(config: SelectConfig<any>, { input = process.stdin, output = process.stderr }: PromptIo = {}): Promise<any> {
  const { select } = await import('@inquirer/prompts');
  const stream = input as NodeJS.ReadStream;
  const controller = new AbortController();
  const end = () => { controller.abort(); };
  const keypress = (_: unknown, key: { ctrl?: boolean; name?: string } | undefined) => { if (key?.ctrl && key.name === 'd') end(); };
  stream.once('end', end);
  stream.on('keypress', keypress);
  try {
    if (stream.readableEnded) end();
    return await select(config as never, { input: stream, output: output as NodeJS.WriteStream, signal: controller.signal });
  } finally {
    stream.off('end', end);
    stream.off('keypress', keypress);
  }
}

// All questions are answered before any tool installation or project mutation.
export async function resolveInitOptions(options: InitOptions, { detection, env = process.env,
  input = process.stdin, output = process.stderr, select = selectInitOption }: ResolveInitIo): Promise<InitOptions> {
  const locale = options.lang || detectLocale();
  const structured = options.json || options.format && options.format !== 'human';
  const terminal = Boolean(input.isTTY && output.isTTY);
  if (options.interactive && (structured || options.nonInteractive || options.yes)) throw new Error('init_mode_conflict: interactive conflicts with structured, non-interactive or yes');
  if (options.interactive && !terminal) throw new Error('init_terminal_required: interactive init needs a terminal');
  const interactive = !structured && !options.nonInteractive && !options.yes && terminal && (options.interactive || !env.CI || env.CI === 'false' || env.CI === '0');
  const report = await detection();
  let provider: string | undefined = options.provider || (options.framework !== 'auto' ? options.framework : undefined) || report.selected || undefined;
  let agent = options.agent;
  let prompted = Boolean(options.interactive);
  const ask = async (key: string, choices: readonly SelectChoice<any>[], defaultValue: unknown) => {
    prompted = true;
    return select({ message: message(key, locale), choices, default: defaultValue,
      theme: { style: { keysHelpTip: () => message('init.keys', locale) } } }, { input, output });
  };
  if (!provider) {
    if (interactive) provider = await ask('init.choose_provider',
      (report.detected.length ? report.detected.map(d => d.framework) : ['openspec', 'speckit'])
        .map(value => ({ value, name: value === 'openspec' ? 'OpenSpec' : 'Spec Kit' })), report.selected || 'openspec');
    else if (options.yes && !report.detected.length && !report.warnings?.length) provider = 'openspec';
    else throw new Error('provider_selection_required: choose openspec or speckit');
  }
  if (!provider || !['openspec', 'speckit'].includes(provider)) throw new Error('provider_selection_required: choose openspec or speckit');
  if (!report.detected.some(d => d.framework === provider) && (report.detected.length || report.warnings?.length)) {
    throw new Error('provider_init_conflict: preserve existing or incomplete native setup');
  }
  if (!agent) {
    const codex = existsSync(join(report.root, '.agents')), claude = existsSync(join(report.root, '.claude'));
    if (codex !== claude) agent = codex ? 'codex' : 'claude';
    else if (interactive) agent = await ask('init.choose_agent', [
      { value: 'codex', name: 'Codex' }, { value: 'claude', name: 'Claude Code' },
    ], 'codex');
    else if (options.yes) agent = 'codex';
    else throw new Error('host_selection_required: choose --agent codex|claude');
  }
  let mcp = options.mcp;
  if (mcp === undefined) {
    if (interactive && prompted) mcp = await ask('init.choose_mcp', [
      { value: true, name: message('init.mcp_yes', locale) },
      { value: false, name: message('init.mcp_no', locale) },
    ], true);
    else mcp = Boolean(options.yes);
  }
  return { ...options, path: report.root, framework: provider, provider, agent, mcp };
}
