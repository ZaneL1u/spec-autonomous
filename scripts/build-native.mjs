import { mkdirSync, copyFileSync, chmodSync } from 'node:fs';
import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';
import { platformFor } from '../packages/cli/lib/platform.mjs';

const root = fileURLToPath(new URL('../', import.meta.url));
const platform = platformFor();
const result = spawnSync('cargo', ['build', '--release', '--locked', '--target', platform.target, '--message-format=json-render-diagnostics', '-p', 'spec-autonomous-cli'], { cwd: root, stdio: ['ignore', 'pipe', 'inherit'], encoding: 'utf8', maxBuffer: 16 * 1024 * 1024, shell: false });
if (result.error) throw result.error;
if (result.status !== 0) process.exit(result.status ?? 1);
const executable = result.stdout.split('\n').filter(Boolean).map((line) => JSON.parse(line))
  .find((item) => item.reason === 'compiler-artifact' && item.target?.name === 'spec-autonomous' && item.executable)?.executable;
if (!executable) throw new Error('Cargo did not report the spec-autonomous executable artifact');
const out = new URL(`../packages/cli/native/${platform.key}/`, import.meta.url);
mkdirSync(out, { recursive: true });
const binary = new URL(platform.executable, out);
copyFileSync(executable, binary);
chmodSync(binary, 0o755);
console.log(`Built ${platform.key}: ${fileURLToPath(binary)}`);
