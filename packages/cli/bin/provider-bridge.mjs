#!/usr/bin/env node
import { errorMessage } from "../lib/errors.mjs";
import { createProviderManager } from "../lib/providers.mjs";
import { spawn } from "node:child_process";
//#region packages/cli/src/bin/provider-bridge.mts
try {
	const manager = createProviderManager();
	const ready = await manager.status("openspec");
	if (!ready.ready) throw new Error("provider_missing: run spec-autonomous providers ensure openspec");
	const command = ready.command;
	const child = spawn(command[0], [...command.slice(1), ...process.argv.slice(2)], {
		env: manager.env,
		stdio: "inherit",
		shell: false
	});
	child.once("error", (error) => {
		console.error(error.message);
		process.exitCode = 1;
	});
	child.once("exit", (code, signal) => {
		process.exitCode = code ?? (signal === "SIGINT" ? 130 : signal === "SIGTERM" ? 143 : 1);
	});
} catch (error) {
	console.error(errorMessage(error));
	process.exitCode = 1;
}
//#endregion
export {};
