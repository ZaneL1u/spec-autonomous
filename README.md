# Spec Autonomous

**用 OpenSpec / Spec Kit 规划好一个里程碑，然后让它自动开发到验收完成。**

复用你已经维护的规范、计划和任务。Rust 主协调器负责里程碑状态、任务调度、验证、修复、集成与恢复；每个子 agent 使用全新会话和独立 worktree。主线程只维护决策、依赖和简短摘要。

当前是 **`0.1.0-alpha.0` 仓库初始化阶段**：已实现只读框架检测 CLI、Rust workspace、npm launcher、Bun 工具链与打包脚本；整里程碑 autonomous 执行已经形成 OpenSpec 方案，**尚未实现**。项目名和 npm 包名暂定 `spec-autonomous`，尚未发布或预留。

## 从这里开始

- [完整技术方案](openspec/changes/autonomous-orchestration/design.md)：自主开发闭环、模块接口、状态机、任务图、上下文、恢复和版本路线。
- [实施任务清单](openspec/changes/autonomous-orchestration/tasks.md)：分阶段验收与并行开发边界。
- [调研结论与上游快照](docs/research/README.md)：OpenSpec、Spec Kit、GSD 的真实适配点。
- [本次验证结果](docs/validation/bootstrap.md)：本机已通过的验证与尚未验证的范围。
- [npm 发行说明](docs/distribution.md)：平台包、CI、发布顺序与安装验证。

## 本地使用（现在可运行）

需要 Git、Node.js 22+、Bun 1.4.2、Rust 1.98.1。Rust 由 `rust-toolchain.toml` 固定。

```sh
# 本次环境已安装 Rust；新终端若尚未加入 PATH，先执行
. "$HOME/.cargo/env"

bun install --frozen-lockfile
bun run build
node packages/cli/bin/spec-autonomous.mjs detect --json
node packages/cli/bin/spec-autonomous.mjs detect --path /path/to/repo --framework speckit

# 生成含本机 Rust 二进制的 npm 包，然后全局安装
bun run pack:local
npm install -g ./.artifacts/local/spec-autonomous-0.1.0-alpha.0.tgz
spec-autonomous detect --json
```

本地 tarball 仅适合生成它的平台。**正式发布后**的安装入口为：

```sh
npm install -g spec-autonomous@next
# 稳定版发布后使用 npm install -g spec-autonomous
```

发布包通过平台 optional dependency 提供二进制，用户无需 Rust 或 Bun。当前不能把这条 registry 命令当成已上线产品；没有执行 npm publish。

`detect` 从指定目录向上寻找最近规范根，遇 Git 根停止；只检查直接标记，不扫描 `.references/`、依赖目录或执行仓库脚本。多个框架共存时返回 `ambiguous: true`，不会选择“最新”项目。它确认安装线索，**不保证规范已规划好或可以执行**。路径错误和显式选择未检测到的框架返回 exit 2。

## 目标使用体验（以下命令待实现）

```sh
spec-autonomous inspect --framework openspec --change add-team-auth
spec-autonomous run --framework openspec --change add-team-auth --autonomous --max-workers 3

spec-autonomous run --framework speckit --feature specs/001-auth --autonomous --max-workers 3
spec-autonomous status
spec-autonomous resume <run-id>
```

一次运行覆盖选定里程碑的全部任务与验收：导入已有规划 → 拆成可执行任务图 → 自动派发 → 验证 → 有界修复 → 集成 → 推进下一批 → 整体验收。遇到可自动修复的失败继续推进；确需新增决策、授权、预算，或已无进展才暂停。不会要求你每完成一个任务再发一次“继续”。首版以一个 OpenSpec change 或一个 Spec Kit feature 作为一个完整里程碑；多 change 聚合留到后续版本。

## 工程约定

```text
crates/core/                 检测；后续拆出 adapter/planner/scheduler/state 等模块
crates/cli/                  Rust CLI 入口
packages/cli/                Node launcher；Bun 用作工程工具
scripts/                    本地构建、npm 组包、锁定源码复现
openspec/specs/              已交付能力的主规范
openspec/changes/            当前实施提案与任务
docs/research/              调研、源码 permalink、upstreams.lock.json
.references/                上游 clone，gitignore，只读，不随 npm 发布
.spec-autonomous/           未来运行时状态，gitignore
```

```sh
bun run check
bun run test
OPENSPEC_TELEMETRY=0 bun run spec:validate
bun run references:clone
```

本仓库采用官方 `spec-driven` schema，OpenSpec CLI 固定 `1.13.0`，Codex 集成已生成到 `.agents/skills/`。自主执行路线仍是开放变更，不因规划文件齐全而标记实现完成。
