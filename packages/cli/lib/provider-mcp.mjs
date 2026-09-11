import { spawn } from 'node:child_process';
import { bridgeEnvironment, mcpNeedsProvider, providerOperation, providerTool } from './provider-cli.mjs';

// A small transport adapter, not a second workflow service: the Rust server
// remains authoritative for protocol validation and every workflow operation.
export async function serveProviderMcp(context, { input = process.stdin, output = process.stdout } = {}) {
  const child = spawn(context.binary, context.args, { env: bridgeEnvironment(context.manager.env), stdio: ['pipe', 'pipe', 'inherit'], shell: false });
  const pending = new Map(); let initialized = false, ready = false, queue = Promise.resolve(), stopped = false;
  const send = value => output.write(JSON.stringify(value) + '\n');
  const failure = (id, message) => send({ jsonrpc: '2.0', id, result: { isError: true, content: [{ type: 'text', text: message }] } });
  function lines(stream, callback, max) {
    let buffer = '';
    stream.setEncoding('utf8');
    stream.on('data', chunk => {
      buffer += chunk;
      let newline;
      while ((newline = buffer.indexOf('\n')) >= 0) {
        const line = buffer.slice(0, newline); buffer = buffer.slice(newline + 1);
        if (Buffer.byteLength(line) > max) { child.kill('SIGTERM'); return; }
        callback(line);
      }
      if (Buffer.byteLength(buffer) > max) child.kill('SIGTERM');
    });
  }
  lines(child.stdout, line => {
    try {
      const response = JSON.parse(line), request = pending.get(response.id);
      pending.delete(response.id);
      if (request?.method === 'initialize' && response.result) initialized = true;
      if (request?.method === 'tools/list' && response.result?.tools) response.result.tools.push(providerTool);
      send(response);
    } catch { child.kill('SIGTERM'); }
  }, 8 * 1024 * 1024);
  lines(input, line => {
    queue = queue.then(async () => {
      if (stopped) return;
      let request;
      try { request = JSON.parse(line); } catch { child.stdin.write(line + '\n'); return; }
      if (request.jsonrpc === '2.0' && request.id === undefined && request.method === 'notifications/initialized' && initialized) ready = true;
      const valid = request.jsonrpc === '2.0' && (typeof request.id === 'string' || typeof request.id === 'number');
      if (valid && ready && request.method === 'tools/call' && request.params?.name === providerTool.name) {
        try {
          const args = request.params.arguments ?? {};
          if (args === null || typeof args !== 'object' || Array.isArray(args) || Object.keys(args).some(k => !['operation', 'provider', 'managed'].includes(k))) throw new Error('invalid_arguments: unexpected provider arguments');
          const data = await providerOperation(context, args), envelope = { schema_version: 1, data };
          send({ jsonrpc: '2.0', id: request.id, result: { content: [{ type: 'text', text: JSON.stringify(envelope) }], structuredContent: envelope, isError: false } });
        } catch (error) { failure(request.id, error.message); }
        return;
      }
      if (valid && ready && mcpNeedsProvider(request)) {
        try {
          const args = request.params.name === 'sa_tools' ? request.params.arguments?.arguments : request.params.arguments;
          await context.ensureSource(args);
        } catch (error) { failure(request.id, error.message); return; }
      }
      if (request.id !== undefined) pending.set(request.id, request);
      child.stdin.write(line + '\n');
    }).catch(error => { process.stderr.write(`spec-autonomous: ${error.message}\n`); child.kill('SIGTERM'); });
  }, 2 * 1024 * 1024);
  input.once('end', () => { queue.finally(() => child.stdin.end()); });
  const interrupt = () => { stopped = true; child.kill('SIGINT'); }, terminate = () => { stopped = true; child.kill('SIGTERM'); };
  process.on('SIGINT', interrupt); process.on('SIGTERM', terminate);
  child.stdin.on('error', () => {});
  return new Promise((resolve, reject) => {
    child.once('error', reject);
    child.once('close', (code, signal) => {
      stopped = true; input.pause();
      process.off('SIGINT', interrupt); process.off('SIGTERM', terminate);
      resolve(code ?? (signal === 'SIGINT' ? 130 : signal === 'SIGTERM' ? 143 : 1));
    });
  });
}
