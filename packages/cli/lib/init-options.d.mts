import { DetectionReport } from "./provider-cli.mjs";
import { Locale } from "./locale.mjs";
//#region packages/cli/src/lib/init-options.d.mts
export interface SelectChoice<T> {
  value: T;
  name?: string;
}
export interface SelectConfig<T> {
  message: string;
  choices: readonly SelectChoice<T>[];
  default?: T;
  theme?: {
    style?: {
      keysHelpTip?: (...args: unknown[]) => string;
    };
  };
}
export interface PromptIo {
  input?: {
    isTTY?: boolean;
  } & Partial<NodeJS.ReadStream>;
  output?: {
    isTTY?: boolean;
  } & Partial<NodeJS.WriteStream>;
}
export type SelectPrompt = (config: SelectConfig<any>, io?: PromptIo) => Promise<any>;
export interface InitOptions {
  lang?: Locale;
  json?: boolean;
  format?: string;
  interactive?: boolean;
  nonInteractive?: boolean;
  yes?: boolean;
  provider?: string;
  framework?: string;
  agent?: string;
  mcp?: boolean;
  path?: string;
  [key: string]: unknown;
}
export interface ResolveInitIo extends PromptIo {
  detection: () => Promise<DetectionReport>;
  env?: NodeJS.ProcessEnv;
  select?: SelectPrompt;
}
export declare function selectInitOption(config: SelectConfig<any>, { input, output }?: PromptIo): Promise<any>;
export declare function resolveInitOptions(options: InitOptions, { detection, env, input, output, select }: ResolveInitIo): Promise<InitOptions>;
//#endregion