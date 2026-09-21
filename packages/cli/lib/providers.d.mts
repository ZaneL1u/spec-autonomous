import { ProcessResult, RunOptions } from "./provider-process.mjs";
import { Platform } from "./platform.mjs";
//#region packages/cli/src/lib/providers.d.mts
export type ProviderName = 'openspec' | 'speckit';
export type ProviderSource = 'project' | 'path' | 'configured' | 'managed' | 'missing';
export interface ProbeResult {
  ready: boolean;
  version?: string;
  reason?: string;
}
export interface ProviderStatus extends ProbeResult {
  provider: ProviderName;
  source: ProviderSource;
  command?: string[];
}
export interface StatusOptions {
  root?: string;
  managed?: boolean;
  command?: string[];
}
export interface ProviderManager {
  home: string;
  env: NodeJS.ProcessEnv;
  run: (argv: string[], options?: RunOptions) => Promise<ProcessResult>;
  status: (name: ProviderName, options?: StatusOptions) => Promise<ProviderStatus>;
  ensure: (name: ProviderName, options?: StatusOptions) => Promise<ProviderStatus>;
  managed: (name: ProviderName) => Promise<ProviderStatus | null>;
}
export interface ProviderManagerOptions {
  env?: NodeJS.ProcessEnv;
  home?: string;
  run?: (argv: string[], options?: RunOptions) => Promise<ProcessResult>;
  find?: (name: string) => string | null;
  platform?: () => Platform;
  download?: typeof downloadVerified;
  log?: (text: string) => void;
}
export declare const providerNames: ProviderName[];
export declare function providerName(value: unknown): ProviderName;
export declare function providerHome(env?: NodeJS.ProcessEnv): string;
export declare function withInstallLock<T>(directory: string, action: (onChild: (pid: number | null) => void) => Promise<T>, { timeout }?: {
  timeout?: number;
}): Promise<T>;
export declare function downloadVerified(url: string, destination: string, expected: string, fetcher?: typeof fetch): Promise<void>;
export declare function createProviderManager({ env, home, run, find, platform, download, log }?: ProviderManagerOptions): ProviderManager;
//#endregion