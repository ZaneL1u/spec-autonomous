import test from 'node:test';
import assert from 'node:assert/strict';
import { catalog, detectLocale, languageArg, localizeError, message, normalizeLocale } from '../lib/locale.mjs';

test('normalizes BCP 47 and POSIX locale forms with English fallback', () => {
  assert.equal(normalizeLocale('zh_CN.UTF-8'), 'zh-CN');
  assert.equal(normalizeLocale('cmn-Hans'), 'zh-CN');
  assert.equal(normalizeLocale('en_US.UTF-8'), 'en');
  assert.equal(normalizeLocale('fr-FR'), null);
  assert.equal(detectLocale({ env: { LC_ALL: 'en_US.UTF-8', LANG: 'zh_CN.UTF-8' }, intl: false }), 'en');
  assert.equal(detectLocale({ env: { LC_ALL: 'C', LC_MESSAGES: 'POSIX', LANG: 'zh_CN.UTF-8' }, intl: false }), 'en');
  assert.equal(detectLocale({ env: { LC_ALL: 'C', LANG: 'zh_CN.UTF-8' }, intl: false }), 'en');
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
});
