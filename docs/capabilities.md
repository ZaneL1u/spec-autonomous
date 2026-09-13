# 能力参考

版本：`0.1.0-alpha.2`，接口 envelope schema 1。完整能力默认公开，细粒度工具通过目录访问。

```sh
spec-autonomous tools list --all --json --limit 200
spec-autonomous tools call <capability> --input '{"argument":"value"}' --json
# 避免 shell JSON 转义，直接读取参数文件
spec-autonomous tools call <capability> --input @arguments.json --json
```

MCP 的七个主工具将连字符替换成下划线并加 `sa_` 前缀。高级 `sa_tools` 使用 `{"operation":"list"}` 或 `{"operation":"call","capability":"...","arguments":{...}}`。`mcp --all-tools` 可显式公开全部细粒度工具。

每个条目的完整 input_schema 可从目录查询。读取工具支持 view、fields、limit、offset；写工具只支持 view，业务 CAS/凭据见必需参数。省略的选择参数由原生框架定位，存在歧义时明确拒绝。

| 能力 | 级别 | 读写 | 必需参数 | 职责 |
| --- | --- | --- | --- | --- |
| `inspect` | 完整 | 只读 | 按场景选择 | Read provider, native artifacts, provenance and repository state. |
| `progress` | 完整 | 只读 | 按场景选择 | Read all Git worktrees and verified progress without mutation. |
| `prepare` | 完整 | 写入/可写 | 按场景选择 | Prepare work packets and advance the selected milestone. |
| `next` | 完整 | 只读 | 按场景选择 | Preview prepared work, blockers and next action without dispatching. |
| `apply-result` | 完整 | 写入/可写 | `token`, `host`, `result` | Accept an owned host receipt, verify and integrate it, and prepare subsequent work. |
| `archive` | 完整 | 写入/可写 | 按场景选择 | Preview a native archive; apply with its plan_hash using an isolated candidate. |
| `doctor` | 完整 | 只读 | 按场景选择 | Inspect configuration, ledger, inventory and recoverable operations. |
| `capabilities` | 细粒度 | 只读 | 按场景选择 | Discover complete and granular interfaces with versioned schemas. |
| `work.claim` | 细粒度 | 写入/可写 | `run_id`, `request_id`, `token`, `host` | Register the host's unique fresh session for a prepared request. |
| `work.heartbeat` | 细粒度 | 写入/可写 | `run_id`, `request_id`, `token` | Report host liveness and receive stop/pause instructions. |
| `work.revoke` | 细粒度 | 写入/可写 | `run_id`, `request_id`, `token`, `host_stopped`, `reason` | Revoke a request after its host confirms execution has stopped. |
| `work.context` | 细粒度 | 只读 | `run_id`, `request_id` | Read complete immutable work context with verified hashes. |
| `hook.resolve` | 细粒度 | 写入/可写 | `run_id`, `key`, `outcome`, `evidence` | Record the explicit outcome of an interrupted native hook. |
| `native.instructions` | 细粒度 | 只读 | 按场景选择 | Read the exact installed provider action without generating a private replacement. |
| `native.create` | 细粒度 | 写入/可写 | 按场景选择 | Create a native change or selected feature directory through the provider bridge. |
| `repair` | 细粒度 | 写入/可写 | `kind` | Preview and apply supported repairs with source hashes and retained backups. |
| `document.inspect` | 细粒度 | 只读 | `file` | Parse Markdown/TOML metadata, headings and task provenance. |
| `frontmatter.get` | 细粒度 | 只读 | `file` | Read structured frontmatter or TOML fields with source hash. |
| `frontmatter.patch` | 细粒度 | 写入/可写 | `file`, `expected_hash`, `patch` | CAS-merge frontmatter fields while retaining Markdown body bytes. |
| `toml.patch` | 细粒度 | 写入/可写 | `file`, `expected_hash`, `patch` | CAS-update selected top-level TOML values. |
| `document.patch` | 细粒度 | 写入/可写 | `file`, `expected_hash`, `find`, `replace` | Replace one exact document span after a source-hash check. |
| `document.scaffold` | 细粒度 | 写入/可写 | `file`, `kind` | Create operational summary/verification/handoff/decision templates without overwriting. |
| `roadmap.import` | 细粒度 | 写入/可写 | `milestone` | Validate and save a typed milestone with its generated Markdown view. |
| `roadmap.get` | 细粒度 | 只读 | `milestone_id` | Read the canonical TOML milestone and its digest. |
| `roadmap.select` | 细粒度 | 只读 | `milestone_id` | Resolve an inclusive range using verified outside prerequisites. |
| `roadmap.add` | 细粒度 | 写入/可写 | `milestone_id`, `expected_hash`, `phase` | Append a phase with dependency and source ownership validation. |
| `roadmap.insert` | 细粒度 | 写入/可写 | `milestone_id`, `expected_hash`, `phase`, `after` | Insert a phase after a stable ID/label without renumbering identities. |
| `roadmap.remove` | 细粒度 | 写入/可写 | `milestone_id`, `expected_hash`, `phase_id` | Remove a phase only if the remaining graph is valid. |
| `roadmap.render` | 细粒度 | 写入/可写 | `milestone_id`, `expected_hash` | Regenerate the roadmap view without overwriting human edits. |
| `worktree.list` | 细粒度 | 只读 | 按场景选择 | List all worktrees, including unmanaged entries. |
| `worktree.diff` | 细粒度 | 只读 | `worktree` | Read worktree differences and status without changing files. |
| `worktree.create` | 细粒度 | 写入/可写 | `name` | Create a named isolated workspace from an explicit commit/ref. |
| `worktree.merge` | 细粒度 | 写入/可写 | `worktree_id`, `expected_head`, `expected_target` | Verify and fast-forward a tool-created workspace with source/destination CAS. |
| `worktree.remove` | 细粒度 | 写入/可写 | `worktree_id`, `expected_head` | Remove a clean owned workspace, keeping branch refs. |
| `state.get` | 细粒度 | 只读 | `run_id` | Read a run, decisions and blockers without exposing credentials. |
| `state.decision` | 细粒度 | 写入/可写 | `run_id`, `summary` | Append a decision with optional state revision guard. |
| `state.block` | 细粒度 | 写入/可写 | `run_id`, `blocker_id`, `reason` | Add an explicit blocker that prevents workflow advancement. |
| `state.unblock` | 细粒度 | 写入/可写 | `run_id`, `blocker_id` | Resolve one explicit blocker without forcing completion. |
| `state.checkpoint` | 细粒度 | 写入/可写 | `run_id`, `stopped_at` | Record continuity and next-action notes. |
| `history.get` | 细粒度 | 只读 | `run_id` | Read ordered durable events and decisions. |
| `verify.source` | 细粒度 | 只读 | 按场景选择 | Read native readiness and source validation diagnostics. |
| `verify.plan` | 细粒度 | 只读 | `plan` | Validate a supplied execution plan against its native source. |
| `verify.references` | 细粒度 | 只读 | `file` | Check local Markdown references without reading outside the project. |
| `audit.open` | 细粒度 | 只读 | `run_id` | Read outstanding native tasks, verification gaps and host work. |
| `history.summaries` | 细粒度 | 只读 | `run_id` | Read compact summaries of completed semantic work and failures. |
| `git.inspect` | 细粒度 | 只读 | 按场景选择 | Read repository HEAD, branch and working/index state. |
| `git.commit` | 细粒度 | 写入/可写 | `expected_head`, `message`, `files` | Commit explicitly selected files without capturing unrelated staged changes. |
| `run.pause` | 细粒度 | 写入/可写 | `run_id` | Request pause and return host cancellation actions. |
| `run.cancel` | 细粒度 | 写入/可写 | `run_id` | Cancel a run without pretending external sessions stopped. |
| `run.cleanup` | 细粒度 | 写入/可写 | `run_id` | Preview terminal cleanup, apply with plan_hash, optionally delete safe merged refs. |
| `task.list` | 细粒度 | 只读 | `run_id` | Read execution tasks and source bindings. |
| `task.ready` | 细粒度 | 只读 | `run_id` | Read eligible tasks under dependency and conflict constraints. |
| `task.complete` | 细粒度 | 写入/可写 | `token`, `host`, `result` | Complete owned task work through the same verified receipt gate. |
| `task.claim` | 细粒度 | 写入/可写 | `run_id`, `request_id`, `token`, `host` | Claim a prepared task for a fresh host context. |

## Work receipt

```json
{
  "token": "<prepare 返回的 token>",
  "host": {"host_id":"my-host","session_id":"unique-work-session","fresh_context":true},
  "result": {
    "schema_version":1,"run_id":"<run>","task_id":"<task>","attempt_id":"<request>",
    "status":"candidate","summary":"Completed the assigned work",
    "blockers":[],"milestone":null,"plan":null,"audit":[]
  }
}
```

具体工作类型及其结果形状以请求的 result.schema.json 为准。宿主传回结果，CLI 执行验证/集成；result 不能声明 integrated/verified。返回 awaiting_host 表示还有宿主工作，不是完成。领取凭据只用于该请求，普通 progress/next 不公开它。

## MCP resources

- `spec-autonomous://capabilities`：完整能力目录。
- `spec-autonomous://progress`：当前仓库进度。
- `spec-autonomous://runs/<run_id>`：只读状态。
- `spec-autonomous://work/<run_id>/<request_id>`：带 hash 校验的不可变上下文。

URI 只接受注册的形式和安全 ID，不提供任意文件路径读取。查询中的缺失字段、不支持能力和模糊选择均有明确错误。

运行恢复新增 `run.revise`（见 [修订与续跑](run-revision.md)）；批量认领使用 `work.claim-batch`。`run.cleanup` 采用预览/哈希应用，详见 [清理规则](run-cleanup.md)。
