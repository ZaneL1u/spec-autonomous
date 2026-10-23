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

## Risks / Trade-offs

- 同树不同环境可能结果不同 → class + environment 进入键；integration 与 worktree 永不互相复用。
- 外部代码改动未经过审计 agent → 命令门仍重跑；文档说明审计针对原生验收引用与任务，源未变则不重审。
- stale 复验失败触发修复 → 走既有 repair 预算，不新增循环入口。

## Migration Plan

纯增量：旧账本读取时 `stale_phases` 与 `verification_cache` 为空；旧写入者看到新字段会被 serde 拒绝的风险由 ledger schema 3 的"拒绝旧写入者"规则覆盖。无需迁移命令。
