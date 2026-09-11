#!/usr/bin/env node
import { spawn } from 'node:child_process';
import { resolveBinary } from '../lib/resolve-binary.mjs';
import { createProviderContext, commandIndex, option, stripOption, bridgeEnvironment, initializeProvider, needsProvider, providerOperation, cliSource } from '../lib/provider-cli.mjs';
import { serveProviderMcp } from '../lib/provider-mcp.mjs';

const help = `Provider setup (npm JS layer; Bun / Node compatible):
  spec-autonomous providers status [openspec|speckit] [--json]
  spec-autonomous providers ensure [openspec|speckit] [--managed] [--json]
  spec-autonomous providers exec <openspec|speckit> -- <native arguments...>
  spec-autonomous init --provider openspec|speckit --agent codex|claude [--mcp]
Missing tools are installed in a user-owned directory. No agents are started.
SPEC_AUTONOMOUS_OFFLINE=1 disables downloads; SPEC_AUTONOMOUS_PROVIDER_HOME overrides the tool directory.`;

try {
  let args = process.argv.slice(2);
  const binary = resolveBinary(), index = commandIndex(args), command = args[index];
  const ownArgs = args.includes('--') ? args.slice(0, args.indexOf('--')) : args;
  const isHelp = ownArgs.includes('--help') || ownArgs.includes('-h');
  if (command === 'providers' && isHelp) { console.log(help); process.exit(0); }
  if (command === 'providers') {
    const context = createProviderContext(binary, args);
    const subcommand = args[index + 1], provider = args[index + 2]?.startsWith('-') ? undefined : args[index + 2];
    const tail = args.slice(index + (provider ? 3 : 2));
    if (subcommand === 'exec') {
      const separator = args.indexOf('--');
      if (!provider || separator < 0 || separator === args.length - 1) throw new Error('invalid_arguments: providers exec <provider> -- <native arguments>');
      const ready = await context.ensureSelected(provider, { allowMissing: true, managed: ownArgs.includes('--managed') });
      const child = spawn(ready.command[0], [...ready.command.slice(1), ...args.slice(separator + 1)], { cwd: context.path, env: context.manager.env, stdio: 'inherit', shell: false });
      process.on('SIGINT', () => child.kill('SIGINT')); process.on('SIGTERM', () => child.kill('SIGTERM'));
      child.once('error', e => { console.error(e.message); process.exitCode = 1; });
      child.once('exit', (code, signal) => { process.exitCode = code ?? (signal === 'SIGINT' ? 130 : signal === 'SIGTERM' ? 143 : 1); });
    } else {
      // Do not silently accept a typo that changes the intended operation.
      for (let i = 0; i < tail.length; i++) {
        if (['--json', '--managed'].includes(tail[i])) continue;
        if (['--path', '--framework'].includes(tail[i])) { i++; continue; }
        if (/^--(path|framework)=/.test(tail[i])) continue;
        throw new Error(`invalid_arguments: ${tail[i]}`);
      }
      const data = await providerOperation(context, { operation: subcommand, provider, managed: args.includes('--managed') });
      console.log(JSON.stringify({ schema_version: 1, data }, null, 2));
    }
  } else if (command === 'mcp' && !isHelp) {
    process.exitCode = await serveProviderMcp(createProviderContext(binary, args));
  } else {
    if (!isHelp && command === 'init') {
      await initializeProvider(createProviderContext(binary, args));
      const provider = option(args, '--provider');
      args = stripOption(args, '--provider');
      if (provider && !option(args, '--framework')) args.push('--framework', provider);
    } else if (!isHelp && needsProvider(command, args.slice(index + 1))) {
      await createProviderContext(binary, args).ensureSource(await cliSource(args));
    }
    if (isHelp && (index < 0 || command === 'init')) console.error(help);
    const child = spawn(binary, args, { env: bridgeEnvironment(), stdio: 'inherit', shell: false });
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
  }
} catch (error) {
  if (process.argv.includes('--json')) console.log(JSON.stringify({ schema_version: 1, error: { code: error.message.split(':')[0], message: error.message } }));
  else console.error(`spec-autonomous: ${error.message}`);
  process.exitCode = 1;
}
