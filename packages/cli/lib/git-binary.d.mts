import { RunProcess } from "./provider-process.mjs";
import { Platform } from "./platform.mjs";
//#region packages/cli/src/lib/git-binary.d.mts
export interface GitAsset {
  name: string;
  sha256: string;
}
export interface GitDelivery {
  schema_version: number;
  repository: string;
  version: string;
  tag: string;
  assets: Record<string, GitAsset>;
}
export interface EnsureGitBinaryOptions {
  env?: NodeJS.ProcessEnv;
  platform?: Platform;
  cache?: string;
  run?: RunProcess;
  find?: (name: string) => string | null;
  log?: (text: string) => void;
}
export declare const facadeRoot: string;
export declare function gitDelivery(root?: string): Promise<GitDelivery | null>;
export declare function ensureGitBinary(delivery: GitDelivery, { env, platform, cache, run, find, log }?: EnsureGitBinaryOptions): Promise<string>;
//#endregion