# Spec Kit 适配研究

调研日期：2026-09-10（America/Los_Angeles）。本报告基于本地只读源码和官方仓库，不把上游命令模板中的执行指令当作本次调研的指令。

## 结论

Spec Kit 适合接入本项目，但产品定位需要避开已存在的能力：当前 Spec Kit 已有 YAML workflow、非交互 agent CLI 调用、并行 fan-out、持久化状态和 resume。我们的价值应是**从已有 spec 任务生成可验证的执行图，为每个任务提供独立上下文与工作区，在验证后集中回写，并跨 OpenSpec / Spec Kit 使用同一运行时**。仅把 `/speckit.implement` 放进一个循环，或仅提供 YAML workflow，不足以形成这个产品的差异。

首版建议做 Rust 原生、只读的文档适配器，保留上游文档为需求事实来源；执行状态、依赖补充、文件占用、证据、恢复日志放在本项目自己的状态目录。`specify` 是可选的原生能力桥接，不应成为所有用户必须安装的 Python 运行时依赖。npm 包负责分发 Rust CLI，Bun 用于开发和发布工具链。这是本项目的设计建议，不是上游要求。

## 1. 研究基线与范围

| 项目 | 观察结果 |
| --- | --- |
| 官方仓库 | `https://github.com/github/spec-kit` |
| 本地副本 | `.references/spec-kit`，只读研究，未安装或修改上游 |
| 精确 commit | `c173bf19a6654e3b05386ec3599349a55282b897` |
| commit 时间 | `2026-09-10T13:16:20-05:00` |
| 包内版本 | `specify-cli`，`1.0.7.dev0` |
| CHANGELOG 首个版本 | `1.0.6`，`2026-09-10`；这是源码中记录的版本，不把 main 当作已发布包 |
| 上游实现 | Python >= 3.11，Typer CLI；Bash / PowerShell / Python 项目脚本 |
| 许可证 | MIT；若后续直接分发上游源码或模板，应保留上游许可声明 |

证据：[版本与打包](https://github.com/github/spec-kit/blob/c173bf19a6654e3b05386ec3599349a55282b897/pyproject.toml#L1-L18)、[CHANGELOG](https://github.com/github/spec-kit/blob/c173bf19a6654e3b05386ec3599349a55282b897/CHANGELOG.md#L1-L28)、[LICENSE](https://github.com/github/spec-kit/blob/c173bf19a6654e3b05386ec3599349a55282b897/LICENSE)。

本次核实的是当前源码 profile。旧版项目的 `.specify/scripts/` 可能来自很早的安装，不能根据用户全局 `specify --version` 推断仓库脚本版本；发布适配器前仍需为明确支持的历史版本建立固定 fixtures。

## 2. 项目标记、文档布局和 feature 选择

常见项目结构如下；其中 feature 目录可以被显式指定到其他位置，并不只能是 `specs/###-*`：

```text
.specify/
  feature.json                 # 当前 feature_directory
  integration.json             # 已安装/默认 coding agent integration
  memory/constitution.md       # 项目原则
  scripts/{bash,powershell,python}/
  templates/                   # 可受 override / preset / extension 影响
  extensions.yml               # 项目 hooks
  workflows/runs/<run-id>/     # 上游自己的 workflow 状态
specs/<feature>/
  spec.md
  plan.md
  tasks.md
  research.md                  # 可选
  data-model.md                # 可选
  contracts/                   # 可选
  quickstart.md                # 可选
  checklists/                  # 执行前的只读检查门
.agents/skills/speckit-*/SKILL.md  # 当前 Codex integration 示例
```

`.specify` 是当前脚本向上寻找项目根目录的主要标记，优先于父级 Git 仓库；`SPECIFY_INIT_DIR` 可以显式指向包含 `.specify/` 的子项目，路径无效时直接失败。这对 monorepo 很关键：Git 根目录与 Spec Kit 项目根目录是两个不同概念。[根目录解析](https://github.com/github/spec-kit/blob/c173bf19a6654e3b05386ec3599349a55282b897/scripts/bash/common.sh#L4-L73)

当前 feature 目录解析的优先级是：

1. `SPECIFY_FEATURE_DIRECTORY`，相对路径以 Spec Kit 项目根目录为基准。
2. `.specify/feature.json` 的 `feature_directory`。
3. 没有上下文就报错。

`SPECIFY_FEATURE` **只给 feature 标签 / BRANCH 输出提供标识，不负责选择目录**。当前 core 不再从 Git 分支推断 feature；未设置标签时可用 feature 目录 basename 作为输出标签。因此不能在 worker worktree 上依赖 `codex/run-...` 分支名来定位 `specs/001-...`。`create-new-feature` 默认生成 `specs/<序号或时间戳>-<名称>`，即使变量仍名为 `BRANCH_NAME`，该 core 脚本的此段逻辑是在创建目录和写 `feature.json`。[feature 解析](https://github.com/github/spec-kit/blob/c173bf19a6654e3b05386ec3599349a55282b897/scripts/bash/common.sh#L75-L231)、[默认目录创建](https://github.com/github/spec-kit/blob/c173bf19a6654e3b05386ec3599349a55282b897/scripts/bash/create-new-feature.sh#L274-L387)

安装后的命令会重写模板路径：源码模板里的 `/memory/constitution.md` 不是操作系统根目录下的文件，安装转换会把 `memory/` 引用改为 `.specify/memory/`。不要直接把未渲染的上游模板当最终 agent prompt，也不要硬编码所有 agent 都使用 `/speckit.implement`：当前 Codex 使用 `.agents/skills/speckit-<name>/SKILL.md`，命令名受 integration 影响。[路径转换](https://github.com/github/spec-kit/blob/c173bf19a6654e3b05386ec3599349a55282b897/src/specify_cli/agents.py#L193-L219)、[Codex integration](https://github.com/github/spec-kit/blob/c173bf19a6654e3b05386ec3599349a55282b897/src/specify_cli/integrations/codex/__init__.py#L1-L65)

### 对本项目的检测建议

- 返回 detection 候选及证据，例如 `.specify`、feature index、`spec.md` / `plan.md` / `tasks.md` 组合；同时返回 `initialized`、`ready`、`ambiguous` 等不同状态，不能仅看到目录就称“可执行”。
- `--provider speckit --feature <path-or-id>` 是确定选择；多候选时列出候选，非交互模式不猜“最新编号”。
- `.specify/feature.json` 只表示上游当前选择，运行开始后记录选定目录及源文件摘要；后续用户切换 feature 不应让正在执行的 run 自动跟着切换。
- 当前 profile 使用显式 feature 目录。旧版 branch profile 是独立兼容层，在 fixtures 验证前不宣称支持；避免把旧行为硬塞到当前 profile。
- 在 worktree 中重新映射选定 feature 和项目根路径；原来的绝对 `feature_directory` 可能仍指向主工作区。各 worker 用私有环境变量固定选择，不竞争写主工作区的 `feature.json`。
- 只读 `detect` 不执行仓库里的任意脚本。原生读取失败可解释原因；调用官方脚本属于可选的可信工具桥接模式。

## 3. 可复用的 JSON 脚本接口与副作用

| 调用 | JSON 主字段 | 适配注意 |
| --- | --- | --- |
| `check-prerequisites.sh --json --paths-only` | `REPO_ROOT`、`BRANCH`、`FEATURE_DIR`、`FEATURE_SPEC`、`IMPL_PLAN`、`TASKS` | 当前源码纯路径解析，**不检查文档是否存在**，不写 `feature.json` |
| `check-prerequisites.sh --json --require-tasks --include-tasks` | `FEATURE_DIR`、`AVAILABLE_DOCS` | 检查 plan 和 tasks，默认不强制 spec；设置 feature 目录环境变量时可能持久化它 |
| 加 `--require-spec` | 同上 | 显式要求 spec 存在 |
| `setup-tasks.sh --json` | `FEATURE_DIR`、`AVAILABLE_DOCS`、`TASKS_TEMPLATE`、`TASKS_TEMPLATE_CONTENT` | 要求 spec / plan；解析组合模板；可能写 feature 选择，不应拿它做纯检测 |

`AVAILABLE_DOCS` 是可选资料清单：research、data-model、非空 contracts、quickstart，以及请求包含时的 tasks；它**不是所有必须读取文档的完整清单**。实现上下文还需明确加入 spec、plan、tasks 和 constitution。模板 override / preset / extension 可改变最终任务模板，不能假设固定模板文本。[检查脚本](https://github.com/github/spec-kit/blob/c173bf19a6654e3b05386ec3599349a55282b897/scripts/bash/check-prerequisites.sh#L99-L226)、[任务初始化脚本](https://github.com/github/spec-kit/blob/c173bf19a6654e3b05386ec3599349a55282b897/scripts/bash/setup-tasks.sh#L25-L84)

本次在临时 fixture 中直接运行了上游 Bash 脚本，未改动 clone：

| 验证 | 结果 |
| --- | --- |
| 没有 Git，feature 放在含空格的 `custom features/auth`，`--paths-only --json` | exit 0；正确绝对路径；`BRANCH=auth`；未创建 `feature.json` |
| 同一 fixture 使用普通 prerequisite JSON | exit 0；写入 `{"feature_directory":"custom features/auth"}` |
| fixture 只有 spec、plan、tasks | `AVAILABLE_DOCS` 只有 `tasks.md` |
| 移除 feature.json 与目录环境变量，只设置 `SPECIFY_FEATURE=001-auth` | exit 1；明确提示缺少 feature 目录 |

只验证了本机 Bash 路径契约；PowerShell / Windows 和实际模型执行不在本次运行验证范围。

## 4. tasks.md 的任务与并行语义

典型任务行：

```markdown
- [ ] T012 [P] [US1] Create User model in src/models/user.py
- [ ] T013 [P] [US1] Create Token model in src/models/token.py
- [ ] T014 [US1] Implement AuthService in src/services/auth.py (depends on T012, T013)
```

`[P]` 表示不同文件、没有未完成依赖的任务可以并行，不表示可以越过前置阶段。`[US1]` 是 story 归属，`P1` 则是 story 优先级；两者都不是完整依赖信息。任务 ID 不能限制为恰好三位数字，上游 changelog 已记录对更长 task ID 的支持修正。[任务生成格式](https://github.com/github/spec-kit/blob/c173bf19a6654e3b05386ec3599349a55282b897/templates/commands/tasks.md#L143-L214)、[ID 修正记录](https://github.com/github/spec-kit/blob/c173bf19a6654e3b05386ec3599349a55282b897/CHANGELOG.md#L205-L215)

默认任务模板包含：Setup → Foundational → 各 User Story → Polish。Foundational 阻挡所有 story，Polish 等待所有选定 story 完成。模板允许基础完成后独立 story 并行，也同时描述了按优先级顺序交付的策略；`implement` 通用提示词偏向逐 phase 执行。因此适配器必须读取**当前任务文档明确描述的执行策略**，不能把每个 `## Phase N` 一律解释成全局串行，也不能把所有 `[P]` 汇总成一个并发池。[任务模板依赖与策略](https://github.com/github/spec-kit/blob/c173bf19a6654e3b05386ec3599349a55282b897/templates/tasks-template.md#L163-L240)、[实现执行规则](https://github.com/github/spec-kit/blob/c173bf19a6654e3b05386ec3599349a55282b897/templates/commands/implement.md#L144-L175)

首版的保守导入规则建议：

1. Markdown parser 保留 task ID、checkbox、完整描述、story、phase、源码范围；跳过代码块、注释和模板示例，避免把文档说明当成待执行任务。
2. 解析明确任务依赖和当前文档中的阶段阻挡规则；先构造保守顺序，再只解除证据充分的并行边。相同文件、目录级重构、锁文件、公共接口或未知写范围应串行或分组。
3. `[P]` 是并行候选信号；还需前置依赖全部满足、文件写集合不冲突、预算和 worker 容量允许。模型可以提出依赖补充，Rust 必须校验未知引用、循环、跨 feature 引用和越权范围。
4. 若选择跨 story 并行，执行计划中明确列出依据和 story 间依赖。推断不确定就保留串行，并说明原因；不要编造原文没有的依赖语义。
5. story 完成后运行其 Independent Test / Checkpoint。测试任务是上游模板的可选项；存在且要求 TDD 时保留“先失败后实现”的顺序，不因适配器偏好自动重写任务计划。

任务分解应形成执行 sidecar，例如把较大 `T014` 拆成内部单元 `T014/a`、`T014/b`；上游仍保留 `T014`，只有所有子单元已集成且证据满足时才勾选它。这样可以利用 fresh context，又不强迫用户迁移或重排已有 spec。

## 5. 上游已经具备的自动化

### Workflow engine

当前 workflow 是有控制流的顺序步骤系统，包含 command、prompt、shell、gate、fan-out / fan-in、条件与循环等。内置 `speckit` workflow 自动串联 specify → review gate → plan → review gate → tasks → implement；不要把它误称为完全没有自动化，也不要说它已自动从 tasks.md 构造通用任务 DAG。[workflow 文档](https://github.com/github/spec-kit/blob/c173bf19a6654e3b05386ec3599349a55282b897/workflows/README.md#L1-L47)、[内置 workflow](https://github.com/github/spec-kit/blob/c173bf19a6654e3b05386ec3599349a55282b897/workflows/speckit/workflow.yml#L39-L74)

fan-out 默认顺序执行，`max_concurrency > 1` 使用有界 `ThreadPoolExecutor`；每项复制 StepContext，限制在途数量。暂停或失败后停止派发新的任务，已经运行的项可以跑完；返回值可能仅保留顺序上第一个停止项之前的前缀。这里的 context 复制是 Python 引擎的变量隔离，不等于文件系统隔离。`project_root` 被传给 integration，最终成为 subprocess 的 `cwd`；在这些代码路径里未发现每项 worktree、文件占用调度或自动合并。[并发实现](https://github.com/github/spec-kit/blob/c173bf19a6654e3b05386ec3599349a55282b897/src/specify_cli/workflows/engine.py#L1431-L1585)、[command 分派](https://github.com/github/spec-kit/blob/c173bf19a6654e3b05386ec3599349a55282b897/src/specify_cli/workflows/steps/command/__init__.py#L251-L261)、[子进程 cwd](https://github.com/github/spec-kit/blob/c173bf19a6654e3b05386ec3599349a55282b897/src/specify_cli/integrations/base.py#L372-L467)

workflow 状态按步骤保存，可 resume；上游文档明确说明恢复跟踪只到顶层 step index，嵌套控制流暂停可能重跑父步骤及其 body。默认 command 分派是流式输出，返回 stdout / stderr 为空，流式分支也不应用 timeout。这意味着任务级恢复、每 attempt 证据、强制超时和完整日志仍是本项目可解决的问题。[恢复边界](https://github.com/github/spec-kit/blob/c173bf19a6654e3b05386ec3599349a55282b897/workflows/ARCHITECTURE.md#L58-L78)、[流式 subprocess 行为](https://github.com/github/spec-kit/blob/c173bf19a6654e3b05386ec3599349a55282b897/src/specify_cli/integrations/base.py#L432-L467)

### Extensions 与 hooks

`.specify/extensions.yml` 可注册 `before_implement` / `after_implement` 等事件；模板要求真正调用 mandatory hook，打印 `EXECUTE_COMMAND:` 并不等于已经执行。当前 command 模板按 YAML 配置顺序处理，跳过有非空 condition 的 hook；`HookExecutor` 有自己的 condition / priority 逻辑，两条路径不能混为一谈。`settings.auto_execute_hooks` 当前是保留字段，并未控制实际执行。[hook 字段语义](https://github.com/github/spec-kit/blob/c173bf19a6654e3b05386ec3599349a55282b897/docs/reference/extensions.md#L247-L269)、[实际 hook 调用要求](https://github.com/github/spec-kit/blob/c173bf19a6654e3b05386ec3599349a55282b897/templates/commands/implement.md#L183-L211)

本项目执行完整 feature 时只运行一次对应的 before / after 生命周期 hook，不应每个 task worker 都完整执行一遍 `/speckit.implement`，导致多个 hook、全局扫描和任务文件写入互相竞争。不支持的 hook 能力应在规划阶段明确显示，不能静默声称与原生完整 workflow 等价。后续可以提供 Spec Kit extension 或 workflow shell step 来调用我们的 CLI，使用户从现有入口进入，但不要求用户先迁移到 extension。

## 6. Fresh context 与主上下文控制

上游 Codex integration 已使用 `codex exec <prompt>`，这是借鉴其非交互进程分派的直接入口。值得注意的是，上游 Claude integration 明确撤销了默认 `context: fork`：注释说明较大的分析结果被注回主会话，后续 fork 继承增长后的上下文，曾造成长会话卡住。不能把“用了 subagent”直接等同于“主上下文很轻”。[Codex 分派](https://github.com/github/spec-kit/blob/c173bf19a6654e3b05386ec3599349a55282b897/src/specify_cli/integrations/codex/__init__.py#L46-L65)、[撤销默认 fork 的说明](https://github.com/github/spec-kit/blob/c173bf19a6654e3b05386ec3599349a55282b897/src/specify_cli/integrations/claude/__init__.py#L24-L35)

建议采用以下执行边界：

- 一个任务 attempt 启动一个新的 agent 进程 / 会话，默认不携带前一个会话的 resume ID。agent backend 与 spec adapter 分离。
- 上下文包只含选定 task、相关 spec / plan 片段、constitution / 仓库指令、依赖交付摘要、文件范围、验证要求和可按需读取的资料路径。
- worker 不接收完整主线程历史，也不负责解释全仓库所有 task；主协调器保留图、状态、决策与摘要。
- 结果必须结构化：attempt ID、源码 task ID、实际变更文件、验证命令与结果、产物 / patch 引用、未决项、简短摘要。完整日志留磁盘，主上下文引用证据而非粘贴报告。
- 每个 attempt 有独立 worktree；父协调器管理合并、冲突解决和公共文件顺序。仓库里可能被 gitignore 的 `.specify` / `.agents` 等配置也要以显式只读上下文快照提供，否则新 worktree 会缺少规则。

## 7. 回写、质量门与 convergence

原生 implement 要求完成任务后在 `tasks.md` 写 `[X]`。同时它要求把 feature/checklists 作为**只读质量门**：这些复选框表达需求评审完成，不能因为代码实现了就替用户勾上。适配器应保持这两类状态的区别。[只读 checklist 规则](https://github.com/github/spec-kit/blob/c173bf19a6654e3b05386ec3599349a55282b897/templates/commands/implement.md#L54-L88)、[任务完成回写](https://github.com/github/spec-kit/blob/c173bf19a6654e3b05386ec3599349a55282b897/templates/commands/implement.md#L163-L175)

协调器应是上游任务状态的唯一写入者，按如下顺序回写：

```text
worker 结束
  → 验证结构化结果与实际 diff
  → 合并到受管理的集成工作区
  → 在集成结果上验证
  → 核对 task ID、原文 / 文件摘要、最新状态
  → 原子更新目标 checkbox
  → 持久化回写事件与证据
```

不能仅靠进程 exit 0、worker 自报完成或旧 checkbox 判断当前代码已满足任务。保存 task 来源范围与文本摘要；若用户重写或删除 task，停止该项自动回写并进入 replan，而非用旧行号覆盖新内容。重试回写须幂等，保留其他任务、顺序、文字、CRLF 与用户编辑。

当前 core 已有 `/speckit.converge`：在 implement 后，以 spec / plan / tasks 和 constitution 为意图来源检查代码差距，把剩余工作追加到新的 `Phase N: Convergence`，不重写旧 task，不改代码，已满足时保持 tasks.md 字节不变。因此“所有已有 checkbox 完成”只能称任务执行完成，最终还需需求和计划的独立验证。[converge 约束](https://github.com/github/spec-kit/blob/c173bf19a6654e3b05386ec3599349a55282b897/templates/commands/converge.md#L57-L102)

自动模式可支持有界的 implement → verify / converge → 新任务导入循环：设置最大轮数、重试预算、无进展判定和需求范围限制。是否追加任务必须记录为一次新的源文档版本和执行图 revision；不允许 worker 自发扩展需求并无限续跑。

## 8. 适配接口建议

以下是本项目建议的职责划分，不是上游 API：

| 接口 | Spec Kit 实现职责 |
| --- | --- |
| `detect(root)` | 只读发现 `.specify` 和 feature 候选，返回证据与能力 profile |
| `select(selection)` | 固定 project root、feature directory 与来源，不修改 feature.json |
| `inspect(selection)` | 检查 spec / plan / tasks / checklist / hook，返回 ready / blocked 及原因 |
| `load(selection)` | 文档快照、task、story、phase、来源引用与摘要 |
| `normalize(graph)` | 将明确依赖转成统一图，不删除原生依赖信息；不确定部分保守处理 |
| `context(task)` | 构建选定 task 的有界上下文包，映射 worker 中的路径 |
| `verify(result)` | 任务、story、feature 的分层验收；结果与证据单独存储 |
| `writeback(task, expected_revision)` | 协调器串行、幂等、乐观并发控制地只更新目标任务状态 |
| `native_bridge(capability)` | 可选调用经过确认的本地 Spec Kit 功能或安装后的 integration 命令 |

最小支持能力应声明为 `artifact-read` / `task-import` / `task-writeback`，而不是泛称“完整兼容 Spec Kit”。原生 hook、workflow、preset、converge 是分开的能力，按已验证情况暴露。

## 9. 建议的首版验收 fixture

1. 单 feature、多个 feature、只有 `.specify` 未创建 feature、OpenSpec 与 Spec Kit 共存。
2. feature.json 相对与绝对路径、空格、Unicode、无 Git、monorepo 子项目、Git worktree。
3. env override 与 feature.json 不一致；`SPECIFY_FEATURE` 不能被误当目录；detect 前后仓库字节不变。
4. 空任务、完成任务、`T1000`、重复 ID、同一 ID 被用户改写、Markdown 示例/注释里的假任务。
5. Setup / Foundational barrier、显式 story 依赖、`[P]` 相同文件、未声明文件范围、缺失依赖和循环。
6. worker 成功但验证失败、merge 冲突、两个 worker 完成、写回前用户编辑、写回后进程崩溃恢复。
7. 未完成 checklist、mandatory / optional / conditional hook，确保报告能力缺口且不误改 checklist。
8. converge 追加 task、无追加时保持源文件不变、有界循环与无进展停止。

本次完成了上游源码研究及第 2 / 3 类中的小规模 Bash 契约实测；其余条目是项目后续的验收要求，不代表已经完成实现或通过测试。
