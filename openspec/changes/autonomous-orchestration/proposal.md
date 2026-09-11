# Autonomous milestone execution

## Why

OpenSpec / Spec Kit 用户规划完里程碑后，仍需反复提示 agent 继续、搬运上下文、处理并行冲突和判断是否完成。核心产品承诺是：**沿用已有规划，一次启动后自主完成整个里程碑的开发、验证、修复和集成。**

## What Changes

- 统一 milestone 入口：首版一个 OpenSpec change 或一个 Spec Kit feature 对应完整里程碑，覆盖全部任务和验收。
- Rust supervisor 自动推进执行闭环，常规任务交接和范围内修复无需用户反复“继续”。
- 每任务使用 fresh-context worker 和独立 worktree，依赖及写集允许时并行；主线程维护决策、计划与简短摘要。
- 增加稳定任务图、预算、无进展检查、持久化账本与暂停恢复。
- 分离 spec adapter 和 agent runner，复用原生规范体系，通过 npm 分发 CLI。

## Capabilities

### New Capabilities

- `spec-adapters`: 选择、导入与回写 OpenSpec / Spec Kit，并报告能力边界。
- `milestone-autonomy`: 整里程碑自动推进、有界修复和最终验收。
- `task-planning`: 稳定任务身份、依赖验证、任务拆分与并行约束。
- `agent-execution`: 新会话、隔离工作区、有界上下文、进程控制。
- `run-recovery`: 持久化状态、租约、预算、暂停恢复和崩溃协调。
- `verified-integration`: 验证证据、受控集成、并发编辑检查和最终交付。

### Modified Capabilities

无。bootstrap 的只读检测与 npm 基础继续保留，新 inspect/run 能力独立增加。

## Impact

影响 Rust core/runtime/adapters、CLI、契约 fixtures 与端到端测试。计划引入 Tokio、SQLite、内容 hash 和 Markdown parser；本次方案阶段不提前安装 runtime 依赖。保留 OpenSpec 内置 spec-driven schema，执行计划和账本放 `.spec-autonomous/`。本变更是待实现路线，不代表 bootstrap 已具备自主开发能力。
