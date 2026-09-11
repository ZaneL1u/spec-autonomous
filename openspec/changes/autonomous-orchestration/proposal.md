# Autonomous milestone execution

## Why

用户需要从一个里程碑目标开始生成 roadmap，再沿 OpenSpec / Spec Kit 原生流程规划和开发，也需要承接已有规划继续工作。核心产品承诺是：**通过 skill 或 CLI，原生手动与 autonomous 两种路径使用相同规范，按完整里程碑或 from/to 阶段范围自主推进，并能查看所有 worktree 的结构化进度。**

## What Changes

- 提供 /autonomous（同义别名 /auto）以及 milestone/progress/resume skills；init 绑定宿主短命令，CLI 是统一编排和结构化读取入口。
- 底层检测并依赖用户已有 SDD provider，遵循其原生 schema/templates/skills/artifacts；不迁移框架，不在缺 provider 时静默切换到私有 SDD。
- milestone 包含 roadmap phases，每 phase 引用一个 OpenSpec change 或 Spec Kit feature；TOML 保存编排声明，原生 Markdown 保存规范、设计与任务。
- 从给定目标研究和规划 roadmap，再通过原生 workflow 补齐工件；已有原生工作可直接接续，保留必需约束和质量门。
- 支持 from/to/only 阶段范围，范围完成与全里程碑完成分开；原生/自主模式可以明确交接。
- progress 汇总同 Git 仓库的所有 worktree，包括并发 workers 与外部 worktrees；提供 human/JSON/TOML 输出，并从 MD 提取带来源的结构化字段。
- Rust supervisor 自动推进执行闭环，常规任务交接和范围内修复无需用户反复“继续”。
- 每任务使用 fresh-context worker 和独立 worktree，依赖及写集允许时并行；主线程维护决策、计划与简短摘要。
- 增加稳定任务图、预算、无进展检查、持久化账本与暂停恢复。
- 分离 spec adapter 和 agent runner，复用原生规范体系，通过 npm 分发 CLI。

## Capabilities

### New Capabilities

- `spec-adapters`: 原生 workflow 桥接、MD 结构化读取、选择/导入/回写及能力边界。
- `milestone-autonomy`: 目标到 roadmap、阶段范围、原生/自主交接、有界修复与最终验收。
- `task-planning`: TOML roadmap 与执行图、稳定身份、依赖/拆分和并行约束。
- `agent-execution`: npm skills、统一 CLI、新会话、隔离工作区、上下文与进程控制。
- `run-recovery`: 全 worktree progress、持久化状态、租约、预算、交接恢复和崩溃协调。
- `verified-integration`: 验证证据、受控集成、并发编辑检查和最终交付。

### Modified Capabilities

无。bootstrap 的只读检测与 npm 基础继续保留，新 inspect/run 能力独立增加。

## Impact

影响 Rust core/runtime/workflow/adapters/progress、CLI、npm skill assets/installer、TOML 契约与端到端测试。计划引入 Tokio、SQLite、TOML/Markdown parser；本次不提前安装 runtime 依赖。保留 OpenSpec spec-driven；可版本化编排 TOML 与运行 DB/logs 分开管理。本变更仍待实现，当前 binary 只有 bootstrap detect 能力。
