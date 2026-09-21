#!/usr/bin/env node
import { runCli } from "../lib/cli-program.mjs";
//#region packages/cli/src/bin/spec-autonomous.mts
process.exitCode = await runCli(process.argv.slice(2));
//#endregion
export {};
