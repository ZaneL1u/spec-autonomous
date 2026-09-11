import { createHash } from 'node:crypto';
import { gunzipSync } from 'node:zlib';
import { readFileSync, readdirSync, lstatSync, mkdtempSync, writeFileSync, rmSync, existsSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { basename, dirname, join, resolve } from 'node:path';
import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';
import { parseArgs } from 'node:util';
import { platforms } from '../packages/cli/lib/platform.mjs';

const REGISTRY = 'https://registry.npmjs.org/';
const MAX_ARCHIVE = 128 * 1024 * 1024;
const digest = (bytes, algorithm, encoding = 'hex') => createHash(algorithm).update(bytes).digest(encoding);
const integrity = bytes => `sha512-${digest(bytes, 'sha512', 'base64')}`;
const reject = message => { throw new Error(`release_rejected: ${message}`); };
const nonempty = value => value && Object.keys(value).length > 0;

function regularFile(path) {
  const stat = lstatSync(path);
  if (!stat.isFile() || stat.isSymbolicLink() || stat.size > MAX_ARCHIVE) reject(`not a bounded regular file: ${path}`);
  return readFileSync(path);
}

// Inspect the archive in memory: never extract untrusted archive paths, links,
// or lifecycle hooks. Current npm packages use ordinary USTAR entries. Unknown
// extensions fail closed rather than being interpreted differently by npm.
export function tarEntries(compressed) {
  const tar = gunzipSync(compressed, { maxOutputLength: MAX_ARCHIVE });
  const entries = new Map();
  const string = field => field.toString('utf8').replace(/\0.*$/s, '');
  const octal = field => {
    const value = string(field).trim();
    if (!/^[0-7]+$/.test(value)) reject('invalid tar numeric field');
    const number = Number.parseInt(value, 8);
    if (!Number.isSafeInteger(number)) reject('oversized tar field');
    return number;
  };
  let offset = 0;
  while (offset + 512 <= tar.length) {
    const header = tar.subarray(offset, offset + 512);
    if (header.every(byte => byte === 0)) {
      if (tar.length - offset < 1024 || !tar.subarray(offset).every(byte => byte === 0)) reject('invalid tar terminator');
      return entries;
    }
    let checksum = 0;
    for (let i = 0; i < 512; i++) checksum += i >= 148 && i < 156 ? 32 : header[i];
    if (checksum !== octal(header.subarray(148, 156))) reject('tar checksum mismatch');
    if (string(header.subarray(257, 263)) !== 'ustar') reject('unsupported tar format');
    const prefix = string(header.subarray(345, 500));
    const name = `${prefix ? `${prefix}/` : ''}${string(header.subarray(0, 100))}`.replace(/\/$/, '');
    if (!name.startsWith('package/') || name.split('/').some(part => !part || part === '.' || part === '..') || /[\\\0\r\n]/.test(name)) reject(`unsafe tar path: ${name}`);
    if (entries.has(name)) reject(`duplicate tar path: ${name}`);
    const type = header[156];
    if (![0, 48, 53].includes(type)) reject(`unsupported tar entry type ${type}: ${name}`);
    const size = octal(header.subarray(124, 136));
    if (offset + 512 + size > tar.length || (type === 53 && size !== 0)) reject(`truncated tar entry: ${name}`);
    entries.set(name, { bytes: tar.subarray(offset + 512, offset + 512 + size), mode: octal(header.subarray(100, 108)), directory: type === 53 });
    offset += 512 + Math.ceil(size / 512) * 512;
  }
  reject('unterminated tar archive');
}

// Header checks reject the text placeholders used by packaging tests. They do
// not replace native execution: the publishing workflow additionally requires
// the successful six-platform build/smoke workflow that produced these bytes.
function nativeHeader(bytes, platform) {
  if (bytes.length < 1024) reject(`native binary is too small: ${platform.key}`);
  if (platform.os === 'darwin') {
    if (bytes.readUInt32LE(0) !== 0xfeedfacf || bytes.readUInt32LE(4) !== (platform.cpu === 'arm64' ? 0x100000c : 0x1000007) || bytes.readUInt32LE(12) !== 2 || bytes.readUInt32LE(16) === 0) reject(`Mach-O architecture/executable mismatch: ${platform.key}`);
  } else if (platform.os === 'linux') {
    if (bytes.subarray(0, 4).toString('hex') !== '7f454c46' || bytes[4] !== 2 || bytes[5] !== 1 || ![2, 3].includes(bytes.readUInt16LE(16)) || bytes.readUInt16LE(18) !== (platform.cpu === 'arm64' ? 183 : 62) || bytes.readUInt16LE(56) === 0) reject(`ELF architecture/executable mismatch: ${platform.key}`);
  } else {
    const pe = bytes.readUInt32LE(0x3c);
    if (bytes.subarray(0, 2).toString() !== 'MZ' || pe < 64 || pe + 26 > bytes.length || bytes.readUInt32LE(pe) !== 0x4550 || bytes.readUInt16LE(pe + 4) !== (platform.cpu === 'arm64' ? 0xaa64 : 0x8664) || !(bytes.readUInt16LE(pe + 22) & 2) || bytes.readUInt16LE(pe + 24) !== 0x20b) reject(`PE architecture/executable mismatch: ${platform.key}`);
  }
}

export function inspectRelease(directory) {
  directory = resolve(directory);
  if (lstatSync(directory).isSymbolicLink()) reject('release directory must not be a symlink');
  const files = readdirSync(directory).filter(name => name.endsWith('.tgz')).sort();
  if (files.length !== platforms.length + 1) reject('expected exactly six platform tarballs and one wrapper');
  const sums = new Map();
  for (const line of regularFile(join(directory, 'SHA256SUMS')).toString('utf8').trim().split('\n')) {
    const match = /^([a-f0-9]{64})  ([a-z0-9][a-z0-9._-]*\.tgz)$/.exec(line);
    if (!match || sums.has(match[2])) reject('invalid or duplicate SHA256SUMS entry');
    sums.set(match[2], match[1]);
  }
  if (sums.size !== files.length || files.some(name => !sums.has(name))) reject('tarball inventory differs from SHA256SUMS');
  const packages = new Map();
  for (const filename of files) {
    const bytes = regularFile(join(directory, filename));
    if (digest(bytes, 'sha256') !== sums.get(filename)) reject(`SHA256 mismatch: ${filename}`);
    const entries = tarEntries(bytes);
    const manifestEntry = entries.get('package/package.json');
    if (!manifestEntry || manifestEntry.directory || manifestEntry.bytes.length > 1024 * 1024) reject(`missing package manifest: ${filename}`);
    const manifest = JSON.parse(manifestEntry.bytes.toString('utf8'));
    const { name, version } = manifest;
    if (typeof version !== 'string' || !/^(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)(?:-[0-9A-Za-z.-]+)?$/.test(version)) reject(`invalid release version: ${filename}`);
    if (filename !== `${name}-${version}.tgz` || packages.has(name)) reject(`unexpected filename or duplicate package: ${filename}`);
    if (manifest.private || nonempty(manifest.scripts) || nonempty(manifest.peerDependencies) || nonempty(manifest.bundledDependencies) || nonempty(manifest.bundleDependencies)) reject(`scripts/private/dependency policy: ${name}`);
    // Only the wrapper may depend on the reviewed CLI parser. Do not relax this
    // to arbitrary package dependencies or ranges when assembling releases.
    if (name === 'spec-autonomous') {
      if (JSON.stringify(manifest.dependencies) !== JSON.stringify({ commander: '14.0.3' })) reject(`scripts/private/dependency policy: ${name}`);
    } else if (nonempty(manifest.dependencies)) reject(`scripts/private/dependency policy: ${name}`);
    if (manifest.publishConfig?.access !== 'public' || (manifest.publishConfig.registry && manifest.publishConfig.registry !== REGISTRY)) reject(`unexpected publishConfig: ${name}`);
    for (const path of entries.keys()) if (/(^|\/)(node_modules|\.git|\.npmrc|binding\.gyp)(\/|$)/.test(path)) reject(`unexpected install/configuration payload: ${path}`);
    const platform = platforms.find(item => name === `spec-autonomous-${item.key}`);
    if (platform) {
      if (JSON.stringify(manifest.os) !== JSON.stringify([platform.os]) || JSON.stringify(manifest.cpu) !== JSON.stringify([platform.cpu]) || JSON.stringify(manifest.libc) !== JSON.stringify(platform.libc ? [platform.libc] : undefined) || nonempty(manifest.optionalDependencies)) reject(`platform manifest mismatch: ${name}`);
      const executable = entries.get(`package/bin/${platform.executable}`);
      if (!executable || executable.directory || !(executable.mode & 0o111)) reject(`missing executable/mode: ${name}`);
      nativeHeader(executable.bytes, platform);
      const allowed = new Set(['package/package.json', 'package/LICENSE', 'package/bin', `package/bin/${platform.executable}`]);
      if ([...entries.keys()].some(path => !allowed.has(path))) reject(`unexpected native package payload: ${name}`);
    } else if (name === 'spec-autonomous') {
      if (JSON.stringify(manifest.bin) !== JSON.stringify({ 'spec-autonomous': 'bin/spec-autonomous.mjs' })) reject('wrapper bin mismatch');
      if (entries.has('package/native') || [...entries.keys()].some(path => path.startsWith('package/native/') || path.endsWith('.exe'))) reject('wrapper contains a native binary');
      for (const entry of ['package/bin/spec-autonomous.mjs', 'package/lib/platform.mjs', 'package/skills/autonomous/SKILL.md', 'package/skills/auto/SKILL.md', 'package/LICENSE']) if (!entries.has(entry)) reject(`missing wrapper payload: ${entry}`);
    } else reject(`unexpected package name: ${name}`);
    packages.set(name, { name, version, filename, path: join(directory, filename), sha512: integrity(bytes), sha256: sums.get(filename), bytes, manifest });
  }
  const wrapper = packages.get('spec-autonomous');
  if (!wrapper) reject('wrapper is missing');
  const expected = Object.fromEntries(platforms.map(p => [`spec-autonomous-${p.key}`, wrapper.version]));
  const actual = wrapper.manifest.optionalDependencies;
  if (!actual || Object.keys(actual).length !== platforms.length || Object.entries(expected).some(([name, version]) => actual[name] !== version)) reject('wrapper optionalDependencies must pin the exact six platform versions');
  const ordered = [...platforms.map(p => packages.get(`spec-autonomous-${p.key}`)), wrapper];
  if (ordered.some(p => !p || p.version !== wrapper.version)) reject('all seven package versions must match');
  return ordered;
}

export async function defaultNpm(args, options) {
  // Unlike npmCommand (pack tooling), preserve structured nonzero output so a
  // JSON E404 can be distinguished from auth/network errors without prose parsing.
  const configured = process.env.npm_execpath;
  const entry = [
    ...(configured && basename(configured) === 'npm-cli.js' ? [configured] : []),
    join(dirname(process.execPath), 'node_modules/npm/bin/npm-cli.js'),
    join(dirname(process.execPath), '../lib/node_modules/npm/bin/npm-cli.js'),
  ].find(existsSync);
  if (!entry) throw new Error('npm_unavailable: use a Node installation that includes npm');
  const result = spawnSync(process.execPath, [entry, ...args], { ...options, encoding: 'utf8', shell: false, timeout: 90_000, maxBuffer: 4 * 1024 * 1024 });
  if (result.error) throw result.error;
  if (result.signal || result.status === null) throw new Error('npm_interrupted: command did not finish');
  return result;
}

export async function publishRelease({ directory = '.artifacts/npm', publish = false, tag = 'next', visibilityAttempts = 6 } = {}, { npm = defaultNpm, wait = ms => new Promise(resolve => setTimeout(resolve, ms)) } = {}) {
  if (!/^[a-z][a-z0-9-]*$/.test(tag)) reject('invalid npm tag');
  if (typeof publish !== 'boolean') reject('publish must be an explicit boolean');
  if (!Number.isInteger(visibilityAttempts) || visibilityAttempts < 1 || visibilityAttempts > 10) reject('visibility attempts must be between one and ten');
  const packages = inspectRelease(directory);
  const repository = p => {
    const url = typeof p.manifest.repository === 'string' ? p.manifest.repository : p.manifest.repository?.url;
    return typeof url === 'string' ? /^(?:git\+)?https:\/\/github\.com\/([A-Za-z0-9_.-]+\/[A-Za-z0-9_.-]+?)(?:\.git)?$/.exec(url)?.[1] : undefined;
  };
  const repo = repository(packages.at(-1));
  const prerequisites = [];
  if (!repo || packages.some(p => repository(p) !== repo)) prerequisites.push('All seven manifests need the same public GitHub repository.url for provenance');
  if (process.env.GITHUB_REPOSITORY && repo !== process.env.GITHUB_REPOSITORY) prerequisites.push('package repository.url must match the publishing GitHub repository exactly');
  const report = { mode: publish ? 'publish' : 'dry-run', registry: REGISTRY, tag, version: packages[0].version, repository: repo ?? null, publication_prerequisites: prerequisites, native_validation: 'format-and-architecture only; execution evidence belongs to the successful release-artifacts workflow', packages: packages.map(({ name, version, filename, sha256, sha512 }) => ({ name, version, filename, sha256, sha512 })) };
  if (!publish) return report; // No npm invocation, registry access or writes.
  if (prerequisites.length) reject(prerequisites.join('; '));
  const stage = mkdtempSync(join(tmpdir(), 'spec-autonomous-release-'));
  try {
    // Publish private snapshots of the exact validated bytes, never a mutable
    // caller-controlled input path inspected earlier in the operation.
    for (const p of packages) writeFileSync(join(stage, p.filename), p.bytes, { flag: 'wx', mode: 0o400 });
    const invoke = args => npm(args, { cwd: stage });
    const visible = async p => {
      const response = await invoke(['view', `${p.name}@${p.version}`, 'dist.integrity', '--json', '--registry', REGISTRY, '--prefer-online', '--fetch-retries=0', '--fetch-timeout=30000']);
      let data;
      try { data = JSON.parse(response.stdout); } catch { throw new Error(`registry_protocol_error: ${p.name}`); }
      if (response.status !== 0) {
        if (data?.error?.code === 'E404') return false;
        throw new Error(`registry_lookup_failed: ${p.name}`);
      }
      if (data !== p.sha512) throw new Error(`registry_integrity_mismatch: ${p.name}@${p.version}`);
      return true;
    };
    const waitVisible = async p => {
      for (let attempt = 0; attempt < visibilityAttempts; attempt++) {
        if (await visible(p)) return;
        if (attempt + 1 < visibilityAttempts) await wait(Math.min(1000 * 2 ** attempt, 8000));
      }
      throw new Error(`registry_visibility_timeout: ${p.name}@${p.version}`);
    };
    // Preflight all existing versions, including the wrapper, before the first
    // irreversible publish. A name/version can only be reused for identical bytes.
    for (const p of packages) await visible(p);
    const ensure = async p => {
      if (await visible(p)) { p.action = 'reused'; return; }
      const result = await invoke(['publish', join(stage, p.filename), '--tag', tag, '--access', 'public', '--provenance', '--ignore-scripts', '--registry', REGISTRY, '--json']);
      if (result.status !== 0) throw new Error(`npm_publish_failed: ${p.name}`);
      p.action = 'published';
      await waitVisible(p);
    };
    for (const p of packages.slice(0, -1)) await ensure(p);
    // Fresh barrier, even for reused packages: no wrapper is published before
    // every exact native version is observable with the expected SRI digest.
    for (const p of packages.slice(0, -1)) await waitVisible(p);
    await ensure(packages.at(-1));
    report.packages.forEach((p, i) => { p.action = packages[i].action; });
    return report;
  } finally { rmSync(stage, { recursive: true, force: true }); }
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  try {
    const { values } = parseArgs({ options: { input: { type: 'string', default: '.artifacts/npm' }, publish: { type: 'boolean', default: false }, tag: { type: 'string', default: 'next' } } });
    console.log(JSON.stringify(await publishRelease({ directory: values.input, publish: values.publish, tag: values.tag }), null, 2));
  } catch (error) { console.error(error.message); process.exitCode = 1; }
}
