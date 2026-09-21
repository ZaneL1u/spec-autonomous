import { platformFor } from "./platform.mjs";
import { ensureGitBinary, gitDelivery } from "./git-binary.mjs";
import { createRequire } from "node:module";
import { existsSync } from "node:fs";
import { fileURLToPath } from "node:url";
//#region packages/cli/src/lib/resolve-binary.mts
const require = createRequire(import.meta.url);
async function resolveBinary() {
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
//#endregion
export { resolveBinary };
