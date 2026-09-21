import { errorCode } from "./errors.mjs";
import { message } from "./locale.mjs";
import { platformFor } from "./platform.mjs";
import { findExecutable, runProcess } from "./provider-process.mjs";
import { withInstallLock } from "./providers.mjs";
import { existsSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { chmod, lstat, mkdir, mkdtemp, readFile, rename, rm } from "node:fs/promises";
import { createHash } from "node:crypto";
import { homedir } from "node:os";
import { join, resolve } from "node:path";
//#region packages/cli/src/lib/git-binary.mts
const facadeRoot = fileURLToPath(new URL("../../../", import.meta.url));
async function gitDelivery(root) {
	if (root === void 0) {
		root = facadeRoot;
		if (fileURLToPath(import.meta.url) !== join(root, "packages/cli/lib/git-binary.mjs")) return null;
	}
	let facade;
	try {
		facade = JSON.parse(await readFile(join(root, "package.json"), "utf8"));
	} catch (error) {
		if (errorCode(error) === "ENOENT" || error instanceof SyntaxError) return null;
		throw error;
	}
	if (facade.private !== true || facade.name !== "spec-autonomous" || facade.bin?.["spec-autonomous"] !== "packages/cli/bin/spec-autonomous.mjs") return null;
	let delivery;
	try {
		delivery = JSON.parse(await readFile(join(root, "git-install.json"), "utf8"));
	} catch {
		throw new Error("git_install_manifest_missing: reinstall the Git package from a published source revision");
	}
	if (delivery.schema_version !== 1 || !/^[0-9]+\.[0-9]+\.[0-9]+(?:-[A-Za-z0-9.-]+)?$/.test(delivery.version) || delivery.version !== facade.version || delivery.tag !== `v${facade.version}` || !/^[A-Za-z0-9_.-]+\/[A-Za-z0-9_.-]+$/.test(delivery.repository) || !delivery.assets || typeof delivery.assets !== "object") throw new Error("git_install_manifest_invalid: repository, version or asset map does not match this package");
	return delivery;
}
function cacheHome(env) {
	const base = process.platform === "win32" ? env.LOCALAPPDATA || join(homedir(), "AppData/Local") : process.platform === "darwin" ? join(homedir(), "Library/Caches") : env.XDG_CACHE_HOME || join(homedir(), ".cache");
	return resolve(env.SPEC_AUTONOMOUS_BINARY_CACHE || join(base, "spec-autonomous/binaries"));
}
async function verified(file, hash) {
	try {
		const metadata = await lstat(file);
		return metadata.isFile() && !metadata.isSymbolicLink() && createHash("sha256").update(await readFile(file)).digest("hex") === hash;
	} catch (error) {
		if (errorCode(error) === "ENOENT") return false;
		throw error;
	}
}
async function ensureGitBinary(delivery, { env = process.env, platform = platformFor(), cache = cacheHome(env), run = runProcess, find = (name) => findExecutable(name, env), log = (text) => {
	process.stderr.write(`[spec-autonomous] ${text}\n`);
} } = {}) {
	const asset = delivery.assets[platform.key];
	if (!asset) throw new Error(`git_binary_unavailable: ${platform.key} has no prebuilt asset in ${delivery.tag}; use a supported platform or build the source checkout`);
	if (!/^[A-Za-z0-9][A-Za-z0-9._-]{0,160}$/.test(asset.name) || !/^[a-f0-9]{64}$/.test(asset.sha256)) throw new Error("git_install_manifest_invalid: invalid asset name or SHA256");
	const directory = join(cache, `${delivery.version}-${platform.key}-${asset.sha256}`);
	const target = join(directory, platform.executable);
	if (await verified(target, asset.sha256)) return target;
	if (env.SPEC_AUTONOMOUS_OFFLINE === "1") throw new Error("git_binary_offline: native binary is not cached; connect once and rerun the CLI");
	const gh = find("gh");
	if (!gh) throw new Error("github_cli_missing: install GitHub CLI (gh), run gh auth login, then rerun spec-autonomous");
	return withInstallLock(`${directory}.lock`, async (onChild) => {
		if (await verified(target, asset.sha256)) return target;
		await mkdir(directory, { recursive: true });
		const temporary = await mkdtemp(join(directory, ".download-"));
		try {
			log(message("log.download", void 0, {
				version: delivery.tag,
				platform: platform.key
			}));
			if ((await run([
				gh,
				"release",
				"download",
				delivery.tag,
				"--repo",
				delivery.repository,
				"--pattern",
				asset.name,
				"--dir",
				temporary
			], {
				env,
				timeout: 18e4,
				output: "log",
				onChild
			})).code !== 0) throw new Error("github_download_failed: check gh auth status and access to the private repository/release, then retry");
			const download = join(temporary, asset.name);
			if (!await verified(download, asset.sha256)) throw new Error("git_binary_integrity_mismatch: downloaded binary does not match the pinned SHA256");
			await chmod(download, 493);
			const version = await run([download, "--version"], {
				env,
				timeout: 15e3,
				onChild
			});
			if (version.code !== 0 || version.stdout.trim() !== `spec-autonomous ${delivery.version}`) throw new Error("git_binary_version_mismatch: downloaded binary has a different version");
			if (existsSync(target)) await rm(target);
			await rename(download, target);
			return target;
		} finally {
			await rm(temporary, {
				recursive: true,
				force: true
			});
		}
	});
}
//#endregion
export { ensureGitBinary, facadeRoot, gitDelivery };
