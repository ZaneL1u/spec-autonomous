import { mkdirSync, copyFileSync, chmodSync, renameSync } from 'node:fs';
import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';
import { platformFor } from '../packages/cli/lib/platform.mjs';

interface CargoArtifact { reason?: string; target?: { name?: string }; executable?: string }

const root = fileURLToPath(new URL('../', import.meta.url));
const platform = platformFor();
const result = spawnSync('cargo', ['build', '--release', '--locked', '--target', platform.target, '--message-format=json-render-diagnostics', '-p', 'spec-autonomous-cli'], { cwd: root, stdio: ['ignore', 'pipe', 'inherit'], encoding: 'utf8', maxBuffer: 16 * 1024 * 1024, shell: false });
if (result.error) throw result.error;
if (result.status !== 0) process.exit(result.status ?? 1);
const executable = result.stdout.split('\n').filter(Boolean).map((line): CargoArtifact => JSON.parse(line) as CargoArtifact)
  .find((item) => item.reason === 'compiler-artifact' && item.target?.name === 'spec-autonomous' && item.executable)?.executable;
if (!executable) throw new Error('Cargo did not report the spec-autonomous executable artifact');
const out = new URL(`../packages/cli/native/${platform.key}/`, import.meta.url);
mkdirSync(out, { recursive: true });
const binary = new URL(platform.executable, out);
// Copy next to the target and rename over it: overwriting an executable that macOS has
// already run in place keeps the old code-signature cache for that inode, and the new
// binary is then killed with SIGKILL (exit 137) on its first start.
const staged = new URL(`${platform.executable}.tmp-${process.pid}`, out);
copyFileSync(executable, staged);
chmodSync(staged, 0o755);
renameSync(staged, binary);
console.log(`Built ${platform.key}: ${fileURLToPath(binary)}`);
