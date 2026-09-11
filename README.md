# Spec Autonomous

**给定里程碑目标，基于 OpenSpec / Spec Kit 规划 roadmap，并按完整里程碑或 from/to 范围自主开发。**

既能从目标开始，也能承接原生流程已有的规范、计划和任务。原生手动与自主模式操作同一批 Markdown；TOML 保存配置和编排声明，CLI 提供结构化读取，skills 提供自然语言入口。主协调器维护里程碑，fresh-context agents 在独立 worktrees 工作，progress 汇总所有 worktree 的实际状态。

当前是 **`0.1.0-alpha.0` 仓库初始化阶段**：已实现只读检测 CLI、Rust workspace、npm launcher 与打包脚本。产品 skills、TOML roadmap、from/to、全 worktree progress 和 autonomous runtime 已列入 OpenSpec 方案，**尚未实现**。现有 openspec-* skills 是上游集成。项目名/npm 包名暂定 spec-autonomous，尚未发布或预留。

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
spec-autonomous init
spec-autonomous milestone new "团队邀请与权限 MVP" --framework openspec --mode native
spec-autonomous milestone new "团队邀请与权限 MVP" --framework openspec --mode autonomous
spec-autonomous run --milestone M001 --from 2 --to 4 --mode autonomous
spec-autonomous run --milestone M001 --only 3 --mode autonomous
spec-autonomous progress --all-worktrees
spec-autonomous progress --all-worktrees --format toml
spec-autonomous progress --all-worktrees --format json

# 已有原生规划也可直接接入
spec-autonomous inspect --framework openspec --change add-team-auth
spec-autonomous run --framework openspec --change add-team-auth --autonomous --max-workers 3

spec-autonomous run --framework speckit --feature specs/001-auth --autonomous --max-workers 3
spec-autonomous status
spec-autonomous resume <run-id>
```

安装绑定后，支持 slash commands 的宿主可直接用 `/autonomous` 或同义别名 `/auto`，另有 `/milestone`、`/progress`、`/resume`。仅支持 skills 的宿主采用其原生等价语法（如 `$autonomous` / `$auto`）。这些入口调用同一 CLI 协议，不持有第二份调度状态。

底层依赖用户当前仓库的 SDD 框架：OpenSpec 项目走其 schema/instructions/artifacts，Spec Kit 项目走其原生 templates/skills/artifacts。init 负责检测和绑定，不迁移或替换 SDD；未检测到框架时给出选择/安装指引，不静默使用本产品自造流程。

完整路径：目标 → 研究与 roadmap → 每 phase 的原生规划 → 执行图 → 自动实现/验证/修复/集成 → 下一 phase → 整体验收。每个 roadmap phase 对应一个原生 change/feature，已有单一来源直接映射为一个 phase。原生模式交出原生命令继续，autonomous 自动推进同一流程。

from/to 指 roadmap 阶段的闭区间，only 只跑一个 phase；不能绕过范围外未完成依赖，范围完成不会提前归档整个 milestone。TOML 编排数据与原生 MD 规范分开保存，MD 的 frontmatter/标题/checkbox/任务 ID 通过 CLI 解析为带来源的结构化视图。progress 从 Git common dir 汇总所有 managed/external worktree，规划百分比与验证进度分开显示。

## 工程约定

```text
crates/core/                 检测；后续拆出 adapter/planner/scheduler/state 等模块
crates/cli/                  Rust CLI 入口
packages/cli/                Node launcher；后续加入本产品 skills 与 installer
scripts/                    本地构建、npm 组包、锁定源码复现
openspec/specs/              已交付能力的主规范
openspec/changes/            当前实施提案与任务
docs/research/              调研、源码 permalink、upstreams.lock.json
.references/                上游 clone，gitignore，只读，不随 npm 发布
.spec-autonomous/           未来 TOML 编排声明与 runtime；实现时分别设置 Git 跟踪规则
```

```sh
bun run check
bun run test
OPENSPEC_TELEMETRY=0 bun run spec:validate
bun run references:clone
```

本仓库采用官方 `spec-driven` schema，OpenSpec CLI 固定 `1.13.0`，Codex 集成已生成到 `.agents/skills/`。自主执行路线仍是开放变更，不因规划文件齐全而标记实现完成。
