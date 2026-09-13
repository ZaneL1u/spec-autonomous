import { Command, Option, Argument, CommanderError } from 'commander';
import { resolveBinary } from './resolve-binary.mjs';
import { runProcess } from './provider-process.mjs';
import { forwardProcess } from './cli-process.mjs';
import { createProviderContext, bridgeEnvironment, initializeProvider, needsProvider, providerOperation, cliSource, nativeAction } from './provider-cli.mjs';
import { serveProviderMcp } from './provider-mcp.mjs';
import { resolveInitOptions } from './init-options.mjs';
import { detectLocale, languageArg, localizeError, message, withLocale } from './locale.mjs';

function syntax(message) { return new CommanderError(2, 'invalid_arguments', message); }

// A framework parser also selects error output when later validation fails.
// Commander owns the -- delimiter, so provider payload flags stay opaque.
export function outputPreferences(argv) {
  const parser = new Command().helpOption(false).allowUnknownOption().allowExcessArguments()
    .option('--json').option('--format <format>').exitOverride().configureOutput({ writeErr: () => {} });
  try { parser.parse(argv, { from: 'user' }); } catch {}
  return parser.opts();
}

function selectedOptions(command) {
  const selected = {};
  for (let current = command; current; current = current.parent) {
    for (const [key, value] of Object.entries(current.opts())) {
      if (current.getOptionValueSource(key) !== 'cli') continue;
      if (key in selected) throw syntax(`option '${key}' was supplied more than once`);
      selected[key] = value;
    }
  }
  return selected;
}

function addDefinition(command, definition, locale) {
  const description = (definition.help || '') + (definition.takes_value && definition.defaults?.length ? ` (${message('help.default', locale)}: ${definition.defaults.join(', ')})` : '');
  if (!definition.long && !definition.short) {
    const name = definition.id + (definition.multiple ? '...' : '');
    const argument = new Argument(definition.required ? `<${name}>` : `[${name}]`, description);
    if (definition.choices?.length) argument.choices(definition.choices);
    command.addArgument(argument);
    return;
  }
  const flags = [definition.short && `-${definition.short}`, definition.long && `--${definition.long}`].filter(Boolean).join(', ')
    + (definition.takes_value ? ` <${definition.id}>` : '');
  const option = new Option(flags, description);
  if (definition.choices?.length) option.choices(definition.choices);
  if (definition.required) option.makeOptionMandatory();
  if (definition.hidden) option.hideHelp();
  command.addOption(option);
}

export function createProgram(schema, { native, providers, locale = detectLocale(), stdout = text => process.stdout.write(text), stderr = text => process.stderr.write(text) }) {
  function configure(command) {
    return command.enablePositionalOptions().exitOverride()
      .helpOption('-h, --help', message('option.help', locale))
      .configureOutput({ writeOut: stdout, writeErr: stderr })
      .configureHelp(locale === 'zh-CN' ? {
        styleTitle: title => ({ 'Usage:': '用法：', 'Options:': '选项：', 'Arguments:': '参数：', 'Commands:': '命令：', 'Global Options:': '全局选项：' })[title] || title,
        styleOptionText: value => value === '[options]' ? '[选项]' : value,
        styleSubcommandText: value => value === '[command]' ? '[命令]' : value,
        optionDescription: option => option.description + (option.argChoices ? `（可选值：${option.argChoices.join(', ')}）` : ''),
        argumentDescription: argument => argument.description + (argument.argChoices ? `（可选值：${argument.argChoices.join(', ')}）` : ''),
      } : {})
      .showSuggestionAfterError().showHelpAfterError(message('help.hint', locale));
  }
  const program = configure(new Command(schema.name)).description(schema.description)
    .version(schema.version, '-V, --version', message('option.version', locale));
  for (const arg of schema.arguments) addDefinition(program, arg, locale);
  function register(parent, definition) {
    const command = configure(new Command(definition.name)).description(definition.description || '');
    if (definition.aliases?.length) command.aliases(definition.aliases);
    for (const arg of definition.arguments) {
      addDefinition(command, definition.name === 'init' && arg.id === 'agent' ? { ...arg, choices: ['codex', 'claude'] } : arg, locale);
    }
    if (definition.name === 'init') {
      command.addOption(new Option('--provider <provider>', message('option.provider', locale)).choices(['openspec', 'speckit']));
      command.option('--no-mcp', message('arg.no_mcp', locale));
    }
    for (const subcommand of definition.commands) register(command, subcommand);
    command.action(function () { return native(this, selectedOptions(this)); });
    parent.addCommand(command, { hidden: definition.hidden });
  }
  for (const command of schema.commands) register(program, command);

  const providerGroup = configure(new Command('providers')).description(message('command.providers', locale));
  const globals = schema.arguments.filter(arg => ['path', 'framework', 'json', 'format', 'lang'].includes(arg.id));
  const providerGlobals = command => {
    for (const arg of globals) addDefinition(command, arg.id === 'format' ? { ...arg, choices: ['json'] } : arg, locale);
    return command;
  };
  providerGlobals(providerGroup);
  for (const name of ['status', 'ensure', 'exec']) {
    const command = providerGlobals(configure(new Command(name)))
      .description({ status: message('command.providers_status', locale), ensure: message('command.providers_ensure', locale), exec: message('command.providers_exec', locale) }[name])
      .addArgument(new Argument(name === 'exec' ? '<provider>' : '[provider]', message('help.provider', locale)).choices(['openspec', 'speckit']))
      .option('--managed', message('option.managed', locale));
    if (name === 'exec') command.argument('<args...>', message('help.native_args', locale));
    command.action(function (provider, ...rest) {
      return providers(name, selectedOptions(this), provider, name === 'exec' ? rest[0] : []);
    });
    providerGroup.addCommand(command);
  }
  providerGroup.action(function () { return providers('status', selectedOptions(this)); });
  program.addCommand(providerGroup);
  program.addCommand(configure(new Command('help')).description(message('command.help', locale))
    .argument('[commands...]', message('help.command_path', locale))
      .action(names => {
      let selected = program;
      for (const name of names) {
        selected = selected.commands.find(command => command.name() === name || command.aliases().includes(name));
        if (!selected) throw syntax(`${message('error.unknown_command', locale)}: ${name}`);
      }
      selected.outputHelp();
    }));
  return program;
}

function initArguments(schema, options) {
  const argv = ['init'];
  const init = schema.commands.find(c => c.name === 'init');
  const seen = new Set();
  for (const definition of [...schema.arguments, ...init.arguments]) {
    if (!definition.long || seen.has(definition.long)) continue;
    seen.add(definition.long);
    const key = new Option(`--${definition.long}`).attributeName();
    const value = options[key];
    if (value !== undefined && value !== false) argv.push(`--${definition.long}`, ...(definition.takes_value ? [String(value)] : []));
  }
  if (options.provider) {
    if (options.framework && options.framework !== 'auto' && options.framework !== options.provider) throw syntax('--provider and --framework disagree');
    if (!options.framework || options.framework === 'auto') return initArguments(schema, { ...options, provider: undefined, framework: options.provider });
  }
  return argv;
}

async function executeCli(argv, locale, { binary: explicitBinary, run = runProcess, forward = forwardProcess,
  context = createProviderContext, mcp = serveProviderMcp, initialize = initializeProvider,
  initOptions = resolveInitOptions, input = process.stdin, promptOutput = process.stderr,
  stdout = text => process.stdout.write(text), stderr = text => process.stderr.write(text) } = {}) {
  const preferences = outputPreferences(argv);
  let diagnostic = '', exitCode = 0;
  try {
    const binary = explicitBinary || await resolveBinary();
    const described = await run([binary, '--lang', locale, 'cli-metadata', 'describe'], { timeout: 15_000 });
    if (described.code !== 0) throw new Error(`cli_metadata_failed: ${described.stderr}`);
    const schema = JSON.parse(described.stdout).data;
    if (!schema?.commands || schema.name !== 'spec-autonomous') throw new Error('cli_metadata_invalid: install matching JS and native package versions');
    const preflight = async args => {
      const result = await run([binary, '--lang', locale, 'cli-metadata', 'parse', '--', ...args], { timeout: 15_000 });
      if (result.code !== 0) throw new Error(`cli_preflight_failed: ${result.stderr}`);
      const checked = JSON.parse(result.stdout);
      if (!checked.ok) {
        if (checked.exit_code === 0) { stdout(checked.stdout); return null; }
        throw syntax(checked.stderr.trim());
      }
      return checked.parsed;
    };
    const program = createProgram(schema, { locale, stdout, stderr: text => { diagnostic += text; },
      native: async (command, options) => {
        let args = command.name() === 'init' ? initArguments(schema, options) : argv;
        let parsed = await preflight(args);
        if (!parsed) return;
        let ctx = context(binary, { ...options, ...parsed, provider: options.provider, lang: locale, nativeArgs: args });
        if (parsed.command.name === 'init' && !parsed.command.arguments.check) {
          // Fully validate the original argv before prompting. Rebuild from the
          // resolved values so inferred choices reach native initialization too.
          options = await initOptions({ ...options, lang: locale }, { detection: () => ctx.detection(), input, output: promptOutput });
          args = initArguments(schema, options);
          parsed = await preflight(args);
          ctx = context(binary, { ...options, ...parsed, provider: options.provider, lang: locale, nativeArgs: args });
          const checked = await run([binary, '--path', options.path, 'init', '--agent', options.agent, '--prefix', options.prefix || '', ...(options.mcp ? ['--mcp'] : []), '--check', '--json'], { timeout: 15000 });
          const result = JSON.parse(checked.stdout || '{}');
          if (checked.code !== 0) throw new Error(`${result.error?.code || 'init_preflight_failed'}: ${result.error?.message || checked.stderr}`);
          await initialize(ctx);
        }
        else if (parsed.command.name === 'mcp') { exitCode = await mcp(ctx); return; }
        else if (needsProvider(parsed.command.name, nativeAction(parsed).arguments?.capability)) await ctx.ensureSource(await cliSource(parsed));
        exitCode = await forward([binary, ...args], { env: { ...bridgeEnvironment(), SPEC_AUTONOMOUS_LANG: locale } });
      },
      providers: async (operation, options, provider, nativeArgs = []) => {
        for (const key of Object.keys(options)) if (!['path', 'framework', 'json', 'format', 'lang', 'managed'].includes(key)) throw syntax(`option '${key}' is only available for native capabilities`);
        if (options.format && options.format !== 'json') throw syntax('providers supports --format json');
        if (provider && options.framework && options.framework !== 'auto' && provider !== options.framework) throw syntax('provider argument and --framework disagree');
        const ctx = context(binary, { ...options, lang: locale });
        if (operation === 'exec') {
          const ready = await ctx.ensureSelected(provider, { allowMissing: true, managed: Boolean(options.managed) });
          exitCode = await forward([...ready.command, ...nativeArgs], { cwd: ctx.path, env: ctx.manager.env });
        } else {
          const data = await providerOperation(ctx, { operation, provider, managed: Boolean(options.managed) });
          stdout(JSON.stringify({ schema_version: 1, data }, null, 2) + '\n');
        }
      },
    });
    if (argv.length === 0) { program.outputHelp(); return 0; }
    await program.parseAsync(argv, { from: 'user' });
    return exitCode;
  } catch (error) {
    if (['ExitPromptError', 'AbortPromptError'].includes(error?.name)) {
      stderr(`spec-autonomous: ${message('init.cancelled', locale)}\n`);
      return 130;
    }
    if (error instanceof CommanderError && error.exitCode === 0) return 0;
    const syntaxError = error instanceof CommanderError;
    const code = syntaxError ? 'invalid_arguments' : /^[a-z_]+:/.test(error.message) ? error.message.split(':')[0] : 'operation_failed';
    const localized = localizeError(error, locale);
    if (preferences.json || preferences.format === 'json') stdout(JSON.stringify({ schema_version: 1, error: { code, message: localized } }) + '\n');
    else stderr((locale === 'en' && diagnostic) || `spec-autonomous: ${localized}\n`);
    return syntaxError ? 2 : 1;
  }
}

export function runCli(argv, options) {
  const locale = detectLocale({ explicit: languageArg(argv) });
  return withLocale(locale, () => executeCli(argv, locale, options));
}
