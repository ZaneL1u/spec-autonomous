# Spec Autonomous

**基于 OpenSpec / Spec Kit，规划、跟踪和交付里程碑。**

用户继续使用 OpenSpec / Spec Kit 的原生规范与工作流。CLI 封装结构化读取、上下文准备、就绪判断、worktree、验证、集成、回写和归档；宿主创建 fresh-context agent 并返回结果。

当前版本 `0.1.0-alpha.9`，通过私有 GitHub 仓库和 Release 分发，npm registry 尚未发布。Rust CLI 使用 Clap，JS 入口使用 Commander.js，并根据用户 locale 提供 English / 简体中文界面。

- [架构与工作协议](docs/architecture.md)
- [全部能力与 CLI/MCP 接口](docs/capabilities.md)
- [测试和独立 mock 宿主](docs/testing.md)
- [本轮验收](docs/validation/host-driven.md)
- [从 alpha.1 迁移](docs/migration-alpha2.md)
- [npm 分平台发行](docs/distribution.md)
- [修订验证并续跑](docs/run-revision.md)
- [运行结束后的资源清理](docs/run-cleanup.md)
- [交互式与参数式初始化](docs/initialization.md)
- [自动安装 OpenSpec / Spec Kit](docs/provider-bootstrap.md)
- [CLI 框架与命令接口](docs/cli-interface.md)
- [私有 GitHub 直装](docs/private-git-install.md)
- [CLI 国际化](docs/localization.md)

## 安装和绑定

另一台已登录同一 GitHub 账号的 gh、且有仓库 SSH 访问权限的 Mac，可以直接安装：

```sh
npm install -g git+ssh://git@github.com/ZaneL1u/spec-autonomous.git
spec-autonomous --version
```

Git 入口没有安装脚本。第一次运行自动用 gh 下载并校验该版本的本机二进制，之后复用缓存；不需要 Rust 或 Bun。目前 Git 直装预编译产物为 macOS arm64。

源码开发需要 Git、Rust 1.98.1、Node 22.13+（或 23.5+）；Bun 管理 workspace 依赖。安装后的 npm 包需要 Node 22.13+（或 23.5+） 和 Git。JS 层自动补齐缺失的原生 SDD 工具及 Spec Kit 的 uv / Python，无需预装 Rust、Bun、Python。

```sh
. "$HOME/.cargo/env"
bun install --frozen-lockfile
node scripts/pack-local.mjs
npm install -g ./.artifacts/local/spec-autonomous-0.1.0-alpha.9.tgz

# 已有原生规范的项目：自动补齐缺失工具并绑定 Skills
spec-autonomous init --agent codex
# 新项目：交互选择框架、编程助手和 MCP
spec-autonomous init
# 参数式：调用原生初始化，生成项目结构并绑定 Skills
spec-autonomous init --provider openspec --agent codex --mcp
spec-autonomous init --provider speckit --agent codex --mcp
# 同时写入受所有权保护的项目 MCP 配置
spec-autonomous init --agent codex --mcp
# Claude Code 对应入口
spec-autonomous init --agent claude --mcp

# 安装状态、显式补齐、执行原生命令
spec-autonomous providers status --json
spec-autonomous providers ensure speckit --json
spec-autonomous providers exec openspec -- --version

# 社区框架生成的统一帮助
spec-autonomous --help
spec-autonomous providers ensure --help
spec-autonomous help providers ensure
```

MCP 配置保留其他服务器与用户设置；宿主原有的项目信任规则仍然适用。Codex 使用 `$autonomous` / `$auto`，Claude 使用 `/autonomous` / `/auto`。另外提供 milestone、progress、resume。冲突时可用 `--prefix sa`，不会覆盖用户修改的文件。

本机 tgz 只包含本机架构。正式发布使用一个 launcher/skills 包与六个精确版本的平台包，npm 按 os/cpu/libc 选择二进制；发布后用户只需 `npm install -g spec-autonomous`。当前不把这条 registry 命令描述成已发布可用。

## 常用完整能力

工作流核心提供七项完整能力；MCP 对应 `sa_inspect`、`sa_progress`、`sa_prepare`、`sa_next`、`sa_apply_result`、`sa_archive`、`sa_doctor`，以及一个高级目录/调用入口 `sa_tools`。npm JS 层另提供 `sa_providers` 安装管理工具。

```sh
spec-autonomous inspect --json
spec-autonomous progress --all-worktrees --json
spec-autonomous doctor --json

# 已有原生工作
spec-autonomous prepare --change add-auth --json
spec-autonomous prepare --feature specs/001-auth --json
# 从目标开始；只生成工作包，语义工作由宿主完成
spec-autonomous prepare --goal "团队邀请 MVP" --id M001 --json
# 已有里程碑的闭区间
spec-autonomous prepare --milestone M001 --from 2 --to 4 --json
spec-autonomous next --run-id <run-id> --json
spec-autonomous prepare --run-id <run-id> --json
```

`prepare` 返回 `awaiting_host` 不表示任务完成。它包含不可变输入/结果 schema 的引用、唯一 request ID、领取凭据、已分配 worktree 和限制。重复 prepare 复用待处理请求；不会重复创建 agent 或工作区。

宿主通过自己的能力创建新上下文，领取请求，在指定 worktree 中完成语义工作，再提交结构化结果：

```sh
spec-autonomous claim <run-id> <request-id> --token <token> \
  --host-id <host> --session-id <unique-session> --fresh-context --json

spec-autonomous apply-result --result <host-result.json> --token <token> \
  --host-id <host> --session-id <unique-session> --fresh-context --json
```

MCP 使用同样的结构化参数，无需生成 shell 字符串。CLI 会处理实际 diff、验证、候选集成、原生 checkbox CAS、下一批就绪工作以及验收；宿主无需拼装底层 Git/文件命令。结果相同的重提是幂等的；不同结果、错误身份或过期输入被拒绝。

## 高级工具

```sh
spec-autonomous tools list --all --json --limit 200
spec-autonomous tools call document.inspect --input '{"file":"specs/001-auth/tasks.md"}' --json
spec-autonomous tools call roadmap.get --input '{"milestone_id":"M001"}' --json
spec-autonomous tools call audit.open --input '{"run_id":"<run-id>"}' --json
spec-autonomous tools call worktree.list --json
```

目录包含参数 schema、读写属性和说明。支持结构化文档/frontmatter/TOML CAS、roadmap 增删与范围选择、任务领取和回执、状态/决策/阻塞/检查点、Git 显式文件提交、worktree 生命周期、历史摘要、校验和修复。`task.complete` 与 apply-result 复用同一验收门，不能直接伪造已验证状态。

读取默认使用 agent 视图；需要详细数据可加 `--view full`。`--fields` 按字段选择，`--limit` / `--offset` 分页。分页保留全仓聚合数量和后续偏移；unknown/stale/partial 和 host-reported 状态不被伪装为已完成。

## 归档、暂停与修复

```sh
# 先预览，再使用返回的 plan_hash 执行
spec-autonomous archive --change add-auth --json
spec-autonomous archive --change add-auth --apply --plan-hash <hash> --json
spec-autonomous archive --milestone M001 --json

spec-autonomous pause <run-id>
spec-autonomous cancel <run-id>
spec-autonomous tools call repair --input '{"kind":"legacy-config"}' --json
```

归档在隔离候选 worktree 中完成，校验后 fast-forward 交付。OpenSpec 使用原生 archive；Spec Kit 保留 feature 目录内容搬入明确归档目录，同时协调当前 feature 指针。里程碑归档按依赖顺序处理全部原生来源并保留 roadmap。预览过期、活跃工作、dirty/locked workspace 或交付分支改变时拒绝或保留待交付候选。

暂停/取消返回宿主需要处理的请求；CLI 不宣称能停止外部 agent。宿主停止它们后，用 `work.revoke` 确认。过期心跳只表示未知状态，不触发重复执行。历史 alpha.1 run 只读；新账本版本阻止旧版程序误续跑。

## 配置与测试

项目配置示例：

```toml
schema_version = 2
verification = [{ argv = ["cargo", "test", "--workspace", "--locked"], cwd = "." }]

[execution]
max_workers = 3
max_attempts = 3
run_timeout_seconds = 28800
max_repair_rounds = 2

[host]
max_concurrency = 3
lease_seconds = 1800
```

宿主并发能力未声明时默认串行。框架与验证命令可使用独立 `environment` 配置；旧 `runner` 字段被忽略并诊断，不会执行。

```sh
# 不调用真实模型、不发布 npm
node scripts/test-all.mjs

# 创建本地仓库，由独立测试宿主模拟语义工作
node scripts/create-mock-repo.mjs --framework openspec --output .artifacts/my-mock --fail-once add
node tests/mock-host.mjs --path .artifacts/my-mock prepare --milestone M001 --max-workers 2
spec-autonomous progress --path .artifacts/my-mock --all-worktrees
```

mock 宿主位于测试目录，不随产品包分发，也不被 CLI 调用。实际 Git、SQLite、OpenSpec、进程验证、CAS、集成和恢复都走生产代码。
