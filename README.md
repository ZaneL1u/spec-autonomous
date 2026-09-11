# Spec Autonomous

**在用户现有的 OpenSpec / Spec Kit 上，从里程碑目标规划 roadmap，再自主实现、验证、修复和交付。**

`0.1.0-alpha.1` 已实现本地编排 CLI、TOML roadmap、原生 Markdown 适配、阶段范围、独立 worktree workers、验证与恢复、全 worktree progress，以及 `autonomous` / `auto` 等 skill。npm registry 尚未发布；当前可从源码或本机 tarball 安装。

- [架构与实际协议](docs/architecture.md)
- [测试、mock 仓库与故障注入](docs/testing.md)
- [本轮验收记录](docs/validation/autonomous.md)
- [OpenSpec 实施清单](openspec/changes/autonomous-orchestration/tasks.md)
- [源码调研](docs/research/README.md) · [发行流程](docs/distribution.md)

## 本地安装

开发环境：Git、Node.js 22+、Bun 1.4.2、Rust 1.98.1（由 rust-toolchain.toml 固定）。安装好的 native npm 包不需要 Rust/Bun。

```sh
. "$HOME/.cargo/env"
bun install --frozen-lockfile
bun run build
bun run pack:local
npm install -g ./.artifacts/local/spec-autonomous-0.1.0-alpha.1.tgz
spec-autonomous --help
```

本机 tarball 只适合生成它的平台。正式 registry 发布后，入口为 `npm install -g spec-autonomous@next`；这条 registry 命令目前不代表已经发布。

进入已配置 OpenSpec 或 Spec Kit 的仓库，绑定宿主入口：

```sh
spec-autonomous detect --json
spec-autonomous init --agent codex
# Claude Code 的 slash command 绑定：
spec-autonomous init --agent claude
```

Codex 使用 `$autonomous` / `$auto`，支持 slash commands 的宿主使用 `/autonomous` / `/auto`。`auto` 是同义别名；另有 milestone、progress、resume。安装会保留用户手写或修改过的同名入口；冲突时可显式使用 `--prefix sa`。npm 全局安装不会猜测并修改当前仓库。

## 配置已有 agent 与验证命令

编辑 `.spec-autonomous/config.toml`。配置优先级为 CLI 覆盖 > 项目 TOML > `$XDG_CONFIG_HOME/spec-autonomous/config.toml`（默认 `~/.config/spec-autonomous/config.toml`）> 默认值。未知配置键会报错，避免把拼错的预算字段静默忽略。

```toml
schema_version = 1
# 按项目实际情况填写；每个阶段/任务也能声明自己的验证命令。
verification = [{ argv = ["node", "--test", "tests/add.test.mjs"], cwd = "." }]

[execution]
mode = "autonomous"
max_workers = 3
max_attempts = 3
attempt_timeout_seconds = 1800
run_timeout_seconds = 28800
max_repair_rounds = 2
max_context_bytes = 131072
max_source_bytes = 2097152
max_planner_tasks = 32
delivery = "ff-original"

[runner]
profile = "codex"
command = ["codex"]
```

Codex profile 使用本机已登录的 `codex exec --ephemeral`，按工作单元使用 read-only/workspace-write，并要求结构化结果。没有携带父会话或 resume ID。其他 agent 可通过 `profile = "command"` 和 argv 数组接入，但 launcher 必须履行 `fresh_session = true` 的契约，详见架构文档；这不是对所有 agent CLI 的自动兼容承诺。

OpenSpec 执行需要其已安装 CLI。可用 `[provider].openspec_command` 配置明确 argv，例如 `['node', '/absolute/path/to/openspec/bin/openspec.js']`。Spec Kit 读取现有文档无需 Python；从头规划会使用目标仓库安装的原生 skill/template，相关脚本需要其原有运行依赖。

```sh
spec-autonomous doctor --json
# 自主写入前需要有初始 commit，且起始 checkout 干净。
git add .
git commit -m "Configure autonomous orchestration"
```

## 规划与自主执行

```sh
# 生成完整 roadmap 后交给原生流程继续
spec-autonomous milestone new "团队邀请与权限 MVP" --mode native

# 从目标自动规划并继续开发
spec-autonomous milestone new "团队邀请与权限 MVP" --mode autonomous
spec-autonomous roadmap --milestone M001 --format toml

# 执行完整里程碑或阶段范围
spec-autonomous run --milestone M001 --mode autonomous
spec-autonomous run --milestone M001 --from 2 --to 4 --mode autonomous
spec-autonomous run --milestone M001 --only 3 --mode autonomous

# 直接接续已存在的原生规划
spec-autonomous run --framework openspec --change add-team-auth --mode autonomous
spec-autonomous run --framework speckit --feature specs/001-auth --mode autonomous
```

实际 milestone ID 会在创建结果中返回，也可通过 `milestone new ... --id M001` 指定。from/to 指 roadmap 阶段闭区间，only 只跑一个阶段。范围外未完成依赖会阻止执行；`scope_completed` 不等于整个 milestone 完成，也不会自动归档。

每个 roadmap phase 指向一个原生 change/feature。规范、设计和原生任务仍是 Markdown；TOML 保存编排拓扑和执行计划，SQLite 保存运行事务。缺工件时沿原生流程规划；每个 worker 使用新会话和独立 worktree；coordinator 验证实际代码、串行集成，再写回 checkbox。失败修复有次数、时间、退避与跨恢复的无进展上限。大任务列表分批规划并做完整 DAG 校验；超出内联预算的原生上下文以不可变文件和 hash 引用传递，保留完整规则。

## 查看、暂停与恢复

```sh
spec-autonomous progress --all-worktrees
spec-autonomous progress --all-worktrees --format json
spec-autonomous progress --all-worktrees --format toml
spec-autonomous inspect --milestone M001 --phase 1 --json
spec-autonomous status <run-id> --json
spec-autonomous report <run-id> --json
spec-autonomous pause <run-id>
spec-autonomous cancel <run-id>
# 终态运行的安全清理，保留 dirty/external/integration worktree、分支和证据
spec-autonomous cleanup <run-id>
spec-autonomous resume <run-id>
spec-autonomous resume <run-id> --mode native
# 显式采用修改后的配置或增加预算
spec-autonomous resume <run-id> --reload-config --extend-seconds 600
```

progress 从 Git common directory 汇总主 checkout、所有受管 worker/candidate/integration worktree 和外部 worktree。原生任务计数标明 recorded/observed_at，验证状态与之分开；不可确认的状态显示 unknown/stale/partial。它不会启动 agent、修复工作区或执行仓库脚本。

默认最终 fast-forward 原 checkout；原分支或文件发生变化时保留已验证分支并报告 delivery_pending。运行中 source drift 可在用户提交原生修改后 resume，由隔离合并与重新验证接续；冲突不会被 force/reset 覆盖。未知 hook 副作用需要明确确认：

```sh
spec-autonomous resolve-hook <run-id> --key '<phase>/<event>/<command>' \
  --outcome completed --evidence '已检查外部结果和本地日志'
spec-autonomous resume <run-id>
```

## 本地 mock 与完整测试

```sh
bun run mock:create --framework openspec --output .artifacts/demo-openspec --fail-once add
node packages/cli/bin/spec-autonomous.mjs run --path .artifacts/demo-openspec --milestone M001 --json

bun run test:all
# 等价入口（无需 Bun shell）：
node scripts/test-all.mjs
```

mock 只替代 agent 决策：实际 CLI、OpenSpec、Git worktrees、SQLite、进程、Node 断言、集成和恢复都在真实临时仓库运行。测试套件默认不调用付费模型或 npm publish；真实 Codex runner 验收单独记录在验收文档中。

支持边界：一个写 coordinator 管理同仓多个并行 workers；外部 worktree 只观察。当前支持 repo-local OpenSpec、单个可确定 tracking file、Spec Kit 文档与原生核心规划步骤，未知必需 hook/条件/外部 store 会明确阻塞。worktree 不是 OS 沙箱，command runner 的执行权限由该 launcher 提供。自动远程 push/publish/deploy 与原生归档不在本地执行 profile 中；使用独立发布流程或原生工具。

本机已验证 macOS arm64。其他平台的构建和测试 workflow 已配置，未经实际运行的平台不记作通过。项目继续通过 OpenSpec 清单维护剩余真实发布与平台验收工作。
