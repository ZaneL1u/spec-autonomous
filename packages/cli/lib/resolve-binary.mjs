import { existsSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { createRequire } from 'node:module';
import { platformFor } from './platform.mjs';

const require = createRequire(import.meta.url);

export function resolveBinary() {
  // Explicit opt-in for source development and test fixtures only.
  if (process.env.SPEC_AUTONOMOUS_BINARY) return process.env.SPEC_AUTONOMOUS_BINARY;
  const platform = platformFor();
  const local = fileURLToPath(new URL(`../native/${platform.key}/${platform.executable}`, import.meta.url));
  if (existsSync(local)) return local;
  try {
    return require.resolve(`spec-autonomous-${platform.key}/bin/${platform.executable}`);
  } catch {
    throw new Error(`Native binary for ${platform.key} is missing. Install with optional dependencies enabled. For a source checkout, run bun run build first.`);
  }
}
