import { defineConfig } from 'tsdown';

// The Git facade runs `packages/cli/bin/*.mjs` straight from a clone, so the
// build output is committed next to the sources instead of a `dist/` folder.
export default defineConfig({
  entry: ['packages/cli/src/**/*.mts'],
  root: 'packages/cli/src',
  outDir: 'packages/cli',
  unbundle: true,
  format: 'esm',
  platform: 'node',
  target: 'node22',
  outExtensions: () => ({ js: '.mjs' }),
  // Scripts and tests import the built runtime, not `src`, because asset paths
  // such as `../locales` and `../bin/provider-bridge.mjs` resolve relative to
  // the output layout. Declarations keep those imports type-checked.
  dts: true,
  clean: false,
  treeshake: false,
  report: false,
  sourcemap: false,
});
