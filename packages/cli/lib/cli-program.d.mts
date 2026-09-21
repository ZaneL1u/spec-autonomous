import { ForwardOptions } from "./cli-process.mjs";
import { RunProcess } from "./provider-process.mjs";
import { ProviderContext, createProviderContext } from "./provider-cli.mjs";
import { McpIo } from "./provider-mcp.mjs";
import { Locale } from "./locale.mjs";
import { resolveInitOptions } from "./init-options.mjs";
import { Command } from "commander";
//#region packages/cli/src/lib/cli-program.d.mts
export interface ArgumentDefinition {
  id: string;
  help?: string;
  long?: string;
  short?: string;
  takes_value?: boolean;
  required?: boolean;
  multiple?: boolean;
  hidden?: boolean;
  choices?: string[];
  defaults?: string[];
}
export interface CommandDefinition {
  name: string;
  description?: string;
  aliases?: string[];
  hidden?: boolean;
  arguments: ArgumentDefinition[];
  commands: CommandDefinition[];
}
export interface CliSchema {
  name: string;
  description: string;
  version: string;
  arguments: ArgumentDefinition[];
  commands: CommandDefinition[];
}
export type OptionBag = Record<string, any>;
export interface CreateProgramOptions {
  native: (command: Command, options: OptionBag) => void | Promise<void>;
  providers: (operation: string, options: OptionBag, provider?: string, nativeArgs?: string[]) => void | Promise<void>;
  locale?: Locale;
  stdout?: (text: string) => void;
  stderr?: (text: string) => void;
}
export interface ExecuteCliOptions {
  binary?: string;
  run?: RunProcess;
  forward?: (argv: string[], options?: ForwardOptions) => Promise<number>;
  context?: typeof createProviderContext;
  mcp?: (context: ProviderContext, io?: McpIo) => Promise<number>;
  initialize?: (context: ProviderContext) => Promise<unknown>;
  initOptions?: typeof resolveInitOptions;
  input?: NodeJS.ReadStream;
  promptOutput?: NodeJS.WriteStream;
  stdout?: (text: string) => void;
  stderr?: (text: string) => void;
}
export declare function outputPreferences(argv: string[]): {
  json?: boolean;
  format?: string;
};
export declare function createProgram(schema: CliSchema, { native, providers, locale, stdout, stderr }: CreateProgramOptions): Command;
export declare function runCli(argv: string[], options?: ExecuteCliOptions): Promise<number>;
//#endregion