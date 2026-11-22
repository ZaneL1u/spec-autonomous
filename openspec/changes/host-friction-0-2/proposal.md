## Why

0.1.0-alpha.16 在一个真实宿主上连续跑了两天（Friday：Django + Vue 单仓，带子模块，后端检查在 `server/` 子目录里执行，宿主是 Claude Code），暴露出的问题集中在"重复劳动、环境不一致、误计失败、宿主看不懂停止原因"四类：

- 一次三行的修复让 combined 与 phase 门槛把本 phase 所有已接受任务的检查各重跑一遍；重复审计把 23 条验收全部重新发给审计者，单 phase 里程碑又把同一份审计做了第二遍；tasks.md 里"依次运行以下检查"的核对任务也签发 worker，worker 跑一遍全量测试，协调器再跑一遍。
- worker 检出没有子模块内容和 `.venv`，任务自己的检查在 worker 里失败、在最终波次通过；只读审计 worker 只能借主检出的环境。每个 `prompt.md` 内嵌整个 `input.json`（约 100 KB），worker 写脚本解析自己的提示词。
- 宿主进程中断时正在运行的检查被记为失败并计入 `max_attempts`；run 停在 `needs_input` 过夜，墙钟预算被耗尽；`reload_config` 把此前的延长丢掉；无进展 blocker 把历次错误拼成几 KB。
- 主检出旁一份调研笔记（未跟踪文件）让完全验证过的 run 停在 `delivery_pending`；`dirty_checkout` 不说是哪些文件。宿主按"用同一个 run_id prepare"去带 `--change --run-id`，被 clap 拒绝；检查失败只给三个日志路径；worker worktree 在 `.git/` 下，跳过 `.git/` 的搜索工具找不到代码；`init --mcp` 写下的裸命令让 MCP 启动报 `ENOENT`。
- 任务必须改写集之外的既有测试时只能 blocked，再由宿主手工改源分支重派；补上自动追加写集后，评审又发现 blocked worker 可以申请原生变更目录的父目录，把 `tasks.md` 和规格一并拿到写权限。

这些改动已在 `release/0.2.0` 上先行实现并通过契约测试，本变更补记其行为契约。

## What Changes

### 少做重复工作

- 审计条目可带 `files`。再次审计同一 phase 时，上次已通过、需求原文未变、且自上次审计提交以来的改动不触及这些文件、写过它们的任务写集、原生文档、全局输入或未声明文件的条目沿用原结论（`carried_over`），审计包只列需要重审的 id；全部可沿用时不签发审计包。
- 里程碑审计对只属于一个 phase 的验收项沿用该 phase 的审计结论，审计包只含里程碑独有项（如 `milestone/<id>/goal`）与依赖已变的条目。
- 新增 `[execution] verification_scope = "affected" | "full"`（默认 `affected`）：combined 与 phase 门槛里由其他任务声明的检查，在上次实际执行通过之后的改动没有触及其 owners 的 `writes` / `reads`、检查路径操作数、准备钩子、全局输入、原生文档或未声明文件时沿用通过证据（`reused_from.scope = "affected"`）；配置的项目 / 阶段检查与当前任务自己的检查总是执行。`milestone_verification_scope` 默认 `full`。检查的路径操作数按检查自己的 cwd 解析。
- 计划任务可设 `verify_only: true`（`writes` 为空、自带检查）：协调器在集成检出上直接运行其检查，通过即接受，失败才签发带失败输出的 implement 修复包。
- 审计包的 `execution_evidence.checks` 按检查给出最近一次的 argv、cwd、stage、revision、退出码、耗时与 `reused_from`。
- converge 只收到未通过的审计条目；不与规格矛盾的合理扩展判为通过（`extension:`），不排修复。

### worker 环境与工作包

- 新增 `[execution] init_submodules`、`[hooks.worker_prepare]`；`[execution] prepare_read_only`（默认 `true`）让只读的规划与审计检出同样准备。钩子留下已跟踪改动按 `worker_prepare_failed` 处理。
- 新增 `[execution] worktree_root = "git" | "sibling" | <绝对路径>`，默认 `git` 不变。
- `prompt.md` 只保留规则、`input.json` 路径与摘要；`input.json` 按包类型裁剪；原生任务的缩进子条目作为 `details` 交给规划者。
- 重派的 implement 带上一次候选的改动（`previous_candidate`）。
- 托管的 `openspec` 在检查中按 provider 命令执行，规划包与 worker 包说明映射（`managed_commands`、`TOOLS:` 行）。
- 检查可声明非敏感的 `env`；保留名与凭证类名字被拒绝。
- implement 与 audit 结果可带 `deferred_evidence`，run 完成时以 `deferred_evidence_pending` 警告交给宿主。

### 恢复、预算与交付

- 检查运行中被取消，或暂停 / 取消 / 进程中断 / 截止时间在集成开始前或两条检查之间被发现时，回执保持 `submitted`，不记失败、不计入 `max_attempts`。
- run 预算按活动时间计（协调器运行 + `awaiting_host`）；`reload_config` 保留此前的延长；`budget_exhausted` 给出最小延长量与 72 小时上限说明。
- 无进展 blocker 只列每个未完成任务最近一次失败；检查失败说明 argv、cwd、退出码和脱敏的输出末尾。
- 只缺可执行文件或目录的预检失败保留候选计划，修好环境只重跑预检。
- 可选 `[execution] baseline_checks`（默认关闭）报告基线上已经失败的检查。
- `ff-original` 交付与 `archive` 最后的快进只被已跟踪改动、子模块改动和与交付 diff 冲突的未跟踪文件挡住，blocker 列出具体路径。
- 开启子模块后，干净的 worker / 集成工作树可被 `run.cleanup` 与自动维护移除。

### 写集授权

- `run.revise` 新增 `task_writes`；blocked worker 可带 `requested_writes`。自动授权只接受字面文件路径，原生上下文及其父目录、匹配它们的 glob 一律 `write_scope_forbidden`；授权开启新的重试周期，每个任务整个 run 至多自动追加 10 条路径。
- 规划指令说明 `writes` / `reads` 可用目录前缀与 glob，冲突按字面前缀保守串行；glob 或目录项不能放开原生上下文文件。OpenSpec 任务没有 `[P]` 标记，按 `depends_on` 与写集冲突决定串行或并行。

### 宿主接入

- 源配置在 run 启动后改变时返回 `config_drift` 警告；full 视图显示 hooks；`prepare` 新增 `--reload-config`、`--extend-seconds`、`--max-attempts`。
- 需要续跑的 blocker 与 skills 写明 `prepare --run-id <id>`；`dirty_checkout` 列出路径与类别；能力目录列出隐藏能力，`limits` 说明不强制宿主 worker 时长；`discussion_auto` 在 blocker 带 `needs_input:` 前缀时生效。
- `init --mcp` 在 `spec-autonomous` 不在 PATH 时写入绝对的 Node + 启动脚本命令；doctor 检查 MCP 命令能否启动、子模块与未挂接的 prepare 脚本、`worktree_root`。
- **BREAKING（账本）**：账本 schema 升到 5，alpha.16 及更早的 CLI 以 `schema_unsupported` 拒绝升级后的账本。

## Capabilities

### New Capabilities
- 无。

### Modified Capabilities
- `verified-integration`: 按改动范围收窄的门槛验证、核对型任务、基线失败报告、失败检查说明、检查环境变量与托管 openspec；交付只被冲突的未跟踪文件挡住。
- `milestone-autonomy`: 增量审计、里程碑审计沿用 phase 结论、扩展判通过与只传失败缺口、延后的真实环境证据；预算按活动时间、无进展 blocker 精简。
- `agent-execution`: worker 检出的位置、子模块与准备钩子，只读检出准备，提示词摘要与包裁剪，继承上一次候选。
- `task-planning`: OpenSpec 并行规则、glob / 目录写集与字面原生上下文、核对型任务、原生子条目。
- `host-work-protocol`: 中断不计失败，写集自动授权的安全边界，配置漂移与续跑参数。
- `recoverable-autonomy`: `run.revise` 修订写集，只缺环境时保留计划，带子模块的干净工作树可回收。
- `run-recovery`: 账本 schema 5。
- `host-interface`: 能力目录列出隐藏能力、可启动的 MCP 命令、明确的续跑命令。

## Impact

- **账本**：`state::SCHEMA_VERSION = 5`，表结构与 schema 4 相同。只有首次可写打开才迁移：schema 4 先以 `VACUUM INTO` 备份为 `state.v4.<uuid>.backup.db`，再在一个事务里把 `user_version` 设为 5，不重写任何 run、attempt、evidence 行；schema 2/3 照旧备份、拆行并标为 5。只读打开与 `run.pause` / `run.cancel` 不迁移。迁移后 alpha.16 及更早的 CLI 读写都报 `schema_unsupported`；回退需停掉新服务并把备份恢复为 `state.db`。升级后需重启项目的 MCP 服务。
- **新配置**：`[execution] verification_scope`、`milestone_verification_scope`、`init_submodules`、`prepare_read_only`、`baseline_checks`、`worktree_root`；`[hooks.worker_prepare]`；检查对象的 `env`；计划任务的 `verify_only`（及规划指令要求的 `reads`）；`run.revise` 的 `task_writes`；WorkerResult 的 `requested_writes`、`deferred_evidence`；审计条目的 `files`；`prepare` 命令行的 `--reload-config` / `--extend-seconds` / `--max-attempts`。默认值为 `false` 或与旧行为一致的选项在序列化配置中省略。
- **兼容性**：JSON / MCP 既有字段、状态值与错误码不变，只新增可选字段、警告前缀与 blocker 原因。请求键仍按升级前的指令与未裁剪快照计算，升级前签发的规划、审计、converge 包照常匹配回执；不带新字段的回执与 attempt 字节不变。run 启动时冻结配置：已有 run 不迁移 worktree 位置；没有活动时间锚点的旧 run 从已记录的 `elapsed_ms` 继续累计；没有配置哈希的旧 run 在 `reload_config` 前不报漂移。
- **代码与文档**：Rust core（engine、scope、verification、plan、work_packet、state、cleanup、reclaim、doctor、capabilities）、CLI 元数据与 npm 启动层、skills、`docs/host-protocol.md`、`docs/run-revision.md`、`docs/long-running-runs.md`、`website/reference/configuration.md` 与 `website/changelog.md`。不启动 agent，不修改消费项目。
