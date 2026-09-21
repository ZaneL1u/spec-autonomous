import { StdioOptions } from "node:child_process";
//#region packages/cli/src/lib/cli-process.d.mts
export interface ForwardOptions {
  cwd?: string;
  env?: NodeJS.ProcessEnv;
  stdio?: StdioOptions;
}
export declare function forwardProcess(argv: string[], { cwd, env, stdio }?: ForwardOptions): Promise<number>;
//#endregion