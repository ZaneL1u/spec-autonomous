import { errorMessage } from "./errors.mjs";
import { bridgeEnvironment, mcpNeedsProvider, providerOperation, providerTool } from "./provider-cli.mjs";
import { spawn } from "node:child_process";
//#region packages/cli/src/lib/provider-mcp.mts
async function serveProviderMcp(context, { input = process.stdin, output = process.stdout } = {}) {
	const child = spawn(context.binary, context.args, {
		env: bridgeEnvironment(context.manager.env),
		stdio: [
			"pipe",
			"pipe",
			"inherit"
		],
		shell: false
	});
	const pending = /* @__PURE__ */ new Map();
	let initialized = false, ready = false, queue = Promise.resolve(), stopped = false;
	const send = (value) => output.write(JSON.stringify(value) + "\n");
	const failure = (id, text) => send({
		jsonrpc: "2.0",
		id,
		result: {
			isError: true,
			content: [{
				type: "text",
				text
			}]
		}
	});
	function lines(stream, callback, max) {
		let buffer = "";
		stream.setEncoding("utf8");
		stream.on("data", (chunk) => {
			buffer += chunk;
			let newline;
			while ((newline = buffer.indexOf("\n")) >= 0) {
				const line = buffer.slice(0, newline);
				buffer = buffer.slice(newline + 1);
				if (Buffer.byteLength(line) > max) {
					child.kill("SIGTERM");
					return;
				}
				callback(line);
			}
			if (Buffer.byteLength(buffer) > max) child.kill("SIGTERM");
		});
	}
	lines(child.stdout, (line) => {
		try {
			const response = JSON.parse(line), request = response.id === void 0 ? void 0 : pending.get(response.id);
			if (response.id !== void 0) pending.delete(response.id);
			if (request?.method === "initialize" && response.result) initialized = true;
			if (request?.method === "tools/list" && response.result?.tools) response.result.tools.push(providerTool);
			send(response);
		} catch {
			child.kill("SIGTERM");
		}
	}, 8388608);
	lines(input, (line) => {
		queue = queue.then(async () => {
			if (stopped) return;
			let request;
			try {
				request = JSON.parse(line);
			} catch {
				child.stdin.write(line + "\n");
				return;
			}
			if (request.jsonrpc === "2.0" && request.id === void 0 && request.method === "notifications/initialized" && initialized) ready = true;
			const valid = request.jsonrpc === "2.0" && (typeof request.id === "string" || typeof request.id === "number");
			if (valid && ready && request.method === "tools/call" && request.params?.name === providerTool.name) {
				try {
					const args = request.params.arguments ?? {};
					if (args === null || typeof args !== "object" || Array.isArray(args) || Object.keys(args).some((k) => ![
						"operation",
						"provider",
						"managed"
					].includes(k))) throw new Error("invalid_arguments: unexpected provider arguments");
					const envelope = {
						schema_version: 1,
						data: await providerOperation(context, args)
					};
					send({
						jsonrpc: "2.0",
						id: request.id,
						result: {
							content: [{
								type: "text",
								text: JSON.stringify(envelope)
							}],
							structuredContent: envelope,
							isError: false
						}
					});
				} catch (error) {
					failure(request.id, errorMessage(error));
				}
				return;
			}
			if (valid && ready && mcpNeedsProvider(request)) try {
				const args = request.params.name === "sa_tools" ? request.params.arguments?.arguments : request.params.arguments;
				await context.ensureSource(args);
			} catch (error) {
				failure(request.id, errorMessage(error));
				return;
			}
			if (request.id !== void 0) pending.set(request.id, request);
			child.stdin.write(line + "\n");
		}).catch((error) => {
			process.stderr.write(`spec-autonomous: ${errorMessage(error)}\n`);
			child.kill("SIGTERM");
		});
	}, 2097152);
	input.once("end", () => {
		queue.finally(() => child.stdin.end());
	});
	const interrupt = () => {
		stopped = true;
		child.kill("SIGINT");
	}, terminate = () => {
		stopped = true;
		child.kill("SIGTERM");
	};
	process.on("SIGINT", interrupt);
	process.on("SIGTERM", terminate);
	child.stdin.on("error", () => {});
	return new Promise((resolve, reject) => {
		child.once("error", reject);
		child.once("close", (code, signal) => {
			stopped = true;
			input.pause();
			process.off("SIGINT", interrupt);
			process.off("SIGTERM", terminate);
			resolve(code ?? (signal === "SIGINT" ? 130 : signal === "SIGTERM" ? 143 : 1));
		});
	});
}
//#endregion
export { serveProviderMcp };
