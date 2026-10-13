import { npmArgv } from '../packages/cli/src/lib/provider-process.mts';
import { spawnSync } from 'node:child_process';

export function npmCommand(args:string[], options:Record<string, unknown> = {}) {
  // Execute npm's JS entry point with Node; never pass paths through cmd.exe.
  const [node, entry] = npmArgv();
  const result = spawnSync(node, [entry, ...args], { ...options, shell: false } as any);
  if (result.error) throw result.error;
  if (result.status !== 0) throw new Error(`npm exited ${result.status}: ${result.stderr ?? ''}`);
  return result;
}
