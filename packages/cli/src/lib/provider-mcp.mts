import { spawn } from 'node:child_process';
import { bridgeEnvironment, mcpNeedsProvider, providerOperation, providerTool, type ProviderContext } from './provider-cli.mts';
import { errorMessage } from './errors.mts';

type JsonRpcId = string | number;

interface JsonRpcEnvelope {
  jsonrpc?: string;
  id?: JsonRpcId;
  method?: string;
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  params?: { name?: string; arguments?: Record<string, any> };
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  result?: { tools?: unknown[]; [key: string]: any };
}

export interface McpIo {
  input?: NodeJS.ReadableStream & { pause(): void };
  output?: NodeJS.WritableStream;
}

// A small transport adapter, not a second workflow service: the Rust server
// remains authoritative for protocol validation and every workflow operation.
export async function serveProviderMcp(context: ProviderContext, { input = process.stdin, output = process.stdout }: McpIo = {}): Promise<number> {
  const child = spawn(context.binary, context.args, { env: bridgeEnvironment(context.manager.env), stdio: ['pipe', 'pipe', 'inherit'], shell: false });
  const pending = new Map<JsonRpcId, JsonRpcEnvelope>(); let initialized = false, ready = false, queue: Promise<void> = Promise.resolve(), stopped = false;
  const send = (value: unknown) => output.write(JSON.stringify(value) + '\n');
  const failure = (id: JsonRpcId | undefined, text: string) => send({ jsonrpc: '2.0', id, result: { isError: true, content: [{ type: 'text', text }] } });
  function lines(stream: NodeJS.ReadableStream, callback: (line: string) => void, max: number) {
    let buffer = '';
    stream.setEncoding('utf8');
    stream.on('data', chunk => {
      buffer += chunk;
      let newline: number;
      while ((newline = buffer.indexOf('\n')) >= 0) {
        const line = buffer.slice(0, newline); buffer = buffer.slice(newline + 1);
        if (Buffer.byteLength(line) > max) { child.kill('SIGTERM'); return; }
        callback(line);
      }
      if (Buffer.byteLength(buffer) > max) child.kill('SIGTERM');
    });
  }
  lines(child.stdout!, line => {
    try {
      const response = JSON.parse(line) as JsonRpcEnvelope, request = response.id === undefined ? undefined : pending.get(response.id);
      if (response.id !== undefined) pending.delete(response.id);
      if (request?.method === 'initialize' && response.result) initialized = true;
      if (request?.method === 'tools/list' && response.result?.tools) response.result.tools.push(providerTool);
      send(response);
    } catch { child.kill('SIGTERM'); }
  }, 8 * 1024 * 1024);
  lines(input, line => {
    queue = queue.then(async () => {
      if (stopped) return;
      let request: JsonRpcEnvelope;
      try { request = JSON.parse(line) as JsonRpcEnvelope; } catch { child.stdin!.write(line + '\n'); return; }
      if (request.jsonrpc === '2.0' && request.id === undefined && request.method === 'notifications/initialized' && initialized) ready = true;
      const valid = request.jsonrpc === '2.0' && (typeof request.id === 'string' || typeof request.id === 'number');
      if (valid && ready && request.method === 'tools/call' && request.params?.name === providerTool.name) {
        try {
          const args = request.params.arguments ?? {};
          if (args === null || typeof args !== 'object' || Array.isArray(args) || Object.keys(args).some(k => !['operation', 'provider', 'managed'].includes(k))) throw new Error('invalid_arguments: unexpected provider arguments');
          const data = await providerOperation(context, args), envelope = { schema_version: 1, data };
          send({ jsonrpc: '2.0', id: request.id, result: { content: [{ type: 'text', text: JSON.stringify(envelope) }], structuredContent: envelope, isError: false } });
        } catch (error) { failure(request.id, errorMessage(error)); }
        return;
      }
      if (valid && ready && mcpNeedsProvider(request)) {
        try {
          const args = request.params!.name === 'sa_tools' ? request.params!.arguments?.arguments : request.params!.arguments;
          await context.ensureSource(args);
        } catch (error) { failure(request.id, errorMessage(error)); return; }
      }
      if (request.id !== undefined) pending.set(request.id, request);
      child.stdin!.write(line + '\n');
    }).catch(error => { process.stderr.write(`spec-autonomous: ${errorMessage(error)}\n`); child.kill('SIGTERM'); });
  }, 2 * 1024 * 1024);
  input.once('end', () => { queue.finally(() => child.stdin!.end()); });
  const interrupt = () => { stopped = true; child.kill('SIGINT'); }, terminate = () => { stopped = true; child.kill('SIGTERM'); };
  process.on('SIGINT', interrupt); process.on('SIGTERM', terminate);
  child.stdin!.on('error', () => {});
  return new Promise((resolve, reject) => {
    child.once('error', reject);
    child.once('close', (code, signal) => {
      stopped = true; input.pause();
      process.off('SIGINT', interrupt); process.off('SIGTERM', terminate);
      resolve(code ?? (signal === 'SIGINT' ? 130 : signal === 'SIGTERM' ? 143 : 1));
    });
  });
}
