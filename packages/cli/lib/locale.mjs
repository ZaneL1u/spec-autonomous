import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';

const catalogs = new Map();
const supported = new Set(['en', 'zh-CN']);

export function normalizeLocale(value) {
  if (!value) return null;
  const normalized = String(value).trim().split(':')[0].replace('_', '-').toLowerCase();
  if (normalized === 'zh' || normalized.startsWith('zh-') || normalized.startsWith('cmn')) return 'zh-CN';
  if (normalized === 'en' || normalized.startsWith('en-')) return 'en';
  return null;
}

export function detectLocale({ explicit, env = process.env, intl = true } = {}) {
  if (explicit !== undefined) return normalizeLocale(explicit) || 'en';
  for (const value of [env.SPEC_AUTONOMOUS_LANG, env.LC_ALL, env.LC_MESSAGES, env.LANGUAGE, env.LANG]) {
    if (!value || !String(value).trim()) continue;
    const raw = String(value).trim().split(':')[0];
    if (/^(c|posix)$/i.test(raw)) return 'en';
    return normalizeLocale(raw) || 'en';
  }
  if (intl) {
    try { return normalizeLocale(new Intl.DateTimeFormat().resolvedOptions().locale) || 'en'; } catch {}
  }
  return 'en';
}

export function catalog(locale = detectLocale()) {
  const resolved = supported.has(locale) ? locale : 'en';
  if (!catalogs.has(resolved)) {
    const file = fileURLToPath(new URL(`../locales/${resolved}.json`, import.meta.url));
    catalogs.set(resolved, JSON.parse(readFileSync(file, 'utf8')));
  }
  return catalogs.get(resolved);
}

export function message(key, locale = detectLocale(), variables = {}) {
  const value = catalog(locale)[key] ?? catalog('en')[key] ?? key;
  return Object.entries(variables).reduce((text, [name, replacement]) => text.replaceAll(`{${name}}`, String(replacement)), value);
}

export function languageArg(argv = []) {
  const separator = argv.indexOf('--');
  const own = separator >= 0 ? argv.slice(0, separator) : argv;
  const index = own.indexOf('--lang');
  return index >= 0 ? own[index + 1] : own.find(value => value.startsWith('--lang='))?.slice(7);
}

export function localizeError(error, locale = detectLocale()) {
  const text = error instanceof Error ? error.message : String(error);
  const [code, ...rest] = text.split(':');
  const key = `error.${code === 'error' || text.startsWith('Usage:') ? 'invalid_arguments' : code}`;
  const translated = message(key, locale);
  return translated === key ? text : [translated, ...(code === 'error' ? rest.slice(1) : rest)].join(':');
}
