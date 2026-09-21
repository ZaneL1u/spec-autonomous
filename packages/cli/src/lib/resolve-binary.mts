import { existsSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { createRequire } from 'node:module';
import { platformFor } from './platform.mts';
import { gitDelivery, ensureGitBinary } from './git-binary.mts';

const require = createRequire(import.meta.url);

export async function resolveBinary(): Promise<string> {
  // Explicit opt-in for source development and test fixtures only.
  if (process.env.SPEC_AUTONOMOUS_BINARY) return process.env.SPEC_AUTONOMOUS_BINARY;
  const platform = platformFor();
  const local = fileURLToPath(new URL(`../native/${platform.key}/${platform.executable}`, import.meta.url));
  if (existsSync(local)) return local;
  try {
    return require.resolve(`spec-autonomous-${platform.key}/bin/${platform.executable}`);
  } catch {
    const delivery = await gitDelivery();
    if (delivery) return ensureGitBinary(delivery, { platform });
    throw new Error(`Native binary for ${platform.key} is missing. Install with optional dependencies enabled. For a source checkout, run bun run build:native first.`);
  }
}
