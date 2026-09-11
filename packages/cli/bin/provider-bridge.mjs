#!/usr/bin/env node
// Called only as the Rust adapter's missing-CLI fallback. Installation is done
// before the core operation by the npm CLI/MCP layer, outside native deadlines.
import { spawn } from 'node:child_process';
import { createProviderManager } from '../lib/providers.mjs';
try {
  const manager = createProviderManager();
  const ready = await manager.status('openspec');
  if (!ready.ready) throw new Error('provider_missing: run spec-autonomous providers ensure openspec');
  const child = spawn(ready.command[0], [...ready.command.slice(1), ...process.argv.slice(2)], { env: manager.env, stdio: 'inherit', shell: false });
  child.once('error', error => { console.error(error.message); process.exitCode = 1; });
  child.once('exit', (code, signal) => { process.exitCode = code ?? (signal === 'SIGINT' ? 130 : signal === 'SIGTERM' ? 143 : 1); });
} catch (error) { console.error(error.message); process.exitCode = 1; }
