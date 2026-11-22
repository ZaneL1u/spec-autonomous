# 配置

项目配置位于 `.spec-autonomous/config.toml`。初始化会创建安全默认值；不认识的字段或拼错的关键字段会报错，不会静默变成无限预算。

## 最小示例

```toml
schema_version = 2
verification = [
  { argv = ["cargo", "test", "--workspace", "--locked"], cwd = "." }
]

[execution]
max_workers = 3
max_attempts = 3
attempt_timeout_seconds = 1800
run_timeout_seconds = 28800
max_repair_rounds = 2

[host]
max_concurrency = 3
lease_seconds = 1800
```

## `execution`

| 配置 | 含义 |
| --- | --- |
| `max_workers` | CLI 允许同时处于执行中的任务上限 |
| `max_attempts` | 每项工作最多尝试次数 |
| `attempt_timeout_seconds` | 单个工作包的时间预算 |
| `run_timeout_seconds` | 整个 run 的时间预算 |
| `max_repair_rounds` | 验证失败后允许的有界修复轮数 |
| `max_context_bytes` | 内联工作包上下文上限，超出时改用 hash 引用 |
| `max_source_bytes` | 原生来源快照预算 |
| `max_planner_tasks` | planner 单批原生任务上限 |
| `verification_reuse` | `same-tree`（默认）在同一源码树、同命令、同环境、同检出类别上复用已通过的验证证据；`none` 每个波次重新执行 |
| `verification_scope` | `affected`（默认）：combined 与 phase 门槛里由其他任务声明的检查，在上次通过后的改动没有触及这些任务的 `writes` / `reads`（也没有全局输入、原生文档或未声明文件的改动）时沿用上次的通过证据；配置的项目 / 阶段检查与当前任务自己的检查总是执行。`full`：每道门槛执行全部检查 |
| `milestone_verification_scope` | 里程碑门槛的同一选择，默认 `full`：在交付前的最终树上把被沿用过的任务检查实际执行一遍；`affected` 按上述规则沿用 |
| `unsafe_budgets` | 默认 `false`：`max_attempts ≤ 10`、`max_repair_rounds ≤ 4`、`run_timeout_seconds ≤ 259200`（72 小时）；设为 `true` 才允许超出，doctor 会告警 |
| `reclaim_worktrees` | `terminal`（默认）在 attempt 终态且补丁落盘后自动删除其 worktree；`never` 全部保留给手动 run.cleanup |
| `retain_failed_worktrees` | 每个未完成任务保留的最新失败 worktree 数，默认 1 |
| `delivery` | `ff-original` 或保留交付分支。`ff-original` 在全部门槛通过后把用户检出快进到 `accepted_head`；与交付 diff 不冲突的未跟踪文件（笔记、草稿）不阻挡，已跟踪改动或冲突的未跟踪文件进入 `delivery_pending`，blocker 的 `blocking:` 列出具体路径 |
| `init_submodules` | 默认 `false`。为 `true` 时，协调器创建的集成检出、写入型 worker worktree 和集成候选 worktree 建好后都执行 `git submodule update --init --recursive`（环境取 `[environment]`）；`prepare_read_only` 开启时只读的规划 / 审计检出同样执行。worker 上失败则不签发该工作，run 进入 `needs_input` |
| `prepare_read_only` | 默认 `true`：只读的规划（`plan-tasks`）与审计（`audit`）检出也像写入型检出一样初始化子模块并运行 `[hooks.worker_prepare]`，审计 worker 才能在自己的检出里跑测试。默认开启的理由：钩子本就要求幂等、只产生被忽略的文件，失败不计入任务失败；关掉它审计者只能借主检出的环境。钩子很慢或规划者用不到环境时可设为 `false`，恢复为只准备写入型检出 |
| `baseline_checks` | 默认 `false`。为 `true` 时，phase 计划被接受后、签发该 phase 的实现工作前，协调器在集成检出（`accepted_head`）上把计划里已经能跑的检查跑一次（预检为 `deferred`、或 argv 指向任务将写入且尚不存在的文件的检查跳过；配置了 `verification_prepare` 时先运行它），失败的检查记为已知基线失败：响应 `warnings[]` 给出 `baseline_failure: phase <id>: ...`（含命令、退出码与脱敏的输出末尾），该 phase 的 implement / converge 包在 `snapshot.metadata.baseline_failures` 中带上同一列表。基线失败不阻塞、不计失败、不排修复轮。默认关闭是因为每个 phase 计划要多跑一遍这些检查（包括全量测试）；全量检查昂贵而基线可能已坏的仓库建议开启。基线检查留下的文件会被清除，集成检出回到 `accepted_head` |

| `worktree_root` | 交给宿主 worker 的 worktree 放在哪里：`git`（默认，`.git/spec-autonomous/worktrees/`）、`sibling`（主检出旁边的 `<仓库目录名>.sa-worktrees/`）或一个绝对路径（其下再建 `<仓库目录名>-<哈希>/`，多个仓库可共用） |

实际并发取 `execution.max_workers` 与 `host.max_concurrency` 的较小值，还会受 DAG 和写集冲突限制。

### worktree 位置

默认的 worker worktree 位于 `.git/spec-autonomous/worktrees/` 之下。很多搜索包装、IDE 索引和「跳过 `.git/`」的工具在这些目录里什么也找不到，worker 会误以为代码不存在。doctor 会给出 `worktree_root_notice` 提示。需要时改成：

```toml
[execution]
worktree_root = "sibling"   # 例如 ~/src/app 的 worker 在 ~/src/app.sa-worktrees/<名字>/
```

默认值暂不改变，原因：

- 仓库外的目录可能不可写（容器只挂载了仓库目录、父目录只读、网络盘），而且限制「只能写当前项目」的宿主沙箱（例如 Claude Code 的沙箱）需要额外放行该目录，否则 worker 写不进自己的 worktree。
- 仓库内的 `.sa-worktrees/` 被排除在候选之外：嵌套 worktree 会被测试发现、打包器、`tsc`/`eslint` 的 `**` 通配、IDE 与 `git status` 当作项目文件，风险比现在更大，配置不允许把根目录放在主检出内部（`worktree_root_unavailable`）。

行为说明：

- 只有交给宿主的工作检出（规划、审计、实现、converge 等）使用该位置；集成检出、集成候选、归档试跑和 `worktree.create` 的工具检出仍在 `.git/spec-autonomous/worktrees/`。
- run 启动时冻结配置：已有 run 继续用原位置，不迁移；改配置后新开的 run（或带 `reload_config` 续跑后新建的 worktree）才使用新位置，旧 worktree 照常被识别与回收。
- 目录建不出来时，签发该工作的 `prepare` / `apply-result` 报 `worktree_root_unavailable` 并给出路径；doctor 对不可写、不是目录的位置提前给出同名诊断。
- 自动回收（`reclaim_worktrees = "terminal"`）、只读检出复用与 `run.cleanup` 的归属判断同时认两个位置。

run 启动时冻结配置。`run_timeout_seconds` 按活动时间计：协调器运行的时间，加上 run 处于 `awaiting_host`（宿主持有已签发的工作包）的时间；停在 `needs_input`、`paused` 或终态期间不计，宿主离开一天后回来不会因此耗尽预算。账本里 0.2.0 之前写入的 run 没有活动时间锚点，从记录的 `elapsed_ms` 继续累计，此前按墙钟记下的用量保留。`prepare --run-id` 带 `extend_seconds` 在 run 当前上限上累加；带 `reload_config` 重读项目配置时，`run_timeout_seconds` 取「配置文件值」与「run 当前值（含此前所有延长）」的较大者再加本次 `extend_seconds`，因此重载不会缩短已延长的期限，要缩短只能新开 run。预算耗尽时 blocker 形如 `budget_exhausted: run deadline reached: <已用>s elapsed (active time: ...) of run_timeout_seconds=<上限>; ... --extend-seconds <n> where n (extend_seconds) is at least <最小值> plus ...`；未开 `unsafe_budgets` 时还会说明延长后总上限不能超过 72 小时、本次最多还能加多少，或者已到上限、需要先在项目配置里设 `unsafe_budgets = true` 并 `reload_config`。

## 分阶段验证命令

| 配置 | 运行阶段 |
| --- | --- |
| `verification` | 每个阶段：worker、combined、phase、post-hook、milestone |
| `task_verification` | 只在任务候选验证时（worker、combined） |
| `phase_verification` | 只在 phase 门槛（phase、post-hook、stale 复验） |
| `milestone_verification` | 只在里程碑门槛 |

phase 门槛没有任何 phase 级检查时退回使用计划里各任务的检查；修复轮或单任务被接受后，`verification_scope = "affected"` 只重跑改动触及的任务的检查（任务需要声明 `reads`，`reads` 为空的任务总是重跑），其余沿用证据并在 `reused_from` 中注明来源与变更文件数。

把快速的单元测试放进 `task_verification`，把 `make race`、全量集成这类昂贵套件放进 `phase_verification` 或 `milestone_verification`，每个任务就不再重复跑整套。规划器拿到的「项目级检查」是 `verification` 与 `task_verification` 之和。

检查的 argv 不能内嵌脚本：单个元素超过 1024 字节，或含 base64 / b85decode / zlib.decompress 这类编码载荷，会被 `invalid_command: inline_script_argv` 拒绝。需要脚本时，把它写成任务 `writes` 里声明的文件再执行；preflight 会把尚不存在但已声明的脚本和可执行文件视为 deferred 而不是缺失。

## 检出准备钩子

`[hooks]` 下两个可选钩子都要求 `idempotent = true`，argv 不经 shell：

| 钩子 | 时机 | cwd | 环境变量 |
| --- | --- | --- | --- |
| `verification_prepare` | 每个验证波次第一条实际执行的检查之前 | 被验证的项目目录 | `SA_VERIFY_*`（见长期运行文档） |
| `worker_prepare` | worker worktree 创建（或只读检出被复用）并初始化子模块之后、签发工作包之前。写入型 attempt（implement / converge 等）总是运行；只读的规划（`plan-tasks`）/ 审计（`audit`）包在 `[execution] prepare_read_only = true`（默认）时运行，用 `SA_WORKER_KIND` 区分 | worktree 根目录 | `SA_WORKER_CHECKOUT`、`SA_WORKER_PROJECT`、`SA_WORKER_ATTEMPT_ID`、`SA_WORKER_KIND`、`SA_WORKER_TASK_ID`、`SA_WORKER_PHASE_ID`、`SA_WORKER_BASE_COMMIT`、`SA_ORIGIN_CHECKOUT`、`SA_RUN_ID`，以及 `[environment]` |

`worker_prepare` 只应创建被忽略的文件（`.venv`、`node_modules`、缓存）。退出码非零、超时（`attempt_timeout_seconds`）、无法启动，或运行后检出出现已跟踪文件改动（blocker 写明 `left tracked changes` 与改动路径）时，该 attempt 记为 `superseded`（不计入任务失败次数），run 进入 `needs_input`，blocker 以 `worker_prepare_failed` 开头并给出 stderr 末尾与日志目录（attempt 目录下的 `worker-prepare/`、`submodules/`）。修好环境或配置后 `prepare --run-id <id>`（改过配置时加 `reload_config`）。

让 worker 环境与验证环境一致：两个钩子可以指向同一个脚本。依赖安装请走共享的包管理缓存，而不要把主 checkout 的 `.venv` / `node_modules` 软链进 worktree —— editable 安装和工作区链接会指回主 checkout 的旧源码，worker 测到的就不是自己的改动。钩子产物应在 `.gitignore` 中，否则会被当作 worker 改动。只读检出被后续规划 / 审计包复用时钩子会再跑一次；它留下的被忽略条目记在 attempt 目录的 `worker-prepare/ignored.json`，复用只容忍这些条目，交接点的自动维护也把它们视为可重建的产物，不会因此保留已关闭的只读检出。

```toml
# Python（uv）：每个 worktree 自建 .venv，包从全局缓存硬链接，通常几秒完成
[hooks.worker_prepare]
argv = ["uv", "sync", "--frozen"]
idempotent = true

[hooks.verification_prepare]
argv = ["uv", "sync", "--frozen"]
idempotent = true
```

```toml
# Node（pnpm）：内容寻址 store 共享，离线优先
[hooks.worker_prepare]
argv = ["pnpm", "install", "--frozen-lockfile", "--prefer-offline"]
idempotent = true
```

需要多步时写成提交在仓库里的脚本（例如 `scripts/worker-prepare.sh`），并在 `[environment]` 里固定 `UV_CACHE_DIR` / `PNPM_STORE_DIR` 等缓存位置。doctor 会对两类遗漏给出警告：`.spec-autonomous/` 下存在 `verification-prepare.*` / `worker-prepare.*` 脚本却没有对应 `[hooks.*]`（`hook_unbound_warning`）；仓库声明了子模块，但 `init_submodules = false` 且没有 `worker_prepare`（`submodules_warning`；只配了 `verification_prepare` 也会提示，因为它只准备验证检出，worker worktree 仍然没有子模块内容）。

## `host`

`max_concurrency` 表示宿主确认能同时管理多少个 fresh session。没有声明时保守串行。 `lease_seconds` 是心跳租约（现在只用于 `stale` 信息）。`[host.capabilities]` 可声明宿主能力（如 `isolation = "worktree"`、`background_dispatch = "true"`），随 `host_capacity` 返回给宿主。

`lease_seconds` 用于判断工作包心跳是否过期。过期只代表状态未知，不代表 Agent 已停止，也不会自动转让所有权。

## 验证

验证命令必须使用 argv 数组：

```toml
verification = [
  { argv = ["bun", "run", "test"], cwd = "." },
  { argv = ["cargo", "clippy", "--workspace", "--locked"], cwd = "." }
]
```

不要把命令写成需要 shell 解析的一整段字符串。这样空格、引号和用户输入不会改变参数边界。

### 检查级环境变量

每条检查（项目配置、phase / milestone、任务计划里的 `verification`，以及 `run.revise` 的检查）可以带可选的 `env`，只对这一条检查生效，叠加在项目环境之上：

```toml
verification = [
  { argv = ["python3", "-m", "pytest", "tests"], cwd = "server", env = { PYTHONDONTWRITEBYTECODE = "1" } }
]
```

- 键名必须匹配 `[A-Za-z_][A-Za-z0-9_]*`，每条检查最多 32 个，值不超过 1024 字节且不含 NUL。
- 不能覆盖 `PATH`、`PATHEXT`、`HOME`、`USER`、`SHELL`、`PWD`、`TMPDIR` 等定位可执行文件、用户与 shell 的变量，也不能设置 `SA_*`（协调器自己的变量）、`GIT_*`、`LD_*`、`DYLD_*`；违反时报 `invalid_command: check_env: ...`。
- `env` 会写进计划、工作包与账本，所以名字像凭证的键（含 `TOKEN`、`SECRET`、`PASSWORD`、`API_KEY`、`AUTH`、`COOKIE` 等）一律拒绝。密钥请放在 `[environment]` 或宿主环境里。
- 值参与验证复用的身份哈希：同一 argv 换了 `env` 就是另一条检查，不会复用旧证据。验证证据的 `result.json` 只记录 `env_keys`（变量名），不记录值。
- 不带 `env` 的检查序列化结果与旧版本完全相同，已有计划、账本和复用身份不受影响。旧版 CLI 不认识 `env` 字段，会拒绝带 `env` 的计划。

phase 和 milestone 还可以声明自己的 verification。任务计划也可有更小范围的检查。

## 环境变量

项目级 provider 和验证环境可以单独配置。敏感值不要提交到仓库；使用 CI secret 或宿主运行环境注入。

结构化输出会把环境内容标记为 redacted，不回显密钥。

## 旧 `runner`

历史 alpha.1 配置可能包含：

```toml
[runner]
profile = "command"
command = ["some-agent"]
```

当前版本不会执行它。Doctor 会报告旧字段，历史账本保持只读兼容，但不会重新激活 CLI agent launcher。

## 配置层级

有效配置按默认值、用户配置、项目配置合并。数组按更具体层完整替换，不做难以预测的拼接。

继续已有 run 时默认使用创建时保存的有效配置。只有明确使用 reload 选项，才会把新配置带入允许变化的续跑边界。
