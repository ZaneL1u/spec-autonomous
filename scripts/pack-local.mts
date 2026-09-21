import { mkdirSync } from 'node:fs';
import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';
import { npmCommand } from './npm-command.mts';

const root = fileURLToPath(new URL('../', import.meta.url));
function run(command:string, args:string[], cwd = root) {
  const result = spawnSync(command, args, { cwd, stdio: 'inherit', shell: false });
  if (result.error) throw result.error;
  if (result.status !== 0) process.exit(result.status ?? 1);
}
run(process.execPath, ['scripts/build-native.mts']);
const artifacts = fileURLToPath(new URL('../.artifacts/local/', import.meta.url));
mkdirSync(artifacts, { recursive: true });
npmCommand(['pack', '--pack-destination', artifacts], { cwd: fileURLToPath(new URL('../packages/cli/', import.meta.url)), stdio: 'inherit' });
