import { detectLocale, message } from "./locale.mjs";
import { existsSync } from "node:fs";
import { join } from "node:path";
//#region packages/cli/src/lib/init-options.mts
async function selectInitOption(config, { input = process.stdin, output = process.stderr } = {}) {
	const { select } = await import("@inquirer/prompts");
	const stream = input;
	const controller = new AbortController();
	const end = () => {
		controller.abort();
	};
	const keypress = (_, key) => {
		if (key?.ctrl && key.name === "d") end();
	};
	stream.once("end", end);
	stream.on("keypress", keypress);
	try {
		if (stream.readableEnded) end();
		return await select(config, {
			input: stream,
			output,
			signal: controller.signal
		});
	} finally {
		stream.off("end", end);
		stream.off("keypress", keypress);
	}
}
async function resolveInitOptions(options, { detection, env = process.env, input = process.stdin, output = process.stderr, select = selectInitOption }) {
	const locale = options.lang || detectLocale();
	const structured = options.json || options.format && options.format !== "human";
	const terminal = Boolean(input.isTTY && output.isTTY);
	if (options.interactive && (structured || options.nonInteractive || options.yes)) throw new Error("init_mode_conflict: interactive conflicts with structured, non-interactive or yes");
	if (options.interactive && !terminal) throw new Error("init_terminal_required: interactive init needs a terminal");
	const interactive = !structured && !options.nonInteractive && !options.yes && terminal && (options.interactive || !env.CI || env.CI === "false" || env.CI === "0");
	const report = await detection();
	let provider = options.provider || (options.framework !== "auto" ? options.framework : void 0) || report.selected || void 0;
	let agent = options.agent;
	let prompted = Boolean(options.interactive);
	const ask = async (key, choices, defaultValue) => {
		prompted = true;
		return select({
			message: message(key, locale),
			choices,
			default: defaultValue,
			theme: { style: { keysHelpTip: () => message("init.keys", locale) } }
		}, {
			input,
			output
		});
	};
	if (!provider) {
		if (interactive) provider = await ask("init.choose_provider", (report.detected.length ? report.detected.map((d) => d.framework) : ["openspec", "speckit"]).map((value) => ({
			value,
			name: value === "openspec" ? "OpenSpec" : "Spec Kit"
		})), report.selected || "openspec");
		else if (options.yes && !report.detected.length && !report.warnings?.length) provider = "openspec";
		else throw new Error("provider_selection_required: choose openspec or speckit");
	}
	if (!provider || !["openspec", "speckit"].includes(provider)) throw new Error("provider_selection_required: choose openspec or speckit");
	if (!report.detected.some((d) => d.framework === provider) && (report.detected.length || report.warnings?.length)) throw new Error("provider_init_conflict: preserve existing or incomplete native setup");
	if (!agent) {
		const codex = existsSync(join(report.root, ".agents"));
		if (codex !== existsSync(join(report.root, ".claude"))) agent = codex ? "codex" : "claude";
		else if (interactive) agent = await ask("init.choose_agent", [{
			value: "codex",
			name: "Codex"
		}, {
			value: "claude",
			name: "Claude Code"
		}], "codex");
		else if (options.yes) agent = "codex";
		else throw new Error("host_selection_required: choose --agent codex|claude");
	}
	let mcp = options.mcp;
	if (mcp === void 0) {
		if (interactive && prompted) mcp = await ask("init.choose_mcp", [{
			value: true,
			name: message("init.mcp_yes", locale)
		}, {
			value: false,
			name: message("init.mcp_no", locale)
		}], true);
		else mcp = Boolean(options.yes);
	}
	return {
		...options,
		path: report.root,
		framework: provider,
		provider,
		agent,
		mcp
	};
}
//#endregion
export { resolveInitOptions, selectInitOption };
