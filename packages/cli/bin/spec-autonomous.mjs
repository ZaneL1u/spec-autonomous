#!/usr/bin/env node
import { runCli } from '../lib/cli-program.mjs';

process.exitCode = await runCli(process.argv.slice(2));
