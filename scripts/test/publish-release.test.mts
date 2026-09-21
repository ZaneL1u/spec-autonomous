import test from 'node:test';
import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import { gzipSync } from 'node:zlib';
import { mkdtempSync, writeFileSync, readFileSync, readdirSync, rmSync, mkdirSync, existsSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, basename } from 'node:path';
import { platforms } from '../../packages/cli/lib/platform.mjs';
import type { Platform } from '../../packages/cli/lib/platform.mjs';
import { inspectRelease, publishRelease, tarEntries } from '../publish-release.mts';
import { npmCommand } from '../npm-command.mts';

const version = '0.1.0-alpha.1';
const sha = (bytes:NodeJS.ArrayBufferView) => createHash('sha256').update(bytes).digest('hex');
type Entry=[string,Uint8Array|string,number?,string?];
type Manifest=Record<string,any>;
type Customize=(manifest:Manifest,entries:Entry[],platform:Platform|null)=>void;

// Synthetic header-only bytes exercise validation branches. These are NOT
// runnable native binaries and are never sent to a real registry. Every test
// with publish=true injects the local fake npm below.
function headerFixture(platform:Platform) {
  const bytes = Buffer.alloc(2048);
  if (platform.os === 'darwin') {
    bytes.writeUInt32LE(0xfeedfacf, 0);
    bytes.writeUInt32LE(platform.cpu === 'arm64' ? 0x100000c : 0x1000007, 4);
    bytes.writeUInt32LE(2, 12); bytes.writeUInt32LE(1, 16);
  } else if (platform.os === 'linux') {
    Buffer.from('7f454c460201', 'hex').copy(bytes);
    bytes.writeUInt16LE(2, 16); bytes.writeUInt16LE(platform.cpu === 'arm64' ? 183 : 62, 18); bytes.writeUInt16LE(1, 56);
  } else {
    bytes.write('MZ'); bytes.writeUInt32LE(128, 0x3c); bytes.writeUInt32LE(0x4550, 128);
    bytes.writeUInt16LE(platform.cpu === 'arm64' ? 0xaa64 : 0x8664, 132); bytes.writeUInt16LE(2, 150); bytes.writeUInt16LE(0x20b, 152);
  }
  bytes.write('NONEXECUTABLE UNIT TEST INPUT', 512);
  return bytes;
}

function archive(entries:Entry[]) {
  const parts:Buffer[] = [];
  for (const [path, bytes, mode = 0o644, type = '0'] of entries) {
    const body = Buffer.from(bytes); const header = Buffer.alloc(512);
    header.write(path, 0); header.write(`${mode.toString(8).padStart(7, '0')}\0`, 100);
    header.write('0000000\0', 108); header.write('0000000\0', 116);
    header.write(`${body.length.toString(8).padStart(11, '0')}\0`, 124);
    header.write('00000000000\0', 136); header.fill(32, 148, 156);
    header.write(type, 156); header.write('ustar\0', 257); header.write('00', 263);
    header.write(`${header.reduce((a, b) => a + b, 0).toString(8).padStart(6, '0')}\0 `, 148);
    parts.push(header, body, Buffer.alloc((512 - body.length % 512) % 512));
  }
  return gzipSync(Buffer.concat([...parts, Buffer.alloc(1024)]));
}

function fixture(t:test.TestContext, customize:Customize = () => {}) {
  const root = mkdtempSync(join(tmpdir(), 'publish contract '));
  t.after(() => rmSync(root, { recursive: true, force: true }));
  const records:string[] = [];
  for (const platform of [...platforms, null]) {
    const name = platform ? `spec-autonomous-${platform.key}` : 'spec-autonomous';
    const manifest:Manifest = { name, version, license: 'MIT', repository: { type: 'git', url: `git+https://github.com/${process.env.GITHUB_REPOSITORY ?? 'example/spec-autonomous'}.git` }, publishConfig: { access: 'public' }, ...(platform ? { os: [platform.os], cpu: [platform.cpu], ...(platform.libc ? { libc: [platform.libc] } : {}) } : { bin: { 'spec-autonomous': 'bin/spec-autonomous.mjs' }, optionalDependencies: Object.fromEntries(platforms.map(p => [`spec-autonomous-${p.key}`, version])) }) };
    const entries:Entry[] = platform ? [[`package/bin/${platform.executable}`, headerFixture(platform), 0o755]] : [
      ['package/bin/spec-autonomous.mjs', '#!/usr/bin/env node\n', 0o755],
      ['package/lib/platform.mjs', 'export const platforms = [];\n'],
      ['package/skills/autonomous/SKILL.md', '# Autonomous\n'],
      ['package/skills/auto/SKILL.md', '# Alias\n'],
    ];
    if (!platform) manifest.dependencies = { commander: '14.0.3' };
    entries.push(['package/LICENSE', 'MIT test input\n']);
    customize(manifest, entries, platform);
    entries.push(['package/package.json', JSON.stringify(manifest)]);
    const filename = `${name}-${manifest.version}.tgz`;
    const bytes = archive(entries);
    writeFileSync(join(root, filename), bytes);
    records.push(`${sha(bytes)}  ${filename}`);
  }
  writeFileSync(join(root, 'SHA256SUMS'), `${records.join('\n')}\n`);
  return root;
}

function fakeNpm(packages:ReturnType<typeof inspectRelease>, { existing = new Map<string,string>(), failName, corruptVisibleName, neverVisibleName, errorCode }:{existing?:Map<string,string>;failName?:string;corruptVisibleName?:string;neverVisibleName?:string;errorCode?:string} = {}) {
  const calls:string[][] = []; const published:string[] = []; const registry = new Map(existing);
  const npm = async (args:string[], { cwd }:{cwd:string}) => {
    calls.push([...args]);
    assert.equal(args.includes('--registry'), true);
    if (args[0] === 'view') {
      if (errorCode) return { status: 1, stdout: JSON.stringify({ error: { code: errorCode } }) };
      const value = registry.get(args[1]!);
      return value ? { status: 0, stdout: JSON.stringify(value) } : { status: 1, stdout: '{"error":{"code":"E404"}}' };
    }
    assert.equal(args[0], 'publish');
    assert.ok(args.includes('--ignore-scripts')); assert.ok(args.includes('--provenance'));
    const record = packages.find(p => p.filename === basename(args[1]!));
    assert.ok(record); assert.equal(cwd, args[1]!.slice(0, -basename(args[1]!).length - 1));
    assert.equal(sha(readFileSync(args[1]!)), record.sha256, 'publish the prevalidated snapshot bytes');
    if (record.name === failName) return { status: 1, stdout: '{"error":{"code":"E403"}}' };
    published.push(record.name);
    if (record.name !== neverVisibleName) registry.set(`${record.name}@${record.version}`, record.name === corruptVisibleName ? 'sha512-mismatch' : record.sha512);
    return { status: 0, stdout: JSON.stringify({ id: `${record.name}@${record.version}` }) };
  };
  return { npm, calls, published, registry, wait: async () => {} };
}

test('default dry-run validates exact tarballs and sha512 with zero npm calls or file changes', async t => {
  const root = fixture(t); const before = new Map(readdirSync(root).map(name => [name, sha(readFileSync(join(root, name)))]));
  const report = await publishRelease({ directory: root }, { npm: async () => { throw Error('dry-run must not call npm'); } });
  assert.equal(report.mode, 'dry-run'); assert.equal(report.packages.length, 7);
  assert.ok(report.packages.every(p => /^sha512-[A-Za-z0-9+/]{86}==$/.test(p.sha512)));
  assert.deepEqual(new Map(readdirSync(root).map(name => [name, sha(readFileSync(join(root, name)))])), before);
});

test('missing platform, extra tarball and altered inventory fail before any npm action', async t => {
  for (const alter of [(root:string) => rmSync(join(root, `spec-autonomous-linux-x64-${version}.tgz`)), (root:string) => writeFileSync(join(root, 'extra.tgz'), 'invalid'), (root:string) => writeFileSync(join(root, 'SHA256SUMS'), 'broken')]) {
    const root = fixture(t); alter(root);
    await assert.rejects(publishRelease({ directory: root, publish: true }, { npm: async () => assert.fail('preflight must finish first') }), /release_rejected/);
  }
});

test('only the exact reviewed Commander dependency is allowed on the wrapper', t => {
  for (const customize of [
    (m:Manifest, _e:Entry[], p:Platform|null) => { if (!p) m.dependencies.commander = '^14.0.3'; },
    (m:Manifest, _e:Entry[], p:Platform|null) => { if (!p) m.dependencies.unreviewed = '1.0.0'; },
    (m:Manifest, _e:Entry[], p:Platform|null) => { if (p) m.dependencies = { commander: '14.0.3' }; },
    (m:Manifest, _e:Entry[], p:Platform|null) => { if (!p) delete m.dependencies; },
  ]) assert.throws(() => inspectRelease(fixture(t, customize)), /dependency policy/);
});

test('version skew and nonexact wrapper optional dependencies are rejected', async t => {
  for (const alter of [(m:Manifest) => { if (m.name.endsWith('linux-x64')) m.version = '0.1.1'; }, (m:Manifest) => { if (m.name === 'spec-autonomous') m.optionalDependencies['spec-autonomous-linux-x64'] = `^${version}`; }]) {
    await assert.rejects(publishRelease({ directory: fixture(t, alter) }), /versions must match|optionalDependencies/);
  }
});

test('missing provenance repository is reported offline and refuses all real publish actions', async t => {
  const root = fixture(t, m => { delete m.repository; });
  const report = await publishRelease({ directory: root });
  assert.ok(report.publication_prerequisites.some(item => item.includes('repository.url')));
  await assert.rejects(publishRelease({ directory: root, publish: true }, { npm: async () => assert.fail('provenance preflight must precede registry access') }), /repository.url/);
});

test('install scripts, archive links, traversal and text placeholder binaries are rejected', async t => {
  const changes:Customize[] = [
    m => { m.scripts = { postinstall: 'unsafe' }; },
    (m, entries) => { entries.push(['package/link', '', 0o777, '2']); },
    (m, entries) => { entries.push(['package/../outside', 'no']); },
    (m, entries, p) => { if (p) entries[0]![1] = 'fixture-linux-x64'; },
  ];
  for (const mutate of changes) await assert.rejects(publishRelease({ directory: fixture(t, mutate) }), /release_rejected/);
});

test('native architecture mismatch and compressed-byte tampering fail preflight', async t => {
  const swapped = fixture(t, (m, entries, p) => { if (p?.key === 'linux-x64') entries[0]![1] = headerFixture(platforms.find(p => p.key === 'linux-arm64')!); });
  await assert.rejects(publishRelease({ directory: swapped }), /architecture/);
  const root = fixture(t); const path = join(root, `spec-autonomous-${version}.tgz`);
  const bytes = readFileSync(path); bytes[bytes.length - 1]! ^= 1; writeFileSync(path, bytes);
  await assert.rejects(publishRelease({ directory: root }), /SHA256 mismatch/);
});

test('preexisting version with different integrity blocks all publication including wrapper', async t => {
  const root = fixture(t); const packages = inspectRelease(root);
  for (const name of ['spec-autonomous-linux-x64', 'spec-autonomous']) {
    const adapter = fakeNpm(packages, { existing: new Map([[`${name}@${version}`, 'sha512-wrong']]) });
    await assert.rejects(publishRelease({ directory: root, publish: true }, adapter), /registry_integrity_mismatch/);
    assert.deepEqual(adapter.published, []);
  }
});

test('platform failure never publishes wrapper and preserves the successful prefix', async t => {
  const root = fixture(t); const packages = inspectRelease(root);
  const adapter = fakeNpm(packages, { failName: packages[2]!.name });
  await assert.rejects(publishRelease({ directory: root, publish: true }, adapter), /npm_publish_failed/);
  assert.deepEqual(adapter.published, packages.slice(0, 2).map(p => p.name));
  assert.ok(!adapter.calls.some(args => args[0] === 'publish' && basename(args[1]!) === packages.at(-1)!.filename));
});

test('postpublish integrity mismatch and visibility timeout both block wrapper', async t => {
  const root = fixture(t); const packages = inspectRelease(root);
  for (const options of [{ corruptVisibleName: packages[1]!.name }, { neverVisibleName: packages[1]!.name }]) {
    const adapter = fakeNpm(packages, options);
    await assert.rejects(publishRelease({ directory: root, publish: true, visibilityAttempts: 2 }, adapter), /registry_integrity_mismatch|registry_visibility_timeout/);
    assert.ok(!adapter.published.includes('spec-autonomous'));
  }
});

test('registry errors are not treated as missing packages', async t => {
  const root = fixture(t); const packages = inspectRelease(root);
  for (const errorCode of ['E401', 'E403', 'ETIMEDOUT']) {
    const adapter = fakeNpm(packages, { errorCode });
    await assert.rejects(publishRelease({ directory: root, publish: true }, adapter), /registry_lookup_failed/);
    assert.deepEqual(adapter.published, []);
  }
});

test('all native packages become visible and are rechecked before the wrapper publishes', async t => {
  const root = fixture(t); const packages = inspectRelease(root); const adapter = fakeNpm(packages);
  const report = await publishRelease({ directory: root, publish: true }, adapter);
  assert.deepEqual(adapter.published, packages.map(p => p.name));
  assert.ok(report.packages.every(p => p.action === 'published'));
  const wrapperPublish = adapter.calls.findIndex(args => args[0] === 'publish' && basename(args[1]!) === packages.at(-1)!.filename);
  const lastPlatformPublish = adapter.calls.findLastIndex(args => args[0] === 'publish' && basename(args[1]!) === packages.at(-2)!.filename);
  const barrier = adapter.calls.slice(lastPlatformPublish + 1, wrapperPublish).filter(args => args[0] === 'view').map(args => args[1]);
  for (const p of packages.slice(0, -1)) assert.ok(barrier.includes(`${p.name}@${p.version}`));
  assert.ok(!existsSync(adapter.calls.find(args => args[0] === 'publish')![1]!), 'private snapshots are removed after completion');
});

test('a platform integrity change at the final barrier prevents wrapper publication', async t => {
  const root = fixture(t); const packages = inspectRelease(root); const adapter = fakeNpm(packages);
  const baseNpm = adapter.npm;
  adapter.npm = async (args, options) => {
    if (adapter.published.length === 6 && args[0] === 'view' && args[1] === `${packages[0]!.name}@${version}`) return { status: 0, stdout: '"sha512-changed-after-platform-publication"' };
    return baseNpm(args, options);
  };
  await assert.rejects(publishRelease({ directory: root, publish: true }, adapter), /registry_integrity_mismatch/);
  assert.deepEqual(adapter.published, packages.slice(0, -1).map(p => p.name));
});

test('same-integrity versions are reused without republishing or retagging', async t => {
  const root = fixture(t); const packages = inspectRelease(root);
  const adapter = fakeNpm(packages, { existing: new Map(packages.map(p => [`${p.name}@${p.version}`, p.sha512])) });
  const report = await publishRelease({ directory: root, publish: true }, adapter);
  assert.deepEqual(adapter.published, []);
  assert.ok(report.packages.every(p => p.action === 'reused'));
  assert.ok(adapter.calls.every(args => args[0] === 'view'));
});

test('the archive reader accepts actual npm pack output without executing package scripts', t => {
  const root = mkdtempSync(join(tmpdir(), 'npm real tar contract '));
  t.after(() => rmSync(root, { recursive: true, force: true }));
  const source = join(root, 'source'); mkdirSync(source);
  writeFileSync(join(source, 'package.json'), JSON.stringify({ name: 'tar-contract', version: '1.0.0', scripts: { prepack: 'node -e "process.exit(99)"' } }));
  writeFileSync(join(source, 'README.md'), 'real npm archive input');
  const packed = npmCommand(['pack', '--ignore-scripts', '--json', '--pack-destination', root], { cwd: source, encoding: 'utf8' });
  const archivePath = join(root, (JSON.parse(packed.stdout) as Array<{filename:string}>)[0]!.filename);
  assert.equal(tarEntries(readFileSync(archivePath)).get('package/README.md')!.bytes.toString(), 'real npm archive input');
});
