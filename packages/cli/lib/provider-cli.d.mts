import { ProviderManager, ProviderName, ProviderStatus } from "./providers.mjs";
//#region packages/cli/src/lib/provider-cli.d.mts
export interface DetectedFramework {
  framework: string;
}
export interface DetectionReport {
  root: string;
  selected?: ProviderName | null;
  detected: DetectedFramework[];
  warnings?: string[];
  provider_commands?: Record<string, string[]>;
}
export interface ProviderSelection {
  provider: ProviderName;
  root: string;
  report: DetectionReport;
}
export interface NativeAction {
  name: string;
  arguments?: Record<string, any>;
}
export interface ParsedCli {
  command: NativeAction;
  framework?: string;
}
export interface SourceRef {
  run_id?: string;
  framework?: string;
  result?: {
    run_id?: string;
  };
  [key: string]: unknown;
}
export interface ProviderContextOptions {
  path?: string;
  provider?: string;
  framework?: string;
  lang?: string;
  agent?: string;
  nativeArgs?: string[];
  [key: string]: unknown;
}
export interface EnsureSelectedOptions {
  allowMissing?: boolean;
  managed?: boolean;
}
export interface ProviderContext {
  binary: string;
  args: string[];
  options: ProviderContextOptions;
  manager: ProviderManager;
  path: string;
  explicit?: string;
  detection: () => Promise<DetectionReport>;
  select: (requested?: string, allowMissing?: boolean) => Promise<ProviderSelection>;
  ensureSelected: (requested?: string, options?: EnsureSelectedOptions) => Promise<ProviderStatus>;
  ensureSource: (source?: SourceRef) => Promise<ProviderStatus>;
}
export interface ProviderReport {
  root: string;
  selected?: ProviderName | null;
  detected: DetectedFramework[];
  home: string;
  providers: ProviderStatus[];
}
export interface JsonRpcMessage {
  method?: string;
  params?: {
    name?: string;
    arguments?: Record<string, any>;
  };
}
export declare function bridgeEnvironment(env?: NodeJS.ProcessEnv): NodeJS.ProcessEnv;
export declare function createProviderContext(binary: string, options?: ProviderContextOptions, manager?: ProviderManager): ProviderContext;
export declare function nativeAction(parsed: ParsedCli): NativeAction;
export declare function cliSource(parsed: ParsedCli): Promise<SourceRef>;
export declare function mergeScaffold(source: string, target: string): Promise<string[]>;
export declare function initializeProvider(context: ProviderContext): Promise<ProviderStatus>;
export declare function providerOperation(context: ProviderContext, { operation, provider, managed }?: {
  operation?: string;
  provider?: string;
  managed?: boolean;
}): Promise<ProviderStatus | ProviderReport>;
export declare const providerTool: {
  name: string;
  description: string;
  inputSchema: {
    type: string;
    additionalProperties: boolean;
    properties: {
      operation: {
        type: string;
        enum: string[];
        default: string;
      };
      provider: {
        type: string;
        enum: ProviderName[];
      };
      managed: {
        type: string;
        default: boolean;
      };
    };
  };
  annotations: {
    readOnlyHint: boolean;
    destructiveHint: boolean;
    openWorldHint: boolean;
  };
};
export declare function needsProvider(command: string, capability?: string): boolean;
export declare function mcpNeedsProvider(message: JsonRpcMessage): boolean;
//#endregion