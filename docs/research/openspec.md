# OpenSpec 适配研究

调研时间：2026-09-10（America/Los_Angeles）。本报告基于已克隆源码及官方仓库；版本结论只对应以下固定快照，不推定所有已发布版本具有相同 JSON 字段。

| 项目 | 核实结果 |
| --- | --- |
| 上游 | [Fission-AI/OpenSpec](https://github.com/Fission-AI/OpenSpec) |
| 本地参考 | `.references/openspec`，只读参考，不纳入发行包 |
| Commit | `9d4e5974e5c0d9a09b9c6c1e1eb0975e80ec4461` |
| Commit 时间 | `2026-09-09T20:59:22Z` |
| package.json | `@fission-ai/openspec`，`1.13.0`，MIT，Node `>=20.19.0` |
| 上游开发栈 | TypeScript / pnpm；与本项目的 Rust / Bun 栈独立 |
| 核实方式 | 源码、schema、官方 agent contract 静态交叉核对；本子研究未安装或执行上游 CLI |

版本及运行时依据：[package.json](https://github.com/Fission-AI/OpenSpec/blob/9d4e5974e5c0d9a09b9c6c1e1eb0975e80ec4461/package.json)。

## 1. 结论与边界

建议把 OpenSpec 作为**规范、规划工件、任务清单及变更生命周期的来源**，通过其 CLI JSON 接口适配；本项目负责**执行计划、fresh-context worker、依赖调度、隔离、恢复、验证与结果回写**。不 fork 上游执行逻辑，不把 slash command 文本当成可调用进程，不要求目标仓库迁移为本项目的专属 schema。

`openspec instructions apply` 的行为是读取文件并输出指令和任务数据，并不会自行启动模型或执行代码。因此，上层 Rust supervisor 可以保持确定性，按需启动短生命周期的 planner、worker、verifier；主线程持有任务图和简短结果，不累计每个 worker 的完整对话。[apply 实现](https://github.com/Fission-AI/OpenSpec/blob/9d4e5974e5c0d9a09b9c6c1e1eb0975e80ec4461/src/commands/workflow/instructions.ts#L482-L634)

上游有工件 DAG，但默认任务只是 Markdown checkbox 的有序列表；没有可直接消费的实现任务 DAG、文件所有权、worker lease、重试策略或验证证据。适配层必须区分两个图，不能把工件 `requires` 当作代码任务依赖。[schema 类型](https://github.com/Fission-AI/OpenSpec/blob/9d4e5974e5c0d9a09b9c6c1e1eb0975e80ec4461/src/core/artifact-graph/types.ts#L24-L52)

## 2. 推荐调用协议

由父进程固定工作目录和已解析的 OpenSpec 可执行文件，使用 argv 数组创建子进程；JSON 从 stdout 解析，stderr 单独留档，同时记录 exit code、版本和命令。不要从模型输出拼接 shell 字符串。

| 阶段 | 官方 CLI | 本项目需要的结果 |
| --- | --- | --- |
| 能力探测 | `openspec --version`；必要时各命令 `--help` | 版本和本机支持的命令能力 |
| 发现变更 | `openspec list --json` | change 名、任务计数、根路径 |
| 规划状态 | `openspec status --change <id> --json` | 工件依赖、状态、具体输出路径、apply gate |
| 批量状态 | `openspec status --all --json` | 新版可用；旧版回退逐 change 查询 |
| 规划工件上下文 | `openspec instructions <artifact-id> --change <id> --json` | 模板、上下文、工件规则、依赖、输出位置 |
| 实现上下文 | `openspec instructions apply --change <id> --json` | 任务、进度、contextFiles、state、当前项目约束 |
| 规范校验 | `openspec validate <id> --strict --json --no-interactive` | 逐项校验结果及失败原因 |
| schema 查询 | `openspec schemas --json` | 可用 schema；成功返回顶层数组 |
| 新建 change | `openspec new change <id> --schema <name> --json` | 上游创建的目录、metadata、schema |
| 归档输入 | `openspec instructions archive --change <id> --json` | 当前 context 和 advisory guidance，不执行归档 |

命令分支及参数依据：[workflow CLI](https://github.com/Fission-AI/OpenSpec/blob/9d4e5974e5c0d9a09b9c6c1e1eb0975e80ec4461/src/cli/index.ts#L642-L749)。JSON、诊断及 root 约定以官方 [Agent Contract](https://github.com/Fission-AI/OpenSpec/blob/9d4e5974e5c0d9a09b9c6c1e1eb0975e80ec4461/docs/agent-contract.md) 为外部契约入口。

新版本 JSON 大多没有统一协议版本，只有 validate 的报告有 `version`。Rust 类型应允许未知字段，显式检查所需字段；不能仅以 semver 推断能力。非零 exit 仍可能带有效 JSON，例如批量状态部分成功和 validate 失败；保留报告后停止相应动作。空 stdout、非法 JSON、缺失必需字段均为适配失败，不能当作空任务或成功。

## 3. 三种“完成”必须分开

1. `status.isPlanningComplete` / 兼容字段 `isComplete`：工件存在且依赖满足；不代表代码实现完毕。`artifacts[].status` 可为 `done`、`skipped`、`ready`、`blocked`，顺序是依赖顺序，平级按 schema 声明顺序。
2. `instructions apply.state == all_done`：受跟踪的 checkbox 全勾选；不代表测试或验收通过。
3. 本项目的 `verified / integrated`：独立验收成功，改动已集成到目标 checkout，并完成安全的上游 checkbox 回写。

状态格式源码：[instruction-loader.ts](https://github.com/Fission-AI/OpenSpec/blob/9d4e5974e5c0d9a09b9c6c1e1eb0975e80ec4461/src/core/artifact-graph/instruction-loader.ts#L453-L536)。apply 的 `blocked / ready / all_done` 判定：[instructions.ts](https://github.com/Fission-AI/OpenSpec/blob/9d4e5974e5c0d9a09b9c6c1e1eb0975e80ec4461/src/commands/workflow/instructions.ts#L564-L609)。

默认 `apply.requires: [tasks]` 仅检查直接要求的 tasks 工件。若用户先创建 tasks 文件但缺少 design 等上游工件，不能假设 apply 的 `ready` 等同完整规划就绪。读取 `missingPrerequisites`、`warnings`，并结合 status 和严格校验建立本项目的启动前检查。调度器发现未知 schema 状态时应报告能力不足，避免猜测。

自定义 schema 可以不配置 tracking file，此时 apply 仍可 `ready`，但没有可持续推进的 checkbox 列表。本项目应降级为规划阶段或报告“无任务跟踪契约”；不能将零任务直接认定为已完成。

## 4. 任务解析与稳定身份

这一快照使用：

```text
^\s*[-*]\s*\[([\sxX])\]\s*(.*)
```

因此 `-` / `*`、缩进、大小写 `X`、CRLF 都可能出现；代码围栏及 HTML 注释里的 checkbox 也参与上游计数。空描述 checkbox 计入 `progress`，却不进入 `tasks` 数组。`tasks[].id` 是过滤空描述后的 `1, 2, ...`，不是任务文本中的 `1.1`，插入任务会改变后续 id。[parser](https://github.com/Fission-AI/OpenSpec/blob/9d4e5974e5c0d9a09b9c6c1e1eb0975e80ec4461/src/utils/task-progress.ts#L7-L80)，[id 分配](https://github.com/Fission-AI/OpenSpec/blob/9d4e5974e5c0d9a09b9c6c1e1eb0975e80ec4461/src/commands/workflow/instructions.ts#L331-L353)。

建议：

- 默认从 apply JSON 获取任务语义，保留原始文档作为定位依据；不要另写一个更严格的 parser 后声称等价。
- 在本项目 sidecar 保存稳定任务 ID、change identity、任务原文、来源文件、原始内容 hash、定位信息和明确依赖；不用 CLI 数字 id 作为持久主键。
- 文件变化后做唯一映射校验；相同描述、插入删除或重排无法唯一映射时重新导入/规划，避免把完成标记打到另一项。
- coordinator 是 `tasks.md` 唯一写入者。worker 只提交结果、改动引用和验证证据；集成成功后 coordinator 比对文件版本并原子修改 checkbox，再调用上游刷新状态。
- `progress.total != tasks.length` 时输出诊断，尤其处理空 checkbox；禁止丢弃不可执行条目后将运行判定成功。

实际受跟踪文件由 schema 的 `apply.tracks` 决定，不总是顶层 `tasks.md`。上游 list/archive 的 progress helper 能通过 artifact `generates` glob 汇总多个文件，而这一版 apply 路径直接读取 `apply.tracks` 指定的文件；因此 MVP 应明确支持单一具体 tracking file，遇到 glob 或无法一致定位的自定义配置就报告不支持，不能假定两个接口完全等价。[progress 文件解析](https://github.com/Fission-AI/OpenSpec/blob/9d4e5974e5c0d9a09b9c6c1e1eb0975e80ec4461/src/utils/task-progress.ts#L92-L124)，[apply 读取](https://github.com/Fission-AI/OpenSpec/blob/9d4e5974e5c0d9a09b9c6c1e1eb0975e80ec4461/src/commands/workflow/instructions.ts#L545-L554)。

## 5. Schema 与自定义工作流

schema 名称选择依次为 CLI override、change `.openspec.yaml`、项目 `openspec/config.yaml`、`spec-driven`。同名 schema 的文件来源则依次为项目 `openspec/schemas/<name>`、用户 XDG data 下 override、包内置。这是两个不同的优先级维度。[配置说明](https://github.com/Fission-AI/OpenSpec/blob/9d4e5974e5c0d9a09b9c6c1e1eb0975e80ec4461/docs/customization.md#schema-resolution-order)，[resolver](https://github.com/Fission-AI/OpenSpec/blob/9d4e5974e5c0d9a09b9c6c1e1eb0975e80ec4461/src/core/artifact-graph/resolver.ts#L122-L174)。

默认工件图是 `proposal -> {specs, design} -> tasks`；artifact 上的 `generates` 允许 glob。理论上 specs 和 design 同级就绪，但并行写作前仍需检查语义依赖和写入位置，默认保留上游排序。每个实现 task 的依赖和文件写集由本项目的 plan sidecar 表达，而非修改内置 schema。[默认 schema](https://github.com/Fission-AI/OpenSpec/blob/9d4e5974e5c0d9a09b9c6c1e1eb0975e80ec4461/schemas/spec-driven/schema.yaml)

目标仓库可以保持自己的工作流。本仓库初期也推荐直接采用 `spec-driven`，在 `openspec/config.yaml` 写 Rust / Bun / npm 工程约束，并用 proposal、design、specs、tasks 形成实施方案。后续若确实要将执行计划升为正式工件，可通过 `openspec schema fork spec-driven autonomous` 增加 `execution-plan`，再 `openspec schema validate autonomous`；这是可选能力，不作为适配 OpenSpec 的前置条件。

## 6. fresh-context worker 的上下文包

每次派发读取一份新的 apply JSON，保留它的 schema、root、当前 `context`、`operationGuidance` 与任务映射；worker 输入只包括：

1. 当前任务原文、验收条件、依赖的集成结果摘要。
2. 上游返回的相关工件路径及必要内容；`contextFiles` 是具体文件数组，不是要让模型自行猜的路径模板。
3. 当前项目规则、运行限制、允许写入路径、worktree 路径与 base commit。
4. 结构化结果要求：状态、修改文件、测试命令/退出码、阻塞原因、集成引用。

`context` 是需要应用的项目事实与约束，`operationGuidance` 是建议；将两者与用户授权、程序状态和路径隔离，不允许 advisory 内容改变调度器的权限或状态机。`references` 是上游只读索引，按需取相关 spec，避免把所有引用仓库及历史对话装进每个 worker。[运行时输入装配](https://github.com/Fission-AI/OpenSpec/blob/9d4e5974e5c0d9a09b9c6c1e1eb0975e80ec4461/src/commands/workflow/instructions.ts#L503-L543)

## 7. 仓库发现与 root 陷阱

这一快照支持本地 planning root、`store:` 指针、显式 `--store`、全局默认 store 和引用 store；仅检测 `cwd/openspec/tasks.md` 或是否存在 `openspec/` 都不充分。需要返回候选检测证据，再由 OpenSpec 的 context/status JSON 确认解析结果。目标路径可能来自父目录或者当前仓库之外的 store。

首期可以只执行 repo-local change，同时把 store 支持声明为尚未支持的 capability。若 `root.path`、`changeDir` 或文件 realpath 超出选定根，应停止写入并给出明确诊断。不能把 store 路径改写成 repo-relative 路径来“继续”。未来支持 store 时需将 spec 根与代码 checkout 分离，并为任务回写单独加锁。[root-selection.ts](https://github.com/Fission-AI/OpenSpec/blob/9d4e5974e5c0d9a09b9c6c1e1eb0975e80ec4461/src/core/root-selection.ts)

`skip_specs: true` 会使 specs 工件为 `skipped` 并满足依赖；不得因为文件缺失又强行生成 specs。不同接口中的 `status` 可能是字符串或诊断数组；root 中的 `store_id` 使用 snake_case，即使外层字段是 camelCase。

## 8. 初始化建议

官方安装方式与适用于本仓库的非交互初始化：

```sh
npm install --save-dev --save-exact @fission-ai/openspec@1.13.0
npx --no-install openspec init --tools codex --profile core --no-animation
npx --no-install openspec new change bootstrap-spec-autonomous --schema spec-driven --json
```

如果仓库统一用 Bun 管理依赖，可用 `bun add --dev --exact @fission-ai/openspec@1.13.0` 和 `bun run` 脚本调用本地 CLI；Bun 是工程工具选择，不意味着上游声明的 Node 最低版本可以忽略。本段是可执行方案，实际初始化与验证由主任务实施。

`--tools none` 可只搭建 OpenSpec 目录；`--tools codex` 在此版本写入 `.agents/skills`。不需要 `--tools all`，也不需要为全局安装修改用户所有 agent 配置。`--force` 会参与旧配置清理，已有仓库的自动初始化应避免无条件添加。[init 参数](https://github.com/Fission-AI/OpenSpec/blob/9d4e5974e5c0d9a09b9c6c1e1eb0975e80ec4461/src/cli/index.ts#L220-L230)，[Codex 集成目录](https://github.com/Fission-AI/OpenSpec/blob/9d4e5974e5c0d9a09b9c6c1e1eb0975e80ec4461/src/core/config.ts#L54)。

## 9. 验收与归档

自动执行完成至少需要：全部本项目任务已集成、必要代码验证通过、所有上游任务已正确回写、`openspec validate <id> --strict --json --no-interactive` 成功。规范校验只能校验规范，不能替代 Rust / Bun 测试。

归档是单独生命周期动作。可在用户已授权的 automation policy 中启用；否则运行结束输出“已验证，待归档”。注意 `openspec archive <id> --yes --json` 的 `--yes` 同时跳过 incomplete-task 确认，调用前必须自行确认 remaining 为零。不要为使流程绿灯而自动添加 `--no-validate` 或 `--skip-specs`。归档可能合并/删除主规范并移动目录，worker 不得并发执行归档。[archive 参数](https://github.com/Fission-AI/OpenSpec/blob/9d4e5974e5c0d9a09b9c6c1e1eb0975e80ec4461/src/cli/index.ts#L479-L486)，[未完成任务处理](https://github.com/Fission-AI/OpenSpec/blob/9d4e5974e5c0d9a09b9c6c1e1eb0975e80ec4461/src/core/archive.ts#L1338-L1371)。

## 10. Adapter 契约测试重点

- 普通 spec-driven change、尚无工件、已有完整规划、全部 checkbox 完成。
- 嵌套 checkbox、CRLF、`* [X]`、空描述、同名任务、代码围栏中的示例 checkbox。
- task 文件被人工插入、改名、删除或重排后，回写拒绝错误目标。
- 自定义 schema 改名 tracking file、`tracks: null`、glob、未解析 schema。
- `skip_specs`、非零 exit 带 JSON、非法 JSON、未知新增字段、批量状态部分失败。
- store 指针、父目录根、全局默认 store、真实路径越界，确保 local-only 能力报告准确。
- worker 成功但验证失败、验证成功但集成冲突、进程中断后恢复，确保 checkbox 不提前勾选。

这些应通过固定版本 JSON fixture 和真实临时仓库的 adapter integration test 组合验证；fixture 中保留上游 commit，避免把未来格式变化伪装为本项目逻辑缺陷。
