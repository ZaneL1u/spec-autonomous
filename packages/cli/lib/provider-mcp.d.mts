import { ProviderContext } from "./provider-cli.mjs";
//#region packages/cli/src/lib/provider-mcp.d.mts
export interface McpIo {
  input?: NodeJS.ReadableStream & {
    pause(): void;
  };
  output?: NodeJS.WritableStream;
}
export declare function serveProviderMcp(context: ProviderContext, { input, output }?: McpIo): Promise<number>;
//#endregion