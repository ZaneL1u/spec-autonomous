## Why

本机 Codex dogfood 证明：合法 worker 改写已声明的 `task.writes` 文件会因该文件同时出现在 `context_files` 被误判 `scope_violation`；同任务后续成功后仍会留下 `submitted`/`issued` 悬挂；心跳因多余 `host` 字段失败；doctor 不提示 skills 版本漂移。这些缺陷让复杂 run 卡在 `awaiting_host`，不能把 unattended closeout 当作已验证能力。

## What Changes

- 明确写入集优先：出现在 `task.writes` 中的 context 文件允许修改；`.spec-autonomous/` 仍禁止。
- 同一 work unit 后续 `accepted`/`integrated` 后，较早的 `submitted` 回执标为 `superseded` 并保留证据；resume 对已存在账本做同样调和。不自动 revoke 仍由宿主持有的 `issued`/`claimed`。
- `work.heartbeat` 忽略可选 `host` 字段，避免宿主复用 claim 参数时 `unknown field arguments.host`。
- `apply-result` 可附带可选 `usage`；progress 在有回传时汇总，否则仍报 `unavailable`。
- `doctor` 报告 CLI 与 `skills-installed.toml` 版本差、ledger `user_version`，以及过期租约/未收回执。
- Skill 文本区分 OpenGSD、禁止把 `prepare` 当健康检查，并规定会话结束时 heartbeat / revoke / cleanup。

## Capabilities

### New Capabilities

- （无）

### Modified Capabilities

- `agent-execution`: 写入范围以 `task.writes` 为准，context 文件不是绝对只读。
- `host-work-protocol`: 心跳参数兼容、doctor 诊断、可选 usage。
- `run-recovery`: 调和过期 submitted 回执，不重复应用已被后续成功取代的规划。

## Impact

Rust `plan`/`execution`/`host`/`recovery`/`progress`/`doctor`、能力 JSON schema、五份 Skills 源文件与合约测试。JSON/MCP 字段保持向后兼容：只增加可选属性。不迁移用户仓库里的旧 run；下次 resume/prepare 时调和账本。不自动删除用户项目里已存在的 worktree。
