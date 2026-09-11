# Open GSD 调研：借鉴自主执行机制，保持适配层轻量

调研日期：2026-09-10（America/Los_Angeles）。本报告核对了官方站点与本地 shallow clone 源码；没有安装或运行上游 agent，也没有消耗模型调用验证 autonomous 行为。以下区分上游可见实现与本项目设计建议。

## 1. 项目身份与固定快照

用户指定的 [opengsd.net](https://opengsd.net/) 对应 GitHub 组织 `open-gsd`，本次研究该组织的两个项目，未用其他同名 GSD 项目替代。官方网站把 Core 定位为嵌入现有 agent 的 prompt framework，把 Pi 定位为 standalone harness。其 GSD Path 页面仍标为 coming soon，不能作为已交付实现的依据。[官网](https://opengsd.net/)

| 项目 | 本地目录 | 源码 package 版本 | 固定 commit | 许可证 |
| --- | --- | --- | --- | --- |
| `@opengsd/gsd-pi` | `.references/gsd-pi` | `1.19.0` | `0fd02c1ea7a87d9d9a8bb8323322de597520e1b4` | MIT |
| `@opengsd/gsd-core` | `.references/gsd-core` | `1.13.0` | `523be34133bf92922f42b031954b53c6101827e4` | MIT |

版本直接读取快照的 [Pi package.json](https://github.com/open-gsd/gsd-pi/blob/0fd02c1ea7a87d9d9a8bb8323322de597520e1b4/package.json#L1-L10) 与 [Core package.json](https://github.com/open-gsd/gsd-core/blob/523be34133bf92922f42b031954b53c6101827e4/package.json#L1-L10)，不代表 npm registry 的实时 dist-tag。两个克隆均为只读调研材料，不进入产品发行包。

调研时官网仍显示 Pi `1.15.0`、Core `1.9.0`，且 [Pi 展示页](https://opengsd.net/pi) 的模拟器显示 `state.json`。这些展示内容落后于本次源码，不能据此设计运行状态兼容层。源码许可证为 [Pi LICENSE](https://github.com/open-gsd/gsd-pi/blob/0fd02c1ea7a87d9d9a8bb8323322de597520e1b4/LICENSE) 和 [Core LICENSE](https://github.com/open-gsd/gsd-core/blob/523be34133bf92922f42b031954b53c6101827e4/LICENSE)；若未来复制实质代码或模板，应随发行保留相应版权与许可证声明。本次建议独立实现机制，不引入源码依赖。

## 2. 两种自主执行机制

### GSD Core：宿主 agent 驱动的工作流

`autonomous.md` 枚举未完成 phase，按 discuss → plan → execute 驱动对应 workflow，每个 phase 后重新读取 roadmap，允许后续计划发生变化。并非一条固定 shell 脚本包办所有工作；workflow 使用宿主的 Skill/Agent 能力，其可靠性受宿主的派发、会话与完成事件契约影响。[autonomous.md](https://github.com/open-gsd/gsd-core/blob/523be34133bf92922f42b031954b53c6101827e4/gsd-core/workflows/autonomous.md)

`execute-phase.md` 明确主 agent 只负责发现计划、分析依赖、分 wave、派发、处理 checkpoint 与汇总。executor 在自己的上下文中加载任务材料。父上下文通常只读 frontmatter、状态与必要摘要，避免把大型 agent 指令和全部结果正文重复塞入父会话。[执行编排](https://github.com/open-gsd/gsd-core/blob/523be34133bf92922f42b031954b53c6101827e4/gsd-core/workflows/execute-phase.md#L8-L39)、[context-budget.md](https://github.com/open-gsd/gsd-core/blob/523be34133bf92922f42b031954b53c6101827e4/gsd-core/references/context-budget.md)

这部分最贴近本项目的用户体验目标，但不宜逐字复制流程：Core 自己有 `.planning/`、phase/plan 命名体系、大量模板、hooks、runtime 差异分支。让 OpenSpec/Spec Kit 再生成整套 `.planning/` 会增加第三套规范事实源。

### GSD Pi：确定性 host 驱动的执行循环

Pi 把调度器放入程序。执行循环根据状态确定下一 unit，构建任务上下文，创建新会话，执行并收集结果，然后验证、持久化与继续。`runUnit()` 每次显式调用 `newSession({ workspaceRoot, abortSignal })`，并处理会话创建超时、晚到事件和取消；“新上下文”是运行时行为，不只是 prompt 中的一句要求。[auto/run-unit.ts](https://github.com/open-gsd/gsd-pi/blob/0fd02c1ea7a87d9d9a8bb8323322de597520e1b4/src/resources/extensions/gsd/auto/run-unit.ts#L73-L200)

下一步选择在可读的 dispatch 规则中实现；同一规则可用于 preview，但 preview 不应留下 dispatch 副作用。本项目应吸收这种“先可视化计划，再用同一个调度模型执行”的边界。[auto-dispatch.ts](https://github.com/open-gsd/gsd-pi/blob/0fd02c1ea7a87d9d9a8bb8323322de597520e1b4/src/resources/extensions/gsd/auto-dispatch.ts)

**建议：采用 Pi 的程序控制循环，采用 Core 的轻主上下文原则。** Rust supervisor 持有任务图、run 状态与调度决策；需要语义判断时才派发 planner/reviewer。执行任务使用独立 agent 进程与新会话，保留现有 agent CLI 的认证、工具与模型能力。

## 3. 如何真正保持上下文干净

Pi 使用 `UnitContextManifest` 描述 unit 所需的 inline/on-demand artifacts、工具策略和 prompt 字符预算，composer 按清单组装。Core 更偏向只传文件定位，让 child 自行加载。二者共通点是显式定义每次任务的输入，而非继承无边界的父会话历史。[manifest](https://github.com/open-gsd/gsd-pi/blob/0fd02c1ea7a87d9d9a8bb8323322de597520e1b4/src/resources/extensions/gsd/unit-context-manifest.ts#L258-L284)、[composer](https://github.com/open-gsd/gsd-pi/blob/0fd02c1ea7a87d9d9a8bb8323322de597520e1b4/src/resources/extensions/gsd/unit-context-composer.ts#L1-L102)

建议本项目为每次 attempt 生成不可变 `TaskPacket`：

- 任务 ID、目标、验收标准、非目标与允许修改的路径。
- 原始 OpenSpec change / Spec Kit feature 的路径、来源段落、内容 hash。
- 明确依赖与已经合入的依赖结果摘要。
- 工作目录、起始 commit、runtime 能力与可执行验证命令。
- 结果 JSON schema、结果输出路径、日志路径。

主线程只接收结构化 `WorkerResult`、短摘要、changed-files、验证结果与 artifact 引用。完整 tool traces 留在磁盘，不汇入长期主上下文。固定的“200k context”属于上游模型时期的参数，不能成为产品兼容承诺；本项目应使用可配置输入预算与实际 runtime 能力探测。

## 4. 并行不是简单地多开 agent

上游已有三种不同粒度，不能把它们混成同一隔离保证：

| 路径 | 派发单位 | 隔离与调度 | 借鉴点 |
| --- | --- | --- | --- |
| Core execute-phase | plan，按 wave 分组 | 依赖 wave；同 wave `files_modified` 重叠则串行；可创建独立 worktree | 适合 MVP 的保守 wave 编排 |
| Pi parallel orchestration | milestone | 独立进程/worktree；共享项目根 SQLite；依赖满足才运行；文件重叠仅警告 | 学习进程管理，不复制其较粗粒度 |
| Pi reactive execution | 同一 slice 内 task | 从 input/output 构图，挑无冲突 ready 子集；child 使用同一 milestone 工作目录 | 学习细粒度依赖与冲突选择，不能宣称每 task 都有独立 worktree |

证据：[Core overlap guard](https://github.com/open-gsd/gsd-core/blob/523be34133bf92922f42b031954b53c6101827e4/gsd-core/workflows/execute-phase.md#L501-L533)、[Pi eligibility](https://github.com/open-gsd/gsd-pi/blob/0fd02c1ea7a87d9d9a8bb8323322de597520e1b4/src/resources/extensions/gsd/parallel-eligibility.ts#L99-L173)、[Pi task graph](https://github.com/open-gsd/gsd-pi/blob/0fd02c1ea7a87d9d9a8bb8323322de597520e1b4/src/resources/extensions/gsd/reactive-graph.ts#L25-L131)、[reactive child cwd](https://github.com/open-gsd/gsd-pi/blob/0fd02c1ea7a87d9d9a8bb8323322de597520e1b4/src/resources/extensions/gsd/prompts/reactive-execute.md#L9-L33)。

Pi 的 milestone worker 通过 `headless --json auto` 启动，NDJSON 事件回传。源码特意避免 `--print "/gsd auto"`：会话切换曾导致外层调用提前返回，让进程在模型实际执行前退出。这说明 runtime adapter 必须验证真正的终态协议，不能把一次命令退出或一条“started”事件当作任务完成。[spawnWorker](https://github.com/open-gsd/gsd-pi/blob/0fd02c1ea7a87d9d9a8bb8323322de597520e1b4/src/resources/extensions/gsd/parallel-orchestrator.ts#L580-L631)

本项目建议的最小规则：

1. planner 只提出 `depends_on`、`read_set`、`write_set` 与验证边界；Rust 验证无环、依赖存在和任务覆盖。
2. 只有依赖已经验证并整合的任务才进入 ready 集合；同批读写冲突、写写冲突或未知写集则串行。
3. 每个写入 worker 使用独立 worktree；共享锁文件、manifest、schema、构建入口的变更优先作为前置串行任务。
4. 默认最多 2 个 worker，允许显式调整上限；worker 不再自行创建递归的平行 coordinator。
5. 主进程串行整合结果，验证整合后的源码，并更新上游任务完成标记。下一 wave 从新的集成 commit 建立 worktree。
6. 文件范围是调度与事后审计依据，worktree 是 Git 隔离；两者都不等于进程安全沙箱。检测到越界写入应阻止自动整合，执行权限仍交给 agent runtime。

Core 已有“上一 wave 合入后下一 wave 错用旧 base”的专项修复与降级策略，证明 `expected_base` 必须是运行记录中的一等字段。[between-wave reset](https://github.com/open-gsd/gsd-core/blob/523be34133bf92922f42b031954b53c6101827e4/gsd-core/references/execute-phase-between-wave-reset.md)

## 5. 磁盘状态、验证与恢复

### 上游可见实现

Pi 当前 `.gsd/gsd.db` 为 runtime 权威，milestone/slice/task、attempt、result、verification、worker、lease、dispatch 等写入 SQLite，Markdown 主要是便于 review 与 prompt 的 projection；数据库不可用时不能悄悄从 Markdown 推导回运行态。项目根数据库被 worktree worker 共用，协调限定在单机本地磁盘，不能把 SQLite WAL 当跨主机协调器。[auto-mode 状态说明](https://github.com/open-gsd/gsd-pi/blob/0fd02c1ea7a87d9d9a8bb8323322de597520e1b4/docs/user-docs/auto-mode.md)

运行记录通过 worker heartbeat、lease fencing token 与 active dispatch 唯一约束避免重复占用。任务完成不等同于 agent 返回成功：host 将验证绑定到被测源码 revision，源代码变化时旧的 pass 不能继续当作当前证明。[dispatch ledger](https://github.com/open-gsd/gsd-pi/blob/0fd02c1ea7a87d9d9a8bb8323322de597520e1b4/src/resources/extensions/gsd/db/unit-dispatches.ts#L1-L82)、[host verification](https://github.com/open-gsd/gsd-pi/blob/0fd02c1ea7a87d9d9a8bb8323322de597520e1b4/src/resources/extensions/gsd/auto-verification.ts#L195-L234)

Pi 还有 no-progress backstop：按目标、guard 与输入 hash 记录不推进结果，重复相同输入达到阈值就中止循环。该快照阈值为 2；值得吸收的是持久化“同一失败没有改变”的检测，不必照搬阈值或其完整 recovery domain。[auto-liveness-backstop.ts](https://github.com/open-gsd/gsd-pi/blob/0fd02c1ea7a87d9d9a8bb8323322de597520e1b4/src/resources/extensions/gsd/auto-liveness-backstop.ts#L1-L49)

Core 将“子会话异常终止”与“工作是否完成”分开，先核对 summary 与任务匹配的 Git commits，避免子 agent 已完成但终态消息丢失后又重复执行。本项目可借鉴先 reconcile 再 retry；不能把 summary 存在当作独立的成功证明，还应验证结果 schema、提交归属、改动范围和测试。[completion reconciliation](https://github.com/open-gsd/gsd-core/blob/523be34133bf92922f42b031954b53c6101827e4/gsd-core/workflows/execute-phase/steps/completion-reconciliation.md)

### 本项目应保留与简化的部分

**规范与运行账本必须分清权威。** 原项目的 OpenSpec/Spec Kit 文件继续承载产品规范、设计与任务意图；本项目只持有 execution plan、attempt、结果与恢复状态。不能学 Pi 把整个规范也导入自己的数据库后宣称原文件只是 projection，否则会破坏“适配现有 spec 工具”的目标。

MVP 可使用单个 supervisor 独占写入、原子 snapshot 与可重放事件日志。worker 只写自己的结果目录，由 supervisor 验证后入账；这样不需要一开始就复制分布式租约体系。即使采用文件账本，也必须明确：进程锁、写入顺序、截断日志处理、schema version、attempt 唯一 ID、source hash、旧 worker 结果拒收、崩溃后的重放与幂等整合。若需要多个 coordinator 同时调度一个仓库，再引入 SQLite 事务、唯一约束和 fencing token。

建议生命周期为：

```text
pending → ready → running → verifying → integrating → completed
                     ↘ failed / blocked / cancelled
```

retry 创建新的 attempt ID，不覆盖旧结果；resume 先检查旧进程是否仍存活，未确认 worker 结束前不派发替身。验证绑定 `{spec_hash, plan_hash, base_commit, result_commit}`。用户改动 spec 或集成 HEAD 变化后，重新检查受影响任务；完成标记只由 adapter 在验证与整合都成功之后写回。

## 6. 不复制的重型部分

Pi 仓库已经包含自有 agent/provider 层、TUI、web 控制面、daemon、RPC、MCP server、众多工具扩展及原生桥接。把它作为依赖会引入完整 agent 产品的生命周期，而非一个 spec scheduler。[Pi workspace](https://github.com/open-gsd/gsd-pi/blob/0fd02c1ea7a87d9d9a8bb8323322de597520e1b4/pnpm-workspace.yaml)、[Pi README](https://github.com/open-gsd/gsd-pi/blob/0fd02c1ea7a87d9d9a8bb8323322de597520e1b4/README.md#L195-L205)

Pi 的 Rust/native 发布方式也需要区别：其 `@gsd/native` 是 N-API binding，平台包包含 `gsd_engine.node`，不是独立 Rust CLI。可以参考按 `os/cpu` 拆分 npm 平台包，但本项目更适合 Node launcher 选择并启动 Rust executable；Bun 用于开发、测试与打包，最终 npm 用户不必安装 Bun 或 Rust。[native package](https://github.com/open-gsd/gsd-pi/blob/0fd02c1ea7a87d9d9a8bb8323322de597520e1b4/packages/native/package.json)、[darwin-arm64 平台包](https://github.com/open-gsd/gsd-pi/blob/0fd02c1ea7a87d9d9a8bb8323322de597520e1b4/native/npm/darwin-arm64/package.json)

MVP 不纳入全套 model routing、OAuth、终端 UI framework、cloud sync、跨主机 worker、可视化浏览器工具、向量记忆、跨模型 convergence review 或 GSD 自有 milestone schema。保留五个必要机制即可：**可审查任务图、全新 worker 上下文、保守并行隔离、验证后推进、磁盘恢复**。

## 7. 对 OpenSpec 编排方案的直接输入

- `SpecAdapter` 和 `AgentRuntime` 必须正交：前者负责读取/校验/写回 spec，后者负责 session/process 及结构化终态。支持一个 spec 工具，不意味着必须绑定一个 agent 品牌。
- planner 产物是对源任务的执行细分，必须保留父任务映射。所有子任务验证并整合后，才能勾选原任务，防止部分完成被误报。
- 先提供 detect / inspect / plan / doctor / dry-run，让用户审查识别结果、冲突边与 worktree 策略，再实现 run / status / pause / resume / cancel。
- “轻量”应通过小协议与单写入者实现，不能通过删掉完成验证、进程取消、恢复与幂等性来实现。
- v0.1 首先证明一个 spec change 可以经过多 fresh workers 执行、合并和恢复；真实跨 runtime 的兼容性必须分别跑 conformance checks，不能只凭 CLI 名称或模拟 runner 声称通过。

以上是源码研究后的工程建议，不是对上游运行质量的实测结论。
