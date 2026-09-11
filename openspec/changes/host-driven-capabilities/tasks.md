## 1. Passive core and host protocol

- [x] 1.1 Remove agent launch paths and introduce host/config/request contracts; verify a trap runner is never executed by any public capability.
- [x] 1.2 Implement finite prepare/next and idempotent pending work packets with bounded context; verify restart and repeated prepare retain identities and all native inputs.
- [x] 1.3 Implement claim/heartbeat/receipt/revoke lifecycle and source/identity checks; verify duplicate, mismatched, stale and cancelled submissions.
- [x] 1.4 Preserve native roadmap/planning, DAG parallelism, verification, repair, audits and delivery through host round trips; verify OpenSpec and Spec Kit complete mock milestones and phase ranges.
- [x] 1.5 Preserve integration/hook crash recovery and old-run read-only compatibility; verify crash cuts and no legacy runner reactivation.

## 2. Complete and granular capabilities

- [x] 2.1 Add capability registry and compact/field/paginated query views; verify CLI schemas and result limits without lost aggregate counts.
- [x] 2.2 Implement document inspect/frontmatter/CAS updates and template generation; verify CRLF, unknown fields, stale writes and path boundaries.
- [x] 2.3 Implement roadmap/task/state/history granular tools through shared validation; verify ranges, dependencies, completion gates and decision/blocker persistence.
- [x] 2.4 Implement worktree inventory/diff/create/merge/cleanup tools; verify active/dirty/locked/external preservation and validated merge behavior.
- [x] 2.5 Implement native archive preview/apply for OpenSpec and Spec Kit; verify actual provider archive, stale previews, active-work refusal and source preservation.
- [x] 2.6 Implement doctor and explicit bounded repairs; verify reads never mutate and unsupported repairs preserve data.

## 3. Host surfaces and delivery

- [x] 3.1 Expose complete capabilities plus advanced tools from CLI using one service; verify legacy aliases prepare work rather than spawning agents.
- [x] 3.2 Implement stdio MCP tools/resources with the same schemas and operations; verify handshake, calls, malformed requests, limits and clean shutdown.
- [x] 3.3 Update five skills, installer and project configuration for host-driven execution; verify ownership/upgrade and absence of agent launcher instructions.
- [x] 3.4 Build an independent mock host and full regression suite; verify concurrency, callbacks, verification failures, crash recovery and process-boundary assertions.
- [x] 3.5 Update architecture/migration/usage docs, version alpha.2 and native npm package; run full checks, strict specs and installed tarball CLI/MCP smoke.

验收：见 [host-driven alpha.2 本地验证](../../../docs/validation/host-driven.md)。16/16 项完成；真实 npm 发布与未运行平台仍由原发行清单维护，不混入本变更的完成声明。
