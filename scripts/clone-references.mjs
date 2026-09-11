import { existsSync, mkdirSync, readFileSync } from 'node:fs';
import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';
const base = new URL('../.references/', import.meta.url);
mkdirSync(base, { recursive: true });
const lock = JSON.parse(readFileSync(new URL('../docs/research/upstreams.lock.json', import.meta.url), 'utf8'));
function git(args) {
  const result = spawnSync('git', args, { encoding: 'utf8', shell: false });
  if (result.error) throw result.error;
  if (result.status !== 0) throw new Error(result.stderr);
  return result.stdout.trim();
}
for (const repo of lock.repositories) {
  if (!/^[a-z0-9-]+$/.test(repo.directory) || !/^[0-9a-f]{40}$/.test(repo.commit)) throw new Error('Invalid reference lock entry');
  const path = fileURLToPath(new URL(repo.directory, base));
  if (!existsSync(path)) {
    git(['clone', '--depth', '1', '--no-checkout', repo.url, path]);
    git(['-C', path, 'fetch', '--depth', '1', 'origin', repo.commit]);
    git(['-C', path, 'checkout', '--detach', repo.commit]);
  }
  const actual = git(['-C', path, 'rev-parse', 'HEAD']);
  const remote = git(['-C', path, 'remote', 'get-url', 'origin']);
  const dirty = git(['-C', path, 'status', '--porcelain']);
  if (actual !== repo.commit || remote !== repo.url || dirty) throw new Error(`${repo.directory}: reference differs from lock or is dirty; existing checkout left intact`);
  console.log(`${repo.directory}: ${actual} (${repo.version})`);
}
