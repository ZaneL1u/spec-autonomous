#!/usr/bin/env node
import { spawn } from 'node:child_process';
import { resolveBinary } from '../lib/resolve-binary.mjs';

try {
  const child = spawn(resolveBinary(), process.argv.slice(2), { stdio: 'inherit', shell: false });
  const handlers = new Map();
  for (const signal of ['SIGINT', 'SIGTERM']) {
    const handler = () => { child.kill(signal); };
    handlers.set(signal, handler);
    process.on(signal, handler);
  }
  const cleanup = () => {
    for (const [signal, handler] of handlers) process.off(signal, handler);
  };
  child.on('error', (error) => {
    cleanup();
    console.error(`spec-autonomous: ${error.message}`);
    process.exitCode = 1;
  });
  child.on('exit', (code, signal) => {
    cleanup();
    process.exitCode = code ?? (signal === 'SIGINT' ? 130 : signal === 'SIGTERM' ? 143 : 1);
  });
} catch (error) {
  console.error(`spec-autonomous: ${error.message}`);
  process.exitCode = 1;
}
