#!/usr/bin/env node
import { runCli } from '../lib/cli-program.mts';

process.exitCode = await runCli(process.argv.slice(2));
