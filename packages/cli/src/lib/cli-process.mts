import { spawn, type StdioOptions } from 'node:child_process';

export interface ForwardOptions {
  cwd?: string;
  env?: NodeJS.ProcessEnv;
  stdio?: StdioOptions;
}

// Forward only an explicitly selected native CLI, never an agent or shell.
export function forwardProcess(argv: string[], { cwd, env = process.env, stdio = 'inherit' }: ForwardOptions = {}): Promise<number> {
  return new Promise((resolve, reject) => {
    const child = spawn(argv[0], argv.slice(1), { cwd, env, stdio, shell: false });
    const handlers = new Map<NodeJS.Signals, () => void>((['SIGINT', 'SIGTERM'] as const).map(signal => [signal, () => { child.kill(signal); }]));
    for (const [signal, handler] of handlers) process.on(signal, handler);
    const cleanup = () => { for (const [signal, handler] of handlers) process.off(signal, handler); };
    child.once('error', error => { cleanup(); reject(error); });
    child.once('exit', (code, signal) => {
      cleanup(); resolve(code ?? (signal === 'SIGINT' ? 130 : signal === 'SIGTERM' ? 143 : 1));
    });
  });
}
