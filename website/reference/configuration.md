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
| `unsafe_budgets` | 默认 `false`：`max_attempts ≤ 10`、`max_repair_rounds ≤ 4`、`run_timeout_seconds ≤ 259200`（72 小时）；设为 `true` 才允许超出，doctor 会告警 |
| `reclaim_worktrees` | `terminal`（默认）在 attempt 终态且补丁落盘后自动删除其 worktree；`never` 全部保留给手动 run.cleanup |
| `retain_failed_worktrees` | 每个未完成任务保留的最新失败 worktree 数，默认 1 |
| `delivery` | `ff-original` 或保留交付分支 |
| `init_submodules` | 默认 `false`。为 `true` 时，协调器创建的集成检出、写入型 worker worktree 和集成候选 worktree 建好后都执行 `git submodule update --init --recursive`（环境取 `[environment]`）；worker 上失败则不签发该工作，run 进入 `needs_input` |

实际并发取 `execution.max_workers` 与 `host.max_concurrency` 的较小值，还会受 DAG 和写集冲突限制。

run 启动时冻结配置。`run_timeout_seconds` 按自 run 开始的墙钟时间计（包括等待宿主的时间）。`prepare --run-id` 带 `extend_seconds` 在 run 当前上限上累加；带 `reload_config` 重读项目配置时，`run_timeout_seconds` 取「配置文件值」与「run 当前值（含此前所有延长）」的较大者再加本次 `extend_seconds`，因此重载不会缩短已延长的期限，要缩短只能新开 run。预算耗尽时 blocker 形如 `budget_exhausted: run deadline reached: <已用>s elapsed ... of run_timeout_seconds=<上限>; ... --extend-seconds <n> where n (extend_seconds) is at least <最小值> plus ...`。

## 分阶段验证命令

| 配置 | 运行阶段 |
| --- | --- |
| `verification` | 每个阶段：worker、combined、phase、post-hook、milestone |
| `task_verification` | 只在任务候选验证时（worker、combined） |
| `phase_verification` | 只在 phase 门槛（phase、post-hook、stale 复验） |
| `milestone_verification` | 只在里程碑门槛 |

把快速的单元测试放进 `task_verification`，把 `make race`、全量集成这类昂贵套件放进 `phase_verification` 或 `milestone_verification`，每个任务就不再重复跑整套。规划器拿到的「项目级检查」是 `verification` 与 `task_verification` 之和。

检查的 argv 不能内嵌脚本：单个元素超过 1024 字节，或含 base64 / b85decode / zlib.decompress 这类编码载荷，会被 `invalid_command: inline_script_argv` 拒绝。需要脚本时，把它写成任务 `writes` 里声明的文件再执行；preflight 会把尚不存在但已声明的脚本和可执行文件视为 deferred 而不是缺失。

## 检出准备钩子

`[hooks]` 下两个可选钩子都要求 `idempotent = true`，argv 不经 shell：

| 钩子 | 时机 | cwd | 环境变量 |
| --- | --- | --- | --- |
| `verification_prepare` | 每个验证波次第一条实际执行的检查之前 | 被验证的项目目录 | `SA_VERIFY_*`（见长期运行文档） |
| `worker_prepare` | 写入型 attempt（implement / converge 等）的 worktree 创建并初始化子模块之后、签发工作包之前；只读的规划 / 审计包不运行 | worktree 根目录 | `SA_WORKER_CHECKOUT`、`SA_WORKER_PROJECT`、`SA_WORKER_ATTEMPT_ID`、`SA_WORKER_KIND`、`SA_WORKER_TASK_ID`、`SA_WORKER_PHASE_ID`、`SA_WORKER_BASE_COMMIT`、`SA_ORIGIN_CHECKOUT`、`SA_RUN_ID`，以及 `[environment]` |

`worker_prepare` 退出码非零、超时（`attempt_timeout_seconds`）或无法启动时，该 attempt 记为 `superseded`（不计入任务失败次数），run 进入 `needs_input`，blocker 以 `worker_prepare_failed` 开头并给出 stderr 末尾与日志目录（attempt 目录下的 `worker-prepare/`、`submodules/`）。修好环境或配置后 `prepare --run-id <id>`（改过配置时加 `reload_config`）。

让 worker 环境与验证环境一致：两个钩子可以指向同一个脚本。依赖安装请走共享的包管理缓存，而不要把主 checkout 的 `.venv` / `node_modules` 软链进 worktree —— editable 安装和工作区链接会指回主 checkout 的旧源码，worker 测到的就不是自己的改动。钩子产物应在 `.gitignore` 中，否则会被当作 worker 改动。

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
