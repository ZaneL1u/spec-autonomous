import { existsSync } from 'node:fs';
import { dirname, join, basename } from 'node:path';
import { spawnSync } from 'node:child_process';

export function npmCommand(args, options = {}) {
  // Execute npm's JS entry point with Node; never pass paths through cmd.exe.
  const envEntry = process.env.npm_execpath;
  const candidates = [
    ...(envEntry && basename(envEntry) === 'npm-cli.js' ? [envEntry] : []),
    join(dirname(process.execPath), 'node_modules/npm/bin/npm-cli.js'),
    join(dirname(process.execPath), '../lib/node_modules/npm/bin/npm-cli.js'),
  ];
  const entry = candidates.find(existsSync);
  if (!entry) throw new Error('Cannot locate npm-cli.js beside Node. Use a Node.js installation that includes npm.');
  const result = spawnSync(process.execPath, [entry, ...args], { ...options, shell: false });
  if (result.error) throw result.error;
  if (result.status !== 0) throw new Error(`npm exited ${result.status}: ${result.stderr ?? ''}`);
  return result;
}
