## Context

See proposal.md and docs/research/long-run-churn-analysis.md (R1~R3) plus docs/research/gsd-lessons-2026-10.md. GSD Core 以 `covered_digest` 派生 `stale` 态，GSD Pi 以检查前后两次整树哈希一致的 `tested_source_revision` 回放结论；两者都不在续跑时清零进度。

## Goals / Non-Goals

Goals: 同树同命令同环境只执行一次；续跑保留已完成 phase 与计划；外部改动只触发命令门复验；保持独立验收语义与 JSON/MCP 稳定。
Non-goals: 跨 run 复用；审计 agent 缓存；在途工作的 source_drift 处理；worktree 回收；账本分表（后续 change）。

## Decisions

- 缓存键 = sha256(JSON{tree, class, argv, cwd(相对), runner.environment, verification_prepare.argv, attempt_timeout_seconds, max_log_bytes})。`tree` 取 `git rev-parse HEAD^{tree}`；`verify()` 已要求 checkout 干净，Wave 在检查后再次确认 HEAD 未变且干净（`tree_unchanged`），只有满足时才写入缓存，等价于"前后两次整树哈希一致"。
- `class` 区分 `integration`（长期集成检出，带构建缓存）与 `worktree`（新建 worker/candidate 检出）。worker→combined 同树同类可复用；phase→post-hook→milestone 同树同类可复用；worktree→integration 不复用。
- 缓存存放在 `HostState.verification_cache`，仅 host-driven run 拥有；旧账本缺省为空。复用时追加一条 Evidence，`details.reused_from` 指向原 wave/manifest/日志，`duration_ms = 0`，事件种类 `verification_reused`。
- 准备钩子按需执行：某波次全部命中则不运行钩子；首个未命中检查前运行一次。
- post-hook：`after_implement` hooks 未改变 `accepted_head` 时直接结束该 phase 的验证循环。
- 对账分类：先判定 `origin HEAD` 是否为 `accepted_head` 的祖先（用户 fast-forward），是则只更新 `origin_head` 与 `origin_sources`；否则合并，并用 `accepted_head...current` 三点 diff 取 origin 侧真正的改动文件。原生源目录命中 → `reopen(phase)` 并移除其计划；其他文件命中 → 所有已完成 phase 进入 `stale_phases`。`reopen` 不再级联，依赖 phase 进入 `stale_phases`。
- `stale_phases` 在全部选中 phase 完成后、交付前逐个重跑 `checks(phase)`（命令门）；失败走既有 `repair_failure` + `reopen` 后重新 drive。审计结果保留：原生任务与验收引用未变。
- handed_off 分支同理：integration HEAD 前进时按 `accepted_head..actual` 的文件分类，不清空计划。
- 配置开关 `verification_reuse`，`none` 时恢复逐次执行；非法值报 `invalid_config`。
- 漂移延后：`check()` 发现原始规范变化时，若存在 issued/claimed/receiving/submitted/candidate 的 attempt，置 `host.reconcile_pending` 并继续；`execute_phase` 处理完回执后，若仍有在途工作则 `awaiting_host`，否则立即 `reconcile_source` 并以 `source_reconciled:` 重启 drive。`reconcile_source` 在有在途工作时同样只置标记。
- 计划扩展：`execute_phase` 在计划 source_hash 失配时先尝试 `extend_plan`：计划覆盖的源任务都仍存在 → 只对未覆盖且未完成的源任务调用 planner（packet 含 `metadata.existing_plan`），新任务追加并默认依赖原计划叶子；否则整体重规划。`converge` 不再 `plans.remove`。
- 审计缓存键 = sha256(JSON{phase, scope, 集成树哈希, 验收 id 批次})，只缓存全部通过的批次；packet 的 `execution_evidence` 不参与键。
- 失败签名：`repair_audit` 以 `phase:sha256(失败项)` 计数，host-verification 项以最近一条失败证据的 argv/cwd/exit_code 标识；计数 > 1 即 `needs_input: repair_repeated`；phase 完成时按前缀清除。
- `run.revise` 的 `blocking_requests` = 修订任务的 implement 尝试 + 修订 phase（或里程碑级）的 converge/audit 尝试；`active_requests` 保留为信息字段。
- 分阶段检查用三个新数组而不是给 `Check` 加字段：`Check` 结构、PartialEq、TOML/JSON 形状与 `run.revise` 的比较逻辑都不变，旧配置零迁移。`task_checks = verification + task_verification + task.verification`；`checks(phase) = verification + phase_verification + phase.verification`；`checks_milestone = verification + milestone_verification + milestone.verification`。规划器的"项目级检查"= `verification + task_verification`。
- `validate_check` 对 argv 元素 > 1024 字节或含 `b85decode` / `b64decode` / `a85decode` / `base64.b` / `zlib.decompress` / `exec(zlib` / `exec(compile(` / `eval(atob` / `from('base64')` 的检查报 `invalid_command: inline_script_argv`，配置与计划同时生效。
- preflight：可执行文件的 future 判定去掉"必须含 `/`"的条件；新增 `script_operand`（python/bash/sh/zsh/ruby/perl 的首个非选项操作数，`-c`/`-e`/`-m`/`-` 视为无脚本）→ `check_script_deferred` / `check_script_missing`。
- `environment_with_overrides` 实际执行 `--version`（go 为 `version`）取首行；读取 `package.json` scripts、清单文件存在性、`git ls-files` 中的测试文件（截断 100 并给总数）、`git check-ignore` 常见缓存目录。`build_plan` 为每个规划单元附 `previous_planning_failures`（最多 10 条，含 `"code":"…"` 提取的诊断代码）。
- 预算：`Execution.unsafe_budgets`（默认 false）；`validate` 在未开启时拒绝超出 `MAX_ATTEMPTS_CAP=10` / `MAX_REPAIR_ROUNDS_CAP=4` / `RUN_TIMEOUT_CAP_SECONDS=259200`，错误文本说明如何放开；`Config::budget_warnings` 供 doctor 输出。

## Risks / Trade-offs

- 同树不同环境可能结果不同 → class + environment 进入键；integration 与 worktree 永不互相复用。
- 外部代码改动未经过审计 agent → 命令门仍重跑；文档说明审计针对原生验收引用与任务，源未变则不重审。
- stale 复验失败触发修复 → 走既有 repair 预算，不新增循环入口。

## Migration Plan

纯增量：旧账本读取时 `stale_phases` 与 `verification_cache` 为空；旧写入者看到新字段会被 serde 拒绝的风险由 ledger schema 3 的"拒绝旧写入者"规则覆盖。无需迁移命令。
