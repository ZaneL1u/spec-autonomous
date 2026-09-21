# 如何贡献

## 开始之前

先读：

- 根目录 `AGENTS.md`：产品意图、目录、工具链和设计不变量；
- `README.md`：当前已实现边界；
- `docs/architecture.md`：host-driven 协议；
- 对应的 `openspec/changes/<change>` 或主规范。

不要因为 proposal / design / tasks 已经存在，就把计划中的功能当作已经实现。

## 本地环境

需要：

- Git
- Rust 1.98.1（以 `rust-toolchain.toml` 为准）
- Node.js 22.13+（或 23.5+）
- Bun 1.4.2

```sh
. "$HOME/.cargo/env"
bun install --frozen-lockfile
```

## 开发流程

行为变化先使用仓库现有 OpenSpec schema：

1. 新建或更新 change；
2. 写清 requirement 和 scenario；
3. 更新设计与任务；
4. 实现；
5. 添加 Rust contract / Node contract / E2E；
6. 通过严格规范校验；
7. 验证后才能勾选对应任务。

## 质量门

```sh
# 完整套件
node scripts/test-all.mts

# 文档站
bun run docs:build

# 本地 npm 包
bun run pack:local
```

完整测试包括：

- TypeScript 类型检查与运行时构建；
- Rust format、Clippy 和 contract tests；
- 真实锁定版本 OpenSpec contract；
- launcher / package tests；
- Git、进程、恢复和 host-driven E2E；
- OpenSpec strict validation。

测试中的 `tests/mock-host.mts` 是独立宿主 fixture。产品 CLI 不导入、不启动也不分发它。

## TypeScript 与构建产物

Rust 之外的源码全部是 TypeScript（`.mts`）。发行运行时写在 `packages/cli/src/`，由 tsdown 构建成 `packages/cli/bin/*.mjs` 与 `packages/cli/lib/*.mjs`：

```sh
bun run typecheck
bun run build:cli
```

这些 `.mjs` 产物**需要提交**，因为 Git 安装会直接从 clone 执行它们。改完 `src/` 请重新构建并把产物一起提交，CI 会校验二者一致。`scripts/` 与 `tests/` 不构建，由 Node 的类型擦除直接运行。

## 文档贡献

网站源码在 `website/`：

```sh
bun run docs:dev
```

正文使用中文；命令、标识符、产品名和 URL 保留英文。优先用大白话解释“为什么”和“什么时候用”，再给底层细节。

更新命令或 MCP 文档时，以以下事实源为准：

- `crates/core/src/capabilities/catalog.rs`
- Clap CLI metadata 和 `--help`
- `packages/cli/skills/*/SKILL.md`
- contract / E2E 测试

## 设计底线

- CLI 永远不创建 Agent 或模型会话；
- 原生规范拥有需求权威；
- unknown write set 保守串行；
- exit 0、checkbox、验证、集成不可混为一谈；
- 恢复先 reconcile，不盲目重放；
- 不能把尚未实测的平台称作支持；
- 不修改 `.references` 中的锁定上游研究副本。

## 提交与 Pull Request

提交使用 Conventional Commits，描述使用简体中文，例如：

```text
feat(core): 增加工作包批量领取
fix(test): 隔离全局 provider 环境
docs(site): 补充 MCP 使用指南
```

Pull Request 请说明：

- 解决什么问题；
- 对应规范 requirement；
- 关键设计边界；
- 执行过哪些验证；
- 有哪些未覆盖平台或后续工作。

不要提交 secret、runtime log、`target/`、`node_modules/` 或 Agent 会话内容。
