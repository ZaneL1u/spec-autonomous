## Context

见 proposal.md。0.1.0-alpha.16 已有同树证据复用（`verification_reuse = "same-tree"`）与完全通过审计的缓存（verification-evidence-reuse），但两者都要求整棵树不变；真实宿主上的修复轮几乎总会改动树，所以重复执行没有减少。其余问题来自 worker 检出与验证检出的环境不一致、把"没有被判定"当作"失败"、以及给宿主的提示不可执行。

本变更是事后补记：每项决策对应 `release/0.2.0` 上已合入并有契约测试的提交，tasks.md 列出对应关系与验收命令。

## Goals / Non-Goals

Goals：
- 修复轮与重复审计只重做改动可能影响的部分，无法证明无关时回到原有的全量行为。
- worker、只读审计者与协调器验证看到同一套依赖环境。
- 没有被判定的工作（被中断、被取消、在等人）不消耗失败预算与时间预算。
- 每个停止都告诉宿主具体原因和下一条可执行命令。
- 写集可以在运行中扩大，但不能借此获得原生规格与任务清单的写权限。

Non-goals：
- 不改变 JSON / MCP 既有字段、状态值与错误码，不改变请求键。
- 不改变 worker worktree 的默认位置，不迁移已有 run。
- 不在 CLI 中启动 agent、执行宿主侧的真实环境验证或替宿主勾销延后证据。
- 不做跨 run 复用。

## Decisions

### 1. 验证与审计按改动收窄：判定规则

两者共用 `scope::invalidated(run, checkout, from, to, changed, related, context)`，只回答一个问题："在 `from` 上得出的结论能否在 `to` 上继续成立"，答"否"的条件按顺序：

1. `from` 不是 `to` 的祖先、Git 失败、或改动落在项目目录之外 → 无法得到变更列表，结论不成立。
2. 变更路径落在 `related` 内（验证：owners 的 `writes` / `reads`、检查 argv 的路径操作数、`verification_prepare` 的路径；审计：条目的 `files` 与写过这些文件的任务写集）→ 不成立。
3. 记账改动跳过：`.spec-autonomous/plans/*`，以及只勾选复选框或在末尾追加整行的 `tasks.md`。新建或改写的 `tasks.md` 不算记账。
4. 全局输入 → 不成立：依赖清单与锁文件、共享测试配置（`conftest.py`、`pytest.ini`、`tsconfig.json` 等）、`.gitmodules`、协调器配置、`AGENTS.md`、`openspec/config.yaml` 与宿主技能目录。
5. 原生文档（当前上下文内的规格、设计、任务）→ 不成立。
6. 不在任何计划任务写集内的文件（owner 未知）→ 不成立。

验证侧另有：只有实际执行并通过的结果才能成为沿用依据（失败、超时、取消、改动了源码树的结果从不沿用）；命令身份含 argv、cwd、环境、准备钩子、超时与检出类别但不含树；owners 中任何任务 `writes` 或 `reads` 为空时无法证明无关，总是重跑。配置的项目 / 阶段 / 里程碑检查与当前任务自己的检查不参与沿用，总是执行（仍可走同树复用）。

审计侧另有：需求原文必须逐字未变；没有 `files` 的条目（包括升级前的回执）每次都重审；`carried_over` 只能由协调器写入，worker 回执里出现即以 `audit_contract_error` 拒收。里程碑审计对没有里程碑范围结论、且只属于一个 phase 的条目取该 phase 的结论，原生文档只看该 phase 自己的文档；被多个 phase 共享的 id 总是重审。

取舍：
- **保守兜底**。规则只做"可以证明无关"的判断，未知 owner、未声明的 `reads`、全局输入、原生文档改动全部回到重跑 / 重审。代价是声明不全的计划收益很小；规划指令因此要求每个任务声明 `reads`，但不把缺 `reads` 作为拒绝理由。
- **里程碑门槛默认 `full`**。`milestone_verification_scope = "full"` 让交付前的最终树把被沿用过的任务检查实际执行一遍（本树已执行的仍按同树复用），因此按范围沿用只减少中间门槛的重复，不减少交付前的独立证据。可显式设为 `affected`。
- **路径操作数按检查 cwd 解析**。最初把检查的 cwd 本身当作输入，导致在 `server/` 里运行的检查依赖整个 `server/`，Friday 上收窄从未生效；改为把 argv 中的路径操作数按 cwd 解析成项目相对路径。
- **开关**：`verification_scope = "full"` 关闭验证侧收窄；`verification_reuse = "none"` 同时关闭同树复用与按范围沿用。审计侧没有开关，不带 `files` 即等同旧行为。
- 请求键仍按原审计指令计算，升级前签发的审计包照常匹配回执。

### 2. 核对型任务（`verify_only`）

计划任务可设 `verify_only: true`，校验要求 `writes` 为空且任务自带 `verification`，否则 `invalid_plan`。依赖就绪时协调器不签发 worker，而是在集成检出上运行 `task_checks`（`SA_VERIFY_STAGE=verify-only`），记一个 `kind: "verify"` 的 attempt：

- 通过：经只勾选原生任务的集成 intent 接受（提交 `sa: accept <task> (verify-only)`）。
- 失败：attempt 记 `failed`，`error` 带失败检查的 argv 与输出末尾；下一步签发 implement 修复包，`failure` 即这段内容。修复包 `writes` 为空，即开放范围、与其他任务串行。
- 取消：attempt 记 `superseded`，续跑重新运行检查。

取舍：省掉"worker 跑一遍、协调器再跑一遍"的重复，同时保留失败时由 fresh worker 修复的路径。`verify` attempt 不计入 `max_attempts`，因为它不是语义工作；修复包是普通 implement，照常计数并经过 combined 验证。修复包写集开放带来串行，但核对失败本就意味着原因未知。

### 3. 写集授权的安全边界

`run.revise` 的 `task_writes` 与 blocked worker 的 `requested_writes` 都只追加，不删除；路径规范化后必须在 `.git/`、`.spec-autonomous/` 与该 phase 的原生上下文之外，且不与其他在途 implement 的写集冲突。

- 原生上下文的判定用与集成相同的 `plan::allowed`：集成把每条写集当作覆盖其下所有文件，所以 `openspec/changes/<id>` 这样的父目录、以及能匹配上下文文件的 glob，与上下文文件本身一样是 `write_scope_forbidden`。否则 blocked worker 申请一个目录就能拿到 `tasks.md` 与规格的写权限。
- 自动授权只接受字面文件路径：glob、带尾部斜杠的路径、在集成检出或 worker 检出中已存在的目录都返回 `write_scope_not_literal`，并附可直接执行的 `run.revise` 输入交人工评审。否则"至多 10 条"的上限没有意义。
- 自动授权与人工 `run.revise` 一样开启新的重试周期：被 blocked 的 attempt 与此前窄写集下的失败不再计入 `max_attempts` 与无进展判断。为防止"授权—blocked—授权"无限循环，每个任务在整个 run 中至多自动追加 10 条路径，超出为 `write_scope_limit`。
- 重派的 implement 带 `previous_candidate`，worker 从 blocked 时的改动继续。
- 规划时 glob 或目录写集照常允许（文件名在实现前未知），但不能放开原生上下文文件，需逐字列出；冲突按字面前缀保守判定为串行。

### 4. 中断不计失败

检查运行中被取消（宿主进程中断、`run.pause`、截止时间，证据 `failure_kind: "cancelled"`、exit 130）时错误以 `paused: verification_cancelled` 开头；暂停 / 取消请求、进程中断或截止时间在集成开始前或两条检查之间（含 combined）被发现时，错误以 `paused:` / `cancel:` / `budget_exhausted:` 开头。两种情况都把 implement 回执放回 `submitted`、不记 `error`，`prepare --run-id` 对同一候选重新验证；phase 与里程碑门槛也不因此排修复轮。

取舍：失败计数的含义是"候选被判定不合格"；中断只说明判定没完成。把它记为失败会让三次中断耗尽默认 `max_attempts = 3` 并迫使重做已完成的实现。代价是一次被中断的验证必须完整重跑（同树复用仍然生效）。调试构建通过 `<runtime>/failpoints/` 在某条检查后确定性地中断，发布构建编译掉该钩子。

### 5. 活动时间预算

`elapsed_ms` 改为活动时间：协调器运行时间加上 run 处于 `awaiting_host`（宿主持有已签发工作包）的时间；`needs_input`、`paused` 与终态不计。新增可选的 `Run.active_at` 记录协调器上次更新 `elapsed_ms` 的时间，两次之间的间隔只在 `awaiting_host` 时计入。看门狗、`budget_exhausted` 与工作包的 `run_remaining_ms` 用同一基准。

取舍：
- `awaiting_host` 计入，因为此时宿主在消耗资源；`needs_input` 不计，因为没有任何工作在运行。宿主离开一天不应耗尽预算。
- 旧 run 没有 `active_at`：从记录的 `elapsed_ms` 继续累计，此前按墙钟记下的用量保留，不追溯重算。
- `reload_config` 取"配置值"与"run 当前值（含此前所有延长）"的较大者再加 `extend_seconds`，重载不会缩短已授予的期限；要缩短只能新开 run。
- `budget_exhausted` 给出已用活动时间、当前上限、重新打开预算所需的最小 `extend_seconds` 与 `prepare --run-id` 命令；未开 `unsafe_budgets` 时说明 72 小时上限下还能加多少，或已到顶需先设 `unsafe_budgets`。

### 6. `worktree_root` 默认不变

新增 `git`（默认，`.git/spec-autonomous/worktrees/`）、`sibling`（`<repo>.sa-worktrees/`）与绝对路径（其下按仓库建 `<名字>-<哈希>/`）。只有交给宿主的工作检出使用它；集成检出、集成候选、归档试跑和工具检出仍在运行时目录。

默认不改的理由：
- 仓库外的目录可能不可写（容器只挂载仓库、父目录只读、网络盘），而限制"只能写当前项目"的宿主沙箱需要额外放行，否则 worker 写不进自己的 worktree。换默认会让目前能跑的宿主直接失败。
- 仓库内的 `.sa-worktrees/` 不是候选：嵌套 worktree 会被测试发现、打包器、`**` 通配、IDE 与 `git status` 当作项目文件，风险更大，配置不允许根目录位于主检出内部。
- 因此只用 doctor 的 `worktree_root_notice` 说明选项，目录不可用时报 `worktree_root_unavailable`。run 冻结配置，已有 run 不迁移；自动回收、只读检出复用与 `run.cleanup` 同时认两个位置。

### 7. worker 检出准备与 `prepare_read_only` 默认开启

`init_submodules`（默认 `false`）在集成检出、写入型 worker worktree 与非直接集成候选里执行 `git submodule update --init --recursive`；`[hooks.worker_prepare]`（必须 `idempotent`，形状同 `verification_prepare`）在 worktree 根目录、签发工作包之前运行，带 `SA_WORKER_*` 环境。任一失败时不签发该包，attempt 被 superseded 而不计任务失败，run 进入 `needs_input: worker_prepare_failed`。

`prepare_read_only` 默认 `true`，让 `plan-tasks` 与 `audit` 检出同样准备。理由：
- 审计者需要在自己的检出里跑测试才能独立核对；不准备时只能借主检出的 `.venv`，那既不是被审计的树，也绕开了只读检出的边界。
- 钩子本就要求幂等且只产生被忽略文件，失败不计入任务失败，开启的风险有限；代价是只读包签发变慢，钩子很慢或规划用不到环境的项目可关闭。
- 只读检出不能被钩子改坏：运行后出现已跟踪改动即按 `worker_prepare_failed` 处理。钩子留下的被忽略条目记在 `worker-prepare/ignored.json`，检出因此仍可复用（复用时再跑一次钩子），其他被忽略文件照旧阻止复用。
- 设为默认值时从序列化配置中省略，冻结的旧 run 配置不变。

doctor 的 `submodules_warning` 看 worker 侧：仓库有子模块、`init_submodules = false` 且没有 `worker_prepare` 时提示（只配 `verification_prepare` 不够）。开启子模块后，`git worktree remove` 会拒绝任何初始化过子模块的检出；清理在变更时复核父仓库与每个子模块都干净后再用一次 `--force`（仍拒绝被锁定的工作树），不用会改写共享配置的 `git submodule deinit`。

### 8. 交付忽略不冲突的未跟踪文件

`ff-original` 交付（及 `archive` 最后的快进）原先在主检出有任何未跟踪文件时拒绝快进。现在未跟踪文件只在与交付 diff 冲突时阻挡：路径相同、位于交付改动的路径之下、或交付需要在其位置建目录（不区分大小写比较）。已跟踪改动（含已暂存）、子模块改动与冲突的未跟踪文件仍进入 `delivery_pending` 并保留 verified 分支，blocker 以 `blocking: {...}` 列出路径，`ignored_untracked` 给出未阻挡的数量；操作者已手工快进时，续跑在只剩未跟踪文件的情况下直接确认交付。

取舍：快进只改写 diff 涉及的路径，不冲突的未跟踪文件不会被覆盖，挡住交付没有保护任何东西。冲突判断宁严勿宽（目录前缀、大小写不敏感）。run 开始前的 `dirty_checkout` 规则不变：未跟踪文件仍阻止开始，因为那时还不知道会改哪些路径。

### 9. 账本 schema 5

0.2.0 写入的 run 可能带 alpha.16 处理不了的字段：冻结配置里的 `init_submodules` / `baseline_checks`（`Execution` 拒绝未知字段，旧 CLI 读不出 run）、`task_writes` 修订（旧 `reconcile_plan` 忽略它，重新规划时丢掉已授予的写集）、`Run.active_at` 与 `HostState.baselines`（旧写入者会丢掉）。沿用 alpha.9 的先例提升 schema 版本而不是依赖 serde 兼容：schema 5 与 4 表结构相同，迁移只备份并改 `user_version`，不重写任何行（重新拆分不含 attempts 的核心载荷会删光所有行）。只读打开和 `run.pause` / `run.cancel` 不迁移，以便旧服务仍在运行时能安全查看与停止。

### 10. 工作包与提示

- `prompt.md` 只放规则、`input.json` 绝对路径与 sha256、按包类型的摘要；`work_packet::trim` 按类型裁剪 `input.json`（只有 audit 保留 `metadata.acceptance`，implement 去掉 `metadata.documents` 并把 OpenSpec apply JSON 换成指令文本）。`input_hash` 仍是写出的 `input.json` 的哈希，请求键用未裁剪的快照，升级前签发的包不变。
- 规划指令中新增的说明（检查 `env`、OpenSpec 并行规则、glob 写集、`verify_only`、`reads`）都附在请求键之外。
- 原生任务子条目进入 `SourceTask.details`，`description` 仍只取复选框一行以便与 OpenSpec 原生输出比对。

### 11. `baseline_checks` 默认关闭

计划被接受后、签发实现前在 `accepted_head` 上把已能运行的检查跑一次，失败记为 `baseline_failure:` 警告并写进该 phase 的 implement / converge 包。它们不阻塞、不计失败、不排修复。默认关闭，因为每个 phase 计划要多跑一遍这些检查（往往包括全量测试）；预检 `deferred` 或 argv 指向将被写入的文件的检查跳过，结束后集成检出恢复到 `accepted_head`。

## Risks / Trade-offs

- 按范围沿用依赖计划声明的 `writes` / `reads` → 声明错误时可能漏跑。缓解：未声明即重跑、全局输入与未知 owner 即重跑、里程碑门槛默认全量执行一次。
- 审计沿用依赖审计者给出的 `files` → 漏列文件会让结论被错误沿用。缓解：需求原文、原生文档、全局输入与未声明文件的改动都使沿用失效；不给 `files` 即每次重审。
- 自动写集授权扩大了 worker 能改的范围 → 字面路径、10 条上限、原生上下文与其父目录禁止、冲突拒绝，超出部分走人工 `run.revise`。
- 活动时间预算让长时间等待宿主的 run 不再超时 → `awaiting_host` 仍计时；`needs_input` 本就需要人处理。
- `prepare_read_only` 默认开启让只读包签发变慢 → 可关闭；钩子失败不计任务失败。
- schema 5 让旧 CLI 无法打开升级后的账本 → 首次写入前自动备份，文档给出回退步骤。

## Migration Plan

升级 CLI 后重启项目的 MCP 服务。首次可写打开自动备份并迁移账本；已有 run 保持冻结配置继续运行（worktree 位置不变、旧预算用量保留、无配置哈希时不报漂移）。新配置项全部可选，默认值保持既有行为，除 `verification_scope = "affected"` 与 `prepare_read_only = true` 外不改变已有项目的执行方式；需要旧行为时分别设为 `full` / `false`。回退：停掉新服务，把 `state.v4.<uuid>.backup.db` 恢复为 `state.db`。
