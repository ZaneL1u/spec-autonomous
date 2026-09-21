import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { spawnSync } from 'node:child_process';
import { Command } from 'commander';
import { AsyncLocalStorage } from 'node:async_hooks';
import { errorCode } from './errors.mts';

export type Locale = 'en' | 'zh-CN';
export type Catalog = Record<string, string>;

// Only the fields this reader needs, so tests can supply a plain stub instead
// of a full `spawnSync` implementation.
export type SystemLanguageReader = (
  command: string,
  args: string[],
  options: { encoding: 'utf8'; timeout: number; maxBuffer: number; windowsHide: boolean; shell: boolean },
) => { status: number | null; stdout: string };

export interface DetectLocaleOptions {
  explicit?: string | undefined;
  env?: NodeJS.ProcessEnv;
  intl?: boolean;
  system?: () => string[];
}

const catalogs = new Map<Locale, Catalog>();
const supported = new Set<string>(['en', 'zh-CN']);
const languageContext = new AsyncLocalStorage<Locale>();
export const withLocale = <T,>(locale: Locale, action: () => T): T => languageContext.run(locale, action);

// Regional formatting (LANG) and the user's preferred interface language are
// distinct on macOS. Query only preferences; never change the shell or OS.
export function systemLanguages(platform: NodeJS.Platform = process.platform, run: SystemLanguageReader = spawnSync as unknown as SystemLanguageReader): string[] {
  const command: [string, string[]] | null = platform === 'darwin' ? ['/usr/bin/defaults', ['read', '-g', 'AppleLanguages']]
    : platform === 'win32' ? ['powershell.exe', ['-NoProfile', '-NonInteractive', '-Command', '[Console]::OutputEncoding=[System.Text.Encoding]::UTF8;(Get-UICulture).Name']] : null;
  if (!command) return [];
  try {
    const result = run(command[0], command[1], { encoding: 'utf8', timeout: 1500, maxBuffer: 16384, windowsHide: true, shell: false });
    if (result.status !== 0) return [];
    return result.stdout.match(/[A-Za-z]{2,3}(?:[-_][A-Za-z0-9]{2,8})*/g) || [];
  } catch { return []; }
}

export function normalizeLocale(value: string | undefined | null): Locale | null {
  if (!value) return null;
  const normalized = String(value).trim().split(':')[0].split(/[.@]/)[0].replaceAll('_', '-').toLowerCase();
  if (normalized === 'zh' || normalized.startsWith('zh-') || normalized.startsWith('cmn')) return 'zh-CN';
  if (normalized === 'en' || normalized.startsWith('en-')) return 'en';
  return null;
}

export function detectLocale({ explicit, env = process.env, intl = true, system = systemLanguages }: DetectLocaleOptions = {}): Locale {
  if (explicit === undefined && env === process.env && languageContext.getStore()) return languageContext.getStore()!;
  if (explicit !== undefined) return normalizeLocale(explicit) || 'en';
  for (const value of [env.SPEC_AUTONOMOUS_LANG, env.LC_ALL, env.LC_MESSAGES, env.LANGUAGE]) {
    if (!value || !String(value).trim()) continue;
    const raw = String(value).trim().split(':')[0];
    if (/^(c|posix)([.@]|$)/i.test(raw)) return 'en';
    return normalizeLocale(raw) || 'en';
  }
  const preferred = system();
  if (preferred.length) return normalizeLocale(preferred[0]) || 'en';
  if (env.LANG?.trim()) return normalizeLocale(env.LANG) || 'en';
  if (intl) {
    try { return normalizeLocale(new Intl.DateTimeFormat().resolvedOptions().locale) || 'en'; } catch {}
  }
  return 'en';
}

export function catalog(locale: Locale = detectLocale()): Catalog {
  const resolved: Locale = supported.has(locale) ? locale : 'en';
  if (!catalogs.has(resolved)) {
    const file = fileURLToPath(new URL(`../locales/${resolved}.json`, import.meta.url));
    catalogs.set(resolved, JSON.parse(readFileSync(file, 'utf8')) as Catalog);
  }
  return catalogs.get(resolved)!;
}

export function message(key: string, locale: Locale = detectLocale(), variables: Record<string, unknown> = {}): string {
  const value = catalog(locale)[key] ?? catalog('en')[key] ?? key;
  return Object.entries(variables).reduce((text, [name, replacement]) => text.replaceAll(`{${name}}`, String(replacement)), value);
}

export function languageArg(argv: string[] = []): string | undefined {
  const parser = new Command().helpOption(false).allowUnknownOption().allowExcessArguments().exitOverride().configureOutput({ writeErr: () => {} });
  // Values that happen to contain --lang remain values, not bootstrap flags.
  for (const name of ['lang','path','framework','provider','goal','input','result','fields','format','view','id','change','feature','milestone','token','prefix','agent']) parser.option(`--${name} <value>`);
  try { parser.parse(argv, { from: 'user' }); } catch {}
  return parser.opts().lang as string | undefined;
}

export function localizeError(error: unknown, locale: Locale = detectLocale()): string {
  const text = error instanceof Error ? error.message : String(error);
  if (locale === 'en') return text;
  const code = errorCode(error);
  if (code === 'invalid_arguments' || code?.startsWith('commander.')) {
    if (code === 'invalid_arguments' && /[\u4e00-\u9fff]/u.test(text)) return text;
    const key = `syntax.${code.slice(10)}`;
    const hint = message(key, locale);
    const tokens = [...text.matchAll(/'([^'\n]+)'/g)].map(m => m[1]);
    return `${hint === key ? message('error.invalid_arguments', locale) : hint}${tokens.length ? `（${tokens.join('、')}）` : ''}。${message('help.hint', locale)}`;
  }
  const [head, ...rest] = text.split(':');
  const key = `error.${head === 'error' || text.startsWith('Usage:') ? 'invalid_arguments' : head}`;
  const translated = message(key, locale);
  if (translated === key) return text;
  // User paths and values are preserved as data; do not append untranslated
  // internal recovery prose. Catalog entries contain the recovery instruction.
  const detail = rest.join(':').trim();
  return detail.startsWith('/') && !detail.includes('\n') ? `${translated}: ${detail}` : translated;
}
