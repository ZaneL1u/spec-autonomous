## 1. Git install facade

- [x] 1.1 Add root bin/files/dependencies without Git preparation or install hooks; verify npm root pack and Bun lockfile behavior.
- [x] 1.2 Implement private gh asset retrieval, hash/version validation, cache locking and lazy recovery; verify unit tests for success, reuse, concurrency and failures.

## 2. Publication and acceptance

- [x] 2.1 Preserve ordinary tarball / CLI / MCP behavior and update documentation; verify package tests, Git-local e2e and repository regression.
- [x] 2.2 Publish alpha.5 source and private Release assets; verify private visibility, exact source commit, asset hashes and real Git SSH npm installation under Node 22.
