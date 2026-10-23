## Why

真实长任务（share-your-terminal、code-wrapper v0.5.5）显示：续跑时 `reconcile_source` 与 handed_off 分支会把已完成 phase 和计划清零；审计结果的每次宿主往返都重新进入 phase 验证；worker/combined 与 phase/post-hook 在完全相同的源码树上重复执行同一组命令。一个 run 里 63% 的验证记录是在同一提交上重复同一条命令，8 个 phase 被"完成"了 28 次。这些重复不是独立复验，而是同树同命令的重跑。

## What Changes

- 验证 PASS 证据以「检查前后一致的 Git tree hash + 命令 + 相对 cwd + 运行环境 + 准备钩子 + 检出类别」为键持久化到 run 账本；同键的后续检查直接复用并记录 `reused_from`，不再执行。失败从不缓存，树变化、环境变化、`run.revise` 改命令都自然失效。
- 无 hook 改变已接受提交时跳过 post-hook 波次。
- 续跑对账不再清空 `completed_phases` / `plans`：origin 仅吸收了 integration 历史（用户 fast-forward）时不合并不重置；外部代码改动把已完成 phase 标记为 `stale`，在交付前只重跑其命令门；原生源目录改动只重开对应 phase，不再沿 `depends_on` 级联，依赖它的 phase 转为 stale。
- progress 的 overhead 区分 `executed_checks` 与 `reused_checks`。
- 新增 `[execution] verification_reuse = "same-tree" | "none"`，默认 `same-tree`。

## Capabilities

### New Capabilities
- 无。

### Modified Capabilities
- `verified-integration`: 新增同树证据复用要求。
- `run-recovery`: 续跑保留已完成 phase，改为 stale 复验。

## Impact

Rust `engine/{lifecycle,recovery}.rs`、`model.rs`、`config.rs`、`git.rs`、`progress.rs`；账本 JSON 仅新增可选字段，ledger schema 不变；JSON/MCP 字段、状态值、错误码保持稳定，仅新增字段与事件种类。文档 `docs/long-running-runs.md`。不启动 agent，不修改消费项目。
