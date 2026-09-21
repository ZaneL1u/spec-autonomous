import test from 'node:test';
import assert from 'node:assert/strict';
import { catalog, detectLocale, languageArg, localizeError, message, normalizeLocale, systemLanguages, withLocale } from '../lib/locale.mjs';

test('normalizes BCP 47 and POSIX locale forms with English fallback', () => {
  assert.equal(normalizeLocale('zh_CN.UTF-8'), 'zh-CN');
  assert.equal(normalizeLocale('cmn-Hans'), 'zh-CN');
  assert.equal(normalizeLocale('en_US.UTF-8'), 'en');
  assert.equal(normalizeLocale('fr-FR'), null);
  assert.equal(detectLocale({ env: { LC_ALL: 'en_US.UTF-8', LANG: 'zh_CN.UTF-8' }, intl: false }), 'en');
  assert.equal(detectLocale({ env: { LC_ALL: 'C', LC_MESSAGES: 'POSIX', LANG: 'zh_CN.UTF-8' }, intl: false }), 'en');
  assert.equal(detectLocale({ env: { LC_ALL: 'C', LANG: 'zh_CN.UTF-8' }, intl: false }), 'en');
});

test('OS interface language takes priority over formatting LANG, with explicit overrides', () => {
  const system = () => ['zh-Hans-CN', 'en-US'];
  for (const LANG of ['en_US.UTF-8', 'C.UTF-8', undefined]) {
    assert.equal(detectLocale({ env: { LANG }, system, intl: false }), 'zh-CN');
  }
  for (const env of [{ LC_ALL: 'C' }, { LC_MESSAGES: 'en_US.UTF-8' }, { SPEC_AUTONOMOUS_LANG: 'en' }]) {
    assert.equal(detectLocale({ env, system }), 'en');
  }
  assert.equal(detectLocale({ env: { LANG: 'zh_CN.UTF-8' }, system: () => [], intl: false }), 'zh-CN');
  assert.equal(detectLocale({ env: {}, system: () => [], intl: false }), 'en');
  assert.equal(detectLocale({ env: {}, system: () => ['fr-FR', 'zh-CN'], intl: false }), 'en');
});

test('system preference readers are bounded and do not use a shell', () => {
  assert.deepEqual(systemLanguages('darwin', (command, args, options) => {
    assert.equal(command, '/usr/bin/defaults');
    assert.deepEqual(args, ['read', '-g', 'AppleLanguages']);
    assert.equal(options.shell, false); assert.equal(options.timeout, 1500);
    return { status: 0, stdout: '(\n "zh-Hans-CN",\n "en-US"\n)' };
  }), ['zh-Hans-CN', 'en-US']);
  assert.deepEqual(systemLanguages('win32', () => ({ status: 0, stdout: 'zh-CN\r\n' })), ['zh-CN']);
  assert.deepEqual(systemLanguages('darwin', () => { throw new Error('timeout'); }), []);
  assert.deepEqual(systemLanguages('linux', () => assert.fail('no subprocess on Linux')), []);
});

test('async language contexts stay isolated and preserve internal English errors', async () => {
  assert.deepEqual(await Promise.all((['zh-CN', 'en'] as const).map(locale => withLocale(locale, async () => {
    await Promise.resolve(); return detectLocale();
  }))), ['zh-CN', 'en']);
  assert.equal(localizeError(new Error('provider_selection_required: choose openspec or speckit'), 'en'), 'provider_selection_required: choose openspec or speckit');
  assert.match(localizeError(new Error('provider_selection_required: choose openspec or speckit'), 'zh-CN'), /init --provider/);
});

test('explicit language wins and all catalogs have the same stable keys', () => {
  assert.equal(detectLocale({ explicit: 'en-US', env: { SPEC_AUTONOMOUS_LANG: 'zh-CN' }, intl: false }), 'en');
  assert.equal(detectLocale({ explicit: 'fr-FR', env: { LANG: 'zh-CN' }, intl: false }), 'en');
  assert.deepEqual(Object.keys(catalog('en')).sort(), Object.keys(catalog('zh-CN')).sort());
  assert.match(message('command.prepare', 'zh-CN'), /准备/);
  assert.match(message('command.prepare', 'en'), /Prepare/);
  assert.match(localizeError(new Error('invalid_path: /tmp/project'), 'zh-CN'), /^仓库路径无效: \/tmp\/project$/);
  assert.match(localizeError(new Error('error: invalid value'), 'zh-CN'), /^参数无效/);
  assert.equal(localizeError(new Error('unknown_code: details'), 'zh-CN'), 'unknown_code: details');
});

test('bootstrap language argument ignores provider payload after --', () => {
  assert.equal(languageArg(['--lang', 'zh-CN', 'providers', 'exec', 'openspec', '--', '--lang', 'en']), 'zh-CN');
  assert.equal(languageArg(['providers', 'exec', 'openspec', '--', '--lang', 'en']), undefined);
  assert.equal(languageArg(['--lang=en-US', 'help']), 'en-US');
  assert.equal(languageArg(['prepare', '--goal', '--lang', '--id', 'en']), undefined);
});
