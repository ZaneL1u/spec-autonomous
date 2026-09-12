import { chmodSync, copyFileSync, cpSync, existsSync, mkdirSync, readFileSync, statSync, writeFileSync } from 'node:fs';
import { resolve, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { parseArgs } from 'node:util';
import { createHash } from 'node:crypto';
import { platforms } from '../packages/cli/lib/platform.mjs';

export function assemble(input, output, { repository } = {}) {
  const source = fileURLToPath(new URL('../packages/cli/', import.meta.url));
  const manifest = JSON.parse(readFileSync(join(source, 'package.json'), 'utf8'));
  if (repository) {
    if (!/^https:\/\/github\.com\/[A-Za-z0-9_.-]+\/[A-Za-z0-9_.-]+$/.test(repository)) throw new Error('Repository must be an explicit public GitHub HTTPS URL');
    manifest.repository = { type: 'git', url: repository };
  }
  const wrapperFiles = ['bin', 'lib', 'locales', 'skills', 'README.md', 'LICENSE'];
  // Preflight before writing anything. A release requires the entire declared matrix.
  if (existsSync(output)) throw new Error(`Output already exists: ${output}. Choose a fresh directory.`);
  for (const name of wrapperFiles) {
    if (!existsSync(join(source, name))) throw new Error(`Missing wrapper asset: ${name}`);
  }
  for (const p of platforms) {
    const binary = join(input, p.target, p.executable);
    if (!existsSync(binary) || !statSync(binary).isFile() || statSync(binary).size === 0) throw new Error(`Missing native artifact: ${binary}`);
  }
  mkdirSync(output, { recursive: true });
  const checksums = [];
  const optionalDependencies = {};
  for (const p of platforms) {
    const name = `spec-autonomous-${p.key}`;
    const directory = join(output, name);
    mkdirSync(join(directory, 'bin'), { recursive: true });
    const binary = join(directory, 'bin', p.executable);
    copyFileSync(join(input, p.target, p.executable), binary);
    chmodSync(binary, 0o755);
    copyFileSync(join(source, 'LICENSE'), join(directory, 'LICENSE'));
    const native = { name, version: manifest.version, description: `Native binary for spec-autonomous (${p.key})`, license: manifest.license, ...(manifest.repository ? { repository: manifest.repository } : {}), os: [p.os], cpu: [p.cpu], ...(p.libc ? { libc: [p.libc] } : {}), files: ['bin', 'LICENSE'], publishConfig: { access: 'public' } };
    writeFileSync(join(directory, 'package.json'), JSON.stringify(native, null, 2) + '\n');
    optionalDependencies[name] = manifest.version;
    checksums.push(`${createHash('sha256').update(readFileSync(binary)).digest('hex')}  ${name}/bin/${p.executable}`);
  }
  const wrapper = join(output, 'spec-autonomous');
  mkdirSync(wrapper);
  for (const name of wrapperFiles) cpSync(join(source, name), join(wrapper, name), { recursive: true });
  chmodSync(join(wrapper, 'bin/spec-autonomous.mjs'), 0o755);
  writeFileSync(join(wrapper, 'package.json'), JSON.stringify({ ...manifest, files: wrapperFiles, optionalDependencies }, null, 2) + '\n');
  writeFileSync(join(output, 'SHA256SUMS'), checksums.join('\n') + '\n');
  return { version: manifest.version, packages: [...Object.keys(optionalDependencies), 'spec-autonomous'] };
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  const { values } = parseArgs({ options: { input: { type: 'string', default: '.artifacts/binaries' }, output: { type: 'string', default: '.artifacts/release' }, repository: { type: 'string' } } });
  console.log(JSON.stringify(assemble(resolve(values.input), resolve(values.output), { repository: values.repository }), null, 2));
}
