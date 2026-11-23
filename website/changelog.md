# Changelog

这里记录面向用户的主要变化。完整提交和 Release 产物请看 [GitHub Releases](https://github.com/ZaneL1u/spec-autonomous/releases)。

## 0.2.1

### 少花 token

- 规划指引新增任务粒度要求：每个计划任务都由一个从零开始的新 worker 上下文执行，重试也是新上下文；同一模块或功能切片的相关源任务（通常 2–5 个，机械性改动可以更多）合成一个计划任务，只在写集真正独立、原生顺序或阶段屏障强制、或单个任务明显超出一次会话时才拆分，不再默认一个复选框一个任务。指引附加在 request key 之外，已签发的规划包仍能匹配回执。
- 新增 `[host.models]`：按工作包类型（`roadmap` / `native-planning` / `plan-tasks` / `implement` / `audit` / `converge`）给宿主一个模型提示，出现在 `work[].requirements.model`，宿主用它启动子上下文（Claude Code 即 Agent 工具的 `model`）。未配置时包的内容与之前完全一致；未知类型或非法模型名按 `invalid_config` 拒绝。
- `autonomous` / `auto` skill 与宿主协议新增开销规则：宿主只做路由，不自己读工作包再转述；项目特有事实写一次文件传路径；优先使用只带 shell 与文件工具的 worker agent 类型；不在包外另起辅助 agent；`no_progress`、`attempts_exhausted`、`repair_repeated` 是停下报告的信号而不是重试。

## 0.2.0

### 少做重复工作

- 增量审计：审计条目可带 `files`（结论依赖的文件）。再次审计同一 phase 时，上一轮已通过、且这些文件、写过它们的任务写集和需求原文自上轮审计以来都没变的条目沿用原结论（标注 `carried_over` 与来源 attempt），审计包只列需要重审的 id；全部可沿用时不再签发审计包。没有 `files` 的条目照旧每次重审。
- 里程碑审计沿用 phase 审计结论：只属于一个 phase、已在该 phase 审计中通过且依赖未变的验收项直接沿用，审计包只含里程碑独有项（如 `milestone/<id>/goal`）与受影响条目。单 phase 里程碑不再把同一份审计重做一遍。
- 验证按改动范围收窄：新增 `[execution] verification_scope = "affected" | "full"`（默认 `affected`）。修复轮或单任务被接受后，combined 与 phase 门槛只重跑当前任务自己的检查、配置的项目 / 阶段检查，以及改动触及其 `writes` / `reads` 的其他任务的检查；其余检查沿用其在祖先提交上的通过证据（`reused_from.scope = "affected"`，附 owners 与变更文件）。里程碑门槛由 `milestone_verification_scope` 控制，默认 `full`。规划指令要求为每个任务声明 `reads`。
- 核对型任务：计划任务新增可选 `verify_only: true`（`writes` 必须为空）。协调器不再为它签发 worker，而是直接在集成检出上运行其检查（`SA_VERIFY_STAGE=verify-only`）：通过即接受并勾选原生任务；失败才签发 implement 修复包，`failure` 带失败检查的输出末尾。

### 宿主协议与恢复

- 工作包提示词只保留摘要（约 3KB），`input.json` 按包类型裁剪；原生任务的子条目（规则与验收命令）作为 `details` 进入快照，规划者不再只看到复选框首行。
- converge 只把未通过的审计条目作为缺口传入；no_progress blocker 只列每个任务最近一次失败，不再把历次错误无限拼接。
- blocked 后重派的 implement 带上上一次候选的改动（`previous_candidate`），worker 不必从零重做；blocked worker 可用 `requested_writes` 申请追加写集，`run.revise` 也可以修订任务写集。
- 检查运行中被取消（宿主中断、`run.pause`）不再记为失败，候选保留，`prepare --run-id` 原样重新验证。
- 托管的 `openspec` 在检查环境中按托管 provider 执行；预检只缺可执行文件或目录时保留候选计划，修好环境后只重跑预检，不重新规划。
- 新增 `[execution] init_submodules` 与 `[hooks.worker_prepare]`：worker 检出也能初始化子模块、准备依赖，与验证环境一致；doctor 提示已存在却未挂上的 `*-prepare` 脚本。只读的规划与审计检出同样准备（`[execution] prepare_read_only`，默认 `true`），审计 worker 不必再借主检出的 `.venv`；钩子用 `SA_WORKER_KIND` 区分包类型，运行后留下已跟踪改动即按 `worker_prepare_failed` 处理，复用的只读检出会再跑一次幂等钩子。
- 配置漂移提示：源配置在 run 启动后改变时，`prepare` / `next` / `apply-result` 的 `warnings` 给出 `config_drift`；full 视图显示 hooks。`reload_config` 保留此前的时长延长；`prepare` 命令行新增 `--reload-config`、`--extend-seconds`、`--max-attempts`。
- `dirty_checkout` 列出具体路径与类别；需要续跑 run 的 blocker 写明 `prepare --run-id`；能力目录列出隐藏能力，`limits` 说明超时与心跳是否强制；`discussion_auto` 在 blocker 带 `needs_input:` 前缀时也生效。
- 集成过程中的中断不再计为失败：暂停 / 取消请求、进程中断或 run 截止时间在开始集成前或两条检查之间（含合并验证）被发现时，回执与检查运行中被取消一样保持 `submitted`，不计入 `max_attempts`，`prepare --run-id` 原样重新验证。
- skills（`autonomous` / `auto` / `resume`）改为 `prepare --run-id <id>`（不再带 `--change` / `--feature`），并说明 `verification_environment_unavailable`（修环境后只重跑预检、不重新规划）、`worker_prepare_failed` 与 `warnings[]` 中 `config_drift:` 的处理。
- 自动追加写集与人工 `run.revise` 一样重置该任务的重试计数，扩写集不再消耗 `max_attempts`；每个任务在整个 run 中至多自动追加 10 条路径。
- 检查失败的错误直接说明失败的是哪条命令、为什么：带实际执行的 argv、cwd、退出码和脱敏后的 stderr / stdout 末尾几行，日志路径放在后面；重派包的 `failure` 与 no_progress blocker 随之受益。
- run 预算改按活动时间计：协调器运行与宿主持有工作包（`awaiting_host`）的时间计入，停在 `needs_input` / `paused` 的时间不计；`budget_exhausted` 说明 72 小时上限下最多还能延长多少，到顶时直接提示需要 `unsafe_budgets`。旧 run 从已记录的用量继续累计。
- `ff-original` 交付不再被无关的未跟踪文件挡住：主检出里的笔记、草稿只要不与交付 diff 的路径重叠（含目录前缀冲突）就留在原处，交付照常快进；已跟踪改动或冲突的未跟踪文件仍进入 `delivery_pending` 并保留 verified 分支，blocker 以 `blocking: {...}` 列出挡住交付的具体路径（`untracked` 只列冲突项，`ignored_untracked` 计数其余）。`archive` 的最后一步快进同样处理。
- 新增可选的 `[execution] baseline_checks`（默认关闭）：计划被接受后先在集成检出上跑一次已能运行的检查，把基线上已经失败的检查作为 `baseline_failure:` 警告报告，并写进该 phase 的实现包，避免它们拖到最后一个任务才暴露。

### 计划、结果与审计

- 检查可以声明 `env`（只对该检查生效的非敏感环境变量，如 `PYTHONDONTWRITEBYTECODE=1`）：键名受校验，不能覆盖 `PATH` / `HOME` / `SA_*` / `GIT_*` 等，凭证类名字被拒绝；值进入验证复用身份，证据清单只记变量名。规划包的指令与结果 schema 同步说明。
- 规划指令补充按框架的并行规则：`[P]` 规则只针对 Spec Kit；OpenSpec 任务没有原生并行标记，协调器按 `depends_on` 与写集冲突决定串行或并行，规划者用 `depends_on` 表达真实依赖、给独立任务精确且互不重叠的写集。这段说明不进入请求键。
- 规划指令明确任务 `writes` / `reads` 可以用目录前缀和 glob（如 `server/*/migrations/*.py`、`tests/**`），并说明保守的冲突串行规则；glob 或目录项不再能放开原生规格上下文文件，需逐字列出。
- implement 与 audit 的结果可以带 `deferred_evidence`（`live` / `manual` / `external`），表示要宿主在真实环境补的证据：它不算失败，协调器在 run 视图、`next`、`progress` 中列出，里程碑完成时以 `deferred_evidence_pending` 警告和待办列表交给宿主。
- 审计包的执行证据可以直接用：`snapshot.metadata.execution_evidence.checks` 列出每条检查最近一次的 `argv` / `executed_argv`、`cwd`（含计划声明的 `declared_cwd`）、`stage`、`revision`、`exit_code`、`duration_ms` 与 `reused_from`，审计者不必再逐个翻 `verify-*/check-*/result.json` 才知道全量测试跑在哪个提交、跑的什么命令；`execution-evidence.json` 的 `verification` 每条在原字段后追加同样的信息。原有字段不变，新内容不进入请求键。
- 审计不再把「规格只举例、实现多支持了几种」判失败：审计包要求把不矛盾的合理扩展判为通过并以 `extension:` 标注证据，协调器对这类条目不排修复轮，converge 也不为它们生成修复任务；与规格矛盾或缺失仍判失败。

### 安全

- 写集追加不再越权：原生上下文的父目录（如 `openspec/changes/<id>`）和能匹配它们的 glob 与上下文文件本身一样被拒绝（`write_scope_forbidden`）；blocked worker 的 `requested_writes` 只自动授权字面文件路径，模式与目录返回 `write_scope_not_literal`，需经人工 `run.revise`。

### 环境与接入

- 新增 `[execution] worktree_root`（`git` / `sibling` / 绝对路径），可把交给 worker 的 worktree 放到 `.git/` 之外，避免跳过 `.git/` 的搜索工具和 IDE 索引找不到代码；默认值不变（仓库外目录的权限与宿主沙箱风险），doctor 以 `worktree_root_notice` 提示。已有 run 不迁移。
- MCP 接入：`init --mcp` 在 `spec-autonomous` 不在 PATH 上时写入 Node + 启动脚本的绝对路径，避免宿主启动 MCP 时 `ENOENT`；doctor 检查已写入的 MCP 命令能否启动。安装文档补充没有 Bun 时从本地源码目录安装的步骤（`npm install --omit=dev` + `build-native` + `npm install -g .`）。
- 开启 `init_submodules` 后 `run.cleanup` 与自动维护能移除干净的 worker / 集成工作树：执行时确认父仓库和子模块都干净后再 `--force` 移除，子模块有改动或工作树被锁定时仍保留。
- doctor 的 `submodules_warning` 改为看 worker 侧：仓库有子模块、`init_submodules = false` 且没有 `worker_prepare` 时提示，只配了 `verification_prepare` 的项目也会收到提示。
- 规划与 worker 工作包说明 `openspec` 映射：PATH 上没有 `openspec` 时，`managed_commands.openspec` 与 prompt 的 `TOOLS:` 给出协调器实际执行的绝对 argv 和 `providers exec` 写法；`verification_environment.executables` 新增 `pnpm`、`uv`、`openspec`。

- `build:native` 先复制到临时文件再改名覆盖：在 macOS 上原地覆盖一个运行过的二进制会沿用旧的签名缓存，新二进制第一次启动就被 SIGKILL（退出码 137，launcher 报 `cli_metadata_failed`）。

### 升级须知

- 账本 schema 升到 5（表结构同 schema 4），首次写入前自动备份为 `state.v4.<uuid>.backup.db`；之后 alpha.16 及更早的 CLI 会以 `schema_unsupported` 拒绝该账本，避免它们读不了 `init_submodules` 配置或在重新规划时丢掉写集修订。升级后重启项目的 MCP 服务，详见[长任务维护](https://github.com/ZaneL1u/spec-autonomous/blob/main/docs/long-running-runs.md)。

## 0.1.0-alpha.16

- 验证证据按源码树哈希复用：同树同命令同环境的检查只执行一次，phase/post-hook、里程碑门槛和审计等待不再重复跑；无 hook 改动时跳过 post-hook。与 alpha.15 的 `reuse_verification` 合并为同一开关，`verification_reuse = "none"` 或 `reuse_verification = false` 均可关闭。
- 续跑不再清零已完成 phase 与计划：原始检出 fast-forward 被识别，外部改动按声明范围只重跑受影响 phase 的命令门，原生改动只重开本 phase。
- 在途工作不因源漂移被撤销，合并延后到回执落地；追加的原生任务增量扩展计划；通过的审计按树缓存；同一失败在一轮修复后再次出现即 `needs_input: repair_repeated`。
- 新增 `task_verification` / `phase_verification` / `milestone_verification` 分阶段检查；拒绝内嵌编码脚本的 argv；preflight 把任务将创建的脚本视为 deferred；规划包携带真实运行时版本、package scripts、测试文件与历史拒绝诊断。
- 预算硬上限（`max_attempts ≤ 10`、`max_repair_rounds ≤ 4`、`run_timeout_seconds ≤ 72h`），`unsafe_budgets = true` 才可超出；doctor 对超默认两倍的预算告警。
- 宿主协议：`prepare` / `apply-result --claim-next` 带 `host` 自动认领并返回令牌；`next` / `request.get` 向拥有者找回令牌；`work.revoke` 可用拥有者身份；协调器锁有界等待；心跳过期不再拒收；`run.revise` / `run.cleanup` 非破坏范围一步应用；discussion 卡片随 prepare 返回。skills 改为路由式说明，协议见 [宿主协议](https://github.com/ZaneL1u/spec-autonomous/blob/main/docs/host-protocol.md)。
- 已关闭的写入 attempt 在补丁落盘后自动回收（`reclaim_worktrees` / `retain_failed_worktrees`，`automatic_cleanup = false` 一并关闭），只读规划/审计检出沿用 alpha.15 的复用与交接维护；账本 schema 4 把 attempts 与 evidence 拆成行并增量写入，首次写入迁移并备份；新增只读 `run.forensics` 与 `host_capacity`。
- 升级须知：预算上限对已有 run 的账本配置同样生效；账本迁移后旧版 CLI 只能读取不能写入；已有 MCP 会话需在安全交接点重连。详见[长任务维护](https://github.com/ZaneL1u/spec-autonomous/blob/main/docs/long-running-runs.md)。

## 0.1.0-alpha.15

- 串行且基线未变化时直接验证写入 worker 的集成候选，减少相同代码的重复门禁。
- 完整同输入验证收据可在审计等待中复用；源码、环境、工具或证据变化时重新验证。
- 安全交接点自动回收已结束的干净工作树，保留活跃任务、未提交改动、分支和证据。
- 复用关闭的只读规划/审计检出，按声明范围判断受影响阶段，未知范围仍保守重验。
- 修复消费者产物目录与协调器日志混用的问题；项目侧示例补丁保持单独提供。
- 现有 run 可保留进度继续执行；升级需统一切换写入器，不能混用新旧程序。

## 0.1.0-alpha.14

- 长任务可在安全交接点预览并清理已结束工作树，保留活跃所有者、恢复证据与未完成任务补丁。
- 验证波次提供独立产物目录、准备钩子、耗时与失败诊断，合并检查去重。
- 重新规划保留已审查的验证修订；修复写入范围误判、悬挂回执和心跳参数兼容。
- 进度支持显式关联执行仓库，并显示原生清单、重试开销和资源压力。
- doctor 检测同版本 skills 内容漂移；修复 Homebrew npm 查找和 CLI 本地化参数解析。
- 升级继续原 run，已运行的命令保留旧可执行文件；详细步骤见[长任务维护](https://github.com/ZaneL1u/spec-autonomous/blob/main/docs/long-running-runs.md)。


- 启动层、脚本与测试全部改用 TypeScript 编写，发行产物仍是 JavaScript：`packages/cli/src/*.mts` 经 tsdown 构建为提交在仓库中的 `packages/cli/{bin,lib}/*.mjs`，CI 校验产物与源码一致。
- 新增 VitePress 中文文档站和 GitHub Pages 自动发布。
- 归档已完成的 OpenSpec changes，并把 host-driven 边界同步到主规范。
- 对齐 Smart Discuss 后的 batch claim、host protocol 和 E2E fixture。
- 补全主规范 Purpose，使 OpenSpec strict validation 全部通过。

## 0.1.0-alpha.13

- 明确敏捷 milestone、phase/spec、workstream 和多团队协作模型。
- 区分 milestone 内部 revision 与产品发布版本。
- 固化 host-driven 架构：CLI 生成工作包、验证宿主回执，不创建 Agent。
- 增加可恢复验证修订、批量领取和终态资源清理能力。

## 0.1.0-alpha.12

- Auto 把 Smart Discuss 作为每个 phase 的自动前置门。
- 无灰区时继续，有用户决策时明确暂停。

## 0.1.0-alpha.11

- 增加可点击的 Smart Discuss phase 推荐与替代选项。
- 决策通过 source hash CAS 持久化。

## 0.1.0-alpha.10

- 构建并打包 alpha.10 macOS 二进制。
- 完善 dogfood 诊断与验收记录。

## 0.1.0-alpha.9

- 支持从错误 verification argv / cwd 恢复同一个 run。
- 保留已验收工作和历史失败证据。
- 降低宿主上下文与重复规划开销。

## 0.1.0-alpha.8

- 新增交互式初始化。
- 空项目可以选择 OpenSpec / Spec Kit、Codex / Claude Code 和 MCP。
- 自动化环境支持完整非交互参数。

## 0.1.0-alpha.7

- 补全简体中文 CLI 展示。
- 增加系统语言检测和显式语言覆盖。

## 0.1.0-alpha.6

- CLI 支持 English / 简体中文本地化。
- 保持命令名、JSON 字段、错误代码和状态值稳定。

## 0.1.0-alpha.5

- 支持从私有 Git SSH 地址安装 npm facade。
- 首次运行通过 GitHub Release 下载并校验原生二进制。

## 版本说明

当前仍是 alpha：

- GitHub Release 安装的实际发布范围是 macOS arm64；
- 公开 npm registry 尚未发布；
- 六平台发行 workflow 是构建能力，不代表所有平台已有可下载产物；
- 上游 OpenSpec / Spec Kit 兼容性按仓库锁定版本和 contract tests 声明。
