# 宿主协议

本文是 skills（`autonomous` / `auto` / `resume`）引用的完整协议。CLI 与 MCP 是同一服务：MCP 工具名把能力 id 的 `.`/`-` 换成 `_` 并加 `sa_` 前缀，细粒度能力通过 `sa_tools {"operation":"call","capability":"...","arguments":{...}}` 或 `spec-autonomous tools call <capability> --input @args.json --json` 调用。默认的 `tools list` 只列主能力，并在 `hidden_capabilities` 中列出被隐藏的细粒度能力 id、在 `hint` 中提示 `tools list --all`；`run.revise` 的 `task_checks` / `phase_checks` / `milestone_checks` / `task_writes` 条目带完整 schema（`{phase_id, task_id, checks:[{argv, cwd?}]}`、`{phase_id, task_id, add:[path]}`）。

## 一次循环需要多少次调用

| 步骤 | 调用 | 说明 |
| --- | --- | --- |
| 会话开始 | `doctor`、`inspect` | 各一次。设置错误（缺原生工具、`invalid_config`、`budget_cap`）是阻塞项，用 `providers status|ensure` 或改配置解决，不要用 prepare 当健康检查 |
| 开始 / 续跑 | `prepare` + `host` | 返回的 `work[]` 已按派生会话认领并带 `token` 与 `owner` |
| 派发 | 宿主自己的 agent 能力 | 只传 packet 的 `input_path` / `prompt_path` / `result_schema_path` 与 worktree；`prompt.md` 是指向 `input.json` 的摘要，不必解析 |
| 回执 | `apply-result`（`host` = packet 的 `owner`，`claim_next: true`） | 验证、集成、签发下一批并为该宿主自动认领，响应即下一批工作 |
| 结束 | `run.cleanup apply:true` | 不删分支 / integration 时不需要预览哈希 |

每个工作单元约 2 次协调器调用（dispatch 由宿主完成）。不要用 `prepare` 轮询；手里没有工作时才调 `next`。

## 宿主身份与自动认领

`host = {host_id, session_id, fresh_context: true}`。`prepare` / `next` / `apply-result` 接受 `host`：

- `prepare` 带 `host`：所有尚无主的 `issued` 请求被认领给 `host_id`，会话 id 派生为 `<session_id>-<request_id>`（截断到 120 字符），因此每个请求仍是唯一的 fresh 会话；响应带 `auto_claimed: true/false`。其他宿主已认领的请求不会被接管。
- `apply-result` 用 packet 的 `owner` 作为 `host`；带 `claim_next: true` 时处理完回执后以同样方式认领后续工作，不带时后续工作保持 `issued`，由 `work.claim` / `work.claim-batch` 显式认领。
- `next` 带 `host`：只读，不认领；对 `owner.host_id` 等于该 `host_id` 的请求返回 `token`（找回令牌）。不带 `host` 时不返回任何令牌。
- `work.claim` / `work.claim-batch` 仍可用于显式认领。

## 令牌、心跳、撤销

- `token` 只在 `prepare`、`apply-result`、带 `host` 的 `next` 与 `request.get` 中返回。
- `request.get {run_id, request_id, host?}` 返回一个请求的 `status`、`owner`、`heartbeat_age_ms`、`stale`、`has_receipt`、packet 路径与 `error`；`host_id` 匹配时附 `token`。
- `stale` 只是信息：所有权从不转移，`apply-result` 不再因心跳过期拒收已持有的回执。`work.heartbeat` 只在 packet 的 `limits` 要求时需要。
- packet 的 `limits` 自带说明：`enforced: false`（CLI 不计时、不终止宿主 worker）；`applies_to` 说明 `attempt_timeout_seconds` 约束的是 CLI 自己启动的验证检查与钩子子进程；`heartbeat_required: false`；`stale_effect: "informational"`。`run_remaining_ms` 与 `max_result_bytes` 含义不变。
- `work.revoke {run_id, request_id, host_stopped: true, reason, token?|host?}`：令牌或拥有者 `host_id` 二选一；只有确认子会话已停止才能调用。
- 完成与否由 CLI 回执与账本决定，而不是 agent 会话的退出状态：结果已提交则该单元已完成；结果未提交则重新提交同一结果或重派同一 packet。

## 工作包内容

`prompt.md` 只放 worker 规则、`input.json` 的绝对路径与 sha256，以及按工作类型的摘要：implement 列出 `task.id` / `description` / `source_ids` / `depends_on` / `writes` / `reads` / `verification`、原生指令摘录与 `previous_candidate`；规划列出来源、`source_hash` 与 `snapshot.tasks` 概要；审计列出验收 id。完整请求只在 `input.json`（超出 `max_context_bytes` 时仍由 `context_reference` 指向 `context-full.json`），worker 必须先完整读取它，摘要不能替代它。

`input.json` 按类型裁剪：只有 audit 带 `snapshot.metadata.acceptance`；implement 不带 `metadata.documents`，`snapshot.next_action.instruction` 由 OpenSpec apply 的整份 JSON 换成其中的 `instruction` 文本（附 `warnings` 与 `context`，任务表与上下文文件已在 `snapshot.tasks` / `snapshot.context_files`）；规划包中与顶层 `failure` 相同的 `previous_planning_failure.error` 换成 `error_in: "failure"`。`context_files` 原样保留。converge 包的 `instruction` 只在 `Gaps (<n> failed; <m> passed audit items omitted): [...]` 中列出 `passed: false` 的审计条目，已通过条目只计数；请求键仍按完整审计计算，升级前签发的 converge 请求照常匹配回执。

`input_hash` 是签发时 `input.json` 字节的哈希，只对新签发的包改变。升级前已签发、尚未回执的包仍使用磁盘上原有的 `input.json` / `prompt.md`，回执与 `work.context` 按原哈希校验，不受影响。

## 重派时继承上次改动

同一任务上一个 implement attempt 以 `failed` / `blocked` 结束且留有未提交改动时，重派的 implement 包在 `input.json` 中带 `previous_candidate = {attempt_id, base_commit, patch_path, files, same_base}`：补丁写在新 attempt 目录的 `previous-candidate.diff`，worktree 仍在时由 `git diff` 相对其 base 生成，已回收时取回收保存的 `patch.diff`。`same_base: true` 时在 worktree 根目录 `git apply --index <patch_path>` 即可复用，否则用 `--3way` 或仅作参考；prompt 同样说明，并要求复用后重新运行该任务的检查。没有上次改动时不输出该字段，旧包的 `input_hash` 不变。

## 追加写集

implement worker 发现必须改动 `task.writes` 之外的文件（例如被改动波及的既有测试）时返回 `status: "blocked"` 并附 `requested_writes`（项目相对路径，至多 10 个；结果 schema 中为可选字段，其他状态忽略）。协调器只自动授权字面文件路径（模式与目录返回 `write_scope_not_literal`，需人工 `run.revise`），且在路径规范且合法、不命中 `.git/` / `.spec-autonomous/` / 原生上下文（含其父目录）、不与其他在途任务写集冲突时自动追加写集并重派，响应里直接是带 `previous_candidate` 的新 implement 包；否则 run 进入 `needs_input`，blocker 含原因（`write_scope_forbidden` / `write_scope_not_literal` / `write_scope_conflict` / `write_scope_limit` 等）和可直接执行的 `run.revise` `task_writes` 输入。操作者也可随时用 `run.revise {task_writes:[{phase_id,task_id,add}]}` 手工追加，详见 [修订与续跑](run-revision.md)。

## worker 检出准备

写入型包签发前，协调器已在其 worktree 中按配置初始化子模块（`[execution] init_submodules`）并运行 `[hooks.worker_prepare]`，宿主和 worker 不需要再手工 clone 子模块或重建依赖环境。准备失败时不会签发该包：run 进入 `needs_input`，blocker 以 `worker_prepare_failed` 开头，修好后 `prepare --run-id`（改过配置加 `reload_config`）。doctor 对未挂钩子的 `verification-prepare.*` / `worker-prepare.*` 脚本和未初始化的子模块给出警告。配置与示例见 website/reference/configuration.md。

## 原生任务子条目

规划包里 `snapshot.tasks[].description` 仍只取复选框那一行，以便与 OpenSpec 原生输出逐字比对；复选框下方缩进更深的非复选框行（子列表、验收命令、围栏代码块原样保留）放在 `details`，遇到下一个复选框、标题或缩进回退即止，没有子条目时不输出该字段。planner 按指令把 `details` 写进对应 Task 的 `description`，其中可执行的验收命令转成该任务的 `verification`。

## 脏工作区

`dirty_checkout` 的消息在原句之后附 `; dirty: {...}`，按 `modified`（已跟踪文件的改动，含已暂存、重命名与冲突，另带 `staged` 计数）、`untracked`、`submodules` 三类给出 `count`、最多 20 条 `paths` 与 `truncated`。错误码与信封不变；未跟踪文件仍会阻止开始，提交、移走或加入 ignore 后再 `prepare`。

## 锁

协调器锁最多等待 10 秒再报 `run_already_active`；并发的心跳、回执、认领不再立刻失败。`project_in_use` 仍表示另一个 run 占有该仓库。

## 视图

- `verification_readiness` 只在 `prepare` / `next` / `apply-result` 计算；`work.claim`、`work.claim-batch`、`work.heartbeat` 的响应里它是 `{"status":"not_computed"}`。
- 检查 argv 以裸 `openspec` 开头且 PATH 解析不到时，预检、检查执行与归档前的配置检查统一改用 run 配置里的 `provider.openspec_command`（受管理或项目本地 OpenSpec）加原参数。`verification_readiness.checks[]` 的 `argv` 仍是声明值，`resolved_argv` 给出实际执行的 argv；证据清单记录 `executed_argv`。
- run 的配置在启动时冻结。`prepare` / `next` / `apply-result` 发现源目录当前的项目配置（用户级与项目 `config.toml` 合并后）与 run 启动或上次 `reload_config` 时加载的不一致时，在 `warnings` 中给出 `config_drift: project configuration changed since the run started; pass reload_config to apply it`，但不会自动重载。`extend_seconds`、`max_attempts`、`max_workers` 这类 run 本地覆盖不算漂移。
- full 视图的 `config` 包含 `hooks`（每个钩子的名称、`argv` 与 `idempotent`），用于确认 `verification_prepare` / `worker_prepare` 等钩子是否生效；`environment` 仍为 `redacted`。
- 读取能力支持 `fields` / `limit` / `offset`，可分页的数组新增 `attempts`、`work`、`evidence`、`intents`。
- 不要直接读取 `.git/spec-autonomous/state.db`。缺字段请报协议缺口。

## 修订与清理一步应用

- `run.revise`：只有被修订范围内的在途工作会阻塞（`blocking_requests`：修订任务的 implement，被修订 phase 或里程碑级的 converge / audit）。`blocking_requests` 为空时 `apply: true` 不需要 `plan_hash`；否则先预览再带哈希应用。
- `run.cleanup`：不删分支、不删 integration 时 `apply: true` 不需要 `plan_hash`；`delete_branches` / `delete_integration` 仍要求预览哈希。

## Smart Discuss

phase 有灰区时 `prepare` 返回 `needs_input`，`blocker` 以 `discussion_required` 开头，响应附 `discussion.cards`。`recommended` 先展示，点击后 `discussion.apply`；用户同意默认值时 `prepare` 带 `discussion_auto: true` 在同一次调用内应用并继续。

## 宿主能力

响应里的 `host_capacity = {max_concurrency, lease_seconds, capabilities}` 来自项目配置的 `[host]`；并发派发不要超过 `max_concurrency`。遇到异常先调只读的 `run.forensics {run_id}`，它汇总重复失败、attempt 过多、空闲请求和 worktree 积压。

## 停止条件

`repair_repeated`（同一失败在一轮修复后再次出现）、`budget_cap`、`verification_missing` 和其他 `needs_input` 都是交给操作者的决定，不是重试信号。`verification_preflight_failed` 用 `spec-autonomous prepare --run-id <run_id>`（不要再带 `--change` / `--feature`，二者与 `--run-id` 互斥）即得到带诊断的新规划包。

`blocker` 只描述本次停止的原因，历史失败在 `attempts[].error` 中。`no_progress: no eligible task remains` 按「最新在前」只列每个未完成任务最近一次失败（每条截断到 600 字符），其余失败只给计数（`(<n> earlier failures are listed in attempts)`）。

检查因宿主进程被中断、`run.pause` / `run.cancel` 或 run 截止时间而被取消（证据 `failure_kind: "cancelled"`，exit 130）时，错误以 `paused: verification_cancelled` 开头而不是 `verification_failed`。暂停 / 取消请求、进程中断或 run 截止时间在开始集成前或两条检查之间（包括合并验证 `combined` 阶段）被发现时同样处理，错误分别以 `paused:` / `cancel:` / `budget_exhausted:` 开头。两种情况下被验证的 implement 回执都保持 `submitted`，不记失败、不计入 `max_attempts` 与无进展判断，阶段 / 里程碑门禁也不会因此排修复轮；`prepare --run-id <run_id>` 对同一候选重新验证，不重新实现。在此之前该请求仍以 `status: "submitted"` 出现在 `work[]` 中，宿主不要重派它。

`verification_environment_unavailable` 表示预检的全部错误都是项目级（与任务无关）的 `check_executable_missing` / `check_cwd_missing`：计划本身没问题，缺的是环境。该规划 attempt 保持 `candidate`，不计入失败次数；装好可执行文件、建好目录或改正 `runner.environment` / 项目验证配置（改配置时带 `reload_config`）后，再 `prepare --run-id <run_id>` 只重跑预检，通过即直接签发实现工作，不会重新规划。源或基线已变化时旧候选会被标为 `superseded` 并重新规划。任务级的缺失仍按 `verification_preflight_failed` 处理并要求更正计划。
