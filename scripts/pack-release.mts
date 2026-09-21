import { mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import { resolve, join } from 'node:path';
import { parseArgs } from 'node:util';
import { createHash } from 'node:crypto';
import { platforms } from '../packages/cli/lib/platform.mjs';
import { npmCommand } from './npm-command.mts';

interface PackResult { filename:string }

const { values } = parseArgs({ options: { input: { type: 'string', default: '.artifacts/release' }, output: { type: 'string', default: '.artifacts/npm' } } });
const input = resolve(values.input);
const output = resolve(values.output);
mkdirSync(output, { recursive: true });
const sums:string[] = [];
for (const name of [...platforms.map((p) => `spec-autonomous-${p.key}`), 'spec-autonomous']) {
  const result = npmCommand(['pack', '--json', '--pack-destination', output], { cwd: join(input, name), encoding: 'utf8' });
  const [{ filename }] = JSON.parse(result.stdout) as PackResult[];
  sums.push(`${createHash('sha256').update(readFileSync(join(output, filename))).digest('hex')}  ${filename}`);
  console.log(filename);
}
writeFileSync(join(output, 'SHA256SUMS'), sums.join('\n') + '\n');
