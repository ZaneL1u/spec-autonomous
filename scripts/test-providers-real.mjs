// Explicit network acceptance gate. It never runs as part of offline unit tests.
import { mkdir, mkdtemp, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { createProviderManager } from '../packages/cli/lib/providers.mjs';
import { createProviderContext, initializeProvider } from '../packages/cli/lib/provider-cli.mjs';
import { runProcess } from '../packages/cli/lib/provider-process.mjs';

if (!process.argv.includes('--run')) throw new Error('This gate downloads real upstream tools. Use --run to execute it.');
const root = fileURLToPath(new URL('../', import.meta.url));
const sandbox = await mkdtemp(join(tmpdir(), 'sa native providers real '));
const home = join(sandbox, 'providers');
const env = { ...process.env, SPEC_AUTONOMOUS_PROVIDER_HOME: home, UV_PYTHON_PREFERENCE: 'only-managed' };
delete env.SPEC_AUTONOMOUS_OFFLINE;
const manager = createProviderManager({ env, home, find: () => null });
const binary = resolve(root, 'target/debug', process.platform === 'win32' ? 'spec-autonomous.exe' : 'spec-autonomous');
const results = [];
console.log(`Real provider sandbox: ${sandbox}`);
for (const provider of ['openspec', 'speckit']) {
  const project = join(sandbox, provider); await mkdir(project);
  await runProcess(['git', 'init', '-q', project]);
  const context = createProviderContext(binary, { path: project, provider, agent: 'codex' }, manager);
  const ready = await initializeProvider(context);
  const detect = await context.detection();
  if (detect.selected !== provider) throw new Error(`${provider}: native scaffolding was not detected`);
  const bind = await runProcess([binary, '--path', project, 'init', '--agent', 'codex', '--json'], { env });
  if (bind.code !== 0) throw new Error(`${provider}: binding failed: ${bind.stderr} ${bind.stdout}`);
  const repeated = await initializeProvider(context);
  if (ready.command.join() !== repeated.command.join()) throw new Error('Repeated init did not reuse provider');
  results.push({ provider, project, ready, detection: detect, binding: JSON.parse(bind.stdout) });
}
const report = { platform: `${process.platform}-${process.arch}`, runtime: process.versions.bun ? `bun ${process.versions.bun}` : `node ${process.version}`, sandbox, forced_missing_path_tools: true, python_preference: env.UV_PYTHON_PREFERENCE, results };
await mkdir(join(root, '.artifacts'), { recursive: true });
await writeFile(join(root, '.artifacts/provider-bootstrap-real.json'), JSON.stringify(report, null, 2) + '\n');
console.log(JSON.stringify(report, null, 2));
