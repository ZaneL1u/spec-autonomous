//#region packages/cli/src/lib/provider-process.d.mts
export interface ProcessResult {
  code: number;
  stdout: string;
  stderr: string;
}
export interface RunOptions {
  cwd?: string;
  env?: NodeJS.ProcessEnv;
  timeout?: number;
  output?: 'capture' | 'log';
  onChild?: (pid: number | null) => void;
}
export type RunProcess = (argv: string[], options?: RunOptions) => Promise<ProcessResult>;
export declare function findExecutable(name: string, env?: NodeJS.ProcessEnv): string | null;
export declare function nodeCommand(env?: NodeJS.ProcessEnv): string;
export declare function npmArgv(env?: NodeJS.ProcessEnv): string[];
export declare function runProcess(argv: string[], { cwd, env, timeout, output, onChild }?: RunOptions): Promise<ProcessResult>;
//#endregion