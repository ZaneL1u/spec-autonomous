# 宿主协议

本文是 skills（`autonomous` / `auto` / `resume`）引用的完整协议。CLI 与 MCP 是同一服务：MCP 工具名把能力 id 的 `.`/`-` 换成 `_` 并加 `sa_` 前缀，细粒度能力通过 `sa_tools {"operation":"call","capability":"...","arguments":{...}}` 或 `spec-autonomous tools call <capability> --input @args.json --json` 调用。

## 一次循环需要多少次调用

| 步骤 | 调用 | 说明 |
| --- | --- | --- |
| 会话开始 | `doctor`、`inspect` | 各一次。设置错误（缺原生工具、`invalid_config`、`budget_cap`）是阻塞项，用 `providers status|ensure` 或改配置解决，不要用 prepare 当健康检查 |
| 开始 / 续跑 | `prepare` + `host` | 返回的 `work[]` 已按派生会话认领并带 `token` 与 `owner` |
| 派发 | 宿主自己的 agent 能力 | 只传 packet 的 `input_path` / `prompt_path` / `result_schema_path` 与 worktree |
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
- `work.revoke {run_id, request_id, host_stopped: true, reason, token?|host?}`：令牌或拥有者 `host_id` 二选一；只有确认子会话已停止才能调用。
- 完成与否由 CLI 回执与账本决定，而不是 agent 会话的退出状态：结果已提交则该单元已完成；结果未提交则重新提交同一结果或重派同一 packet。

## 锁

协调器锁最多等待 10 秒再报 `run_already_active`；并发的心跳、回执、认领不再立刻失败。`project_in_use` 仍表示另一个 run 占有该仓库。

## 视图

- `verification_readiness` 只在 `prepare` / `next` / `apply-result` 计算；`work.claim`、`work.claim-batch`、`work.heartbeat` 的响应里它是 `{"status":"not_computed"}`。
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

`repair_repeated`（同一失败在一轮修复后再次出现）、`budget_cap`、`verification_missing` 和其他 `needs_input` 都是交给操作者的决定，不是重试信号。`verification_preflight_failed` 用相同 `run_id` 再 `prepare` 即得到带诊断的新规划包。
