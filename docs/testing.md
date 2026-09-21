# 测试与复现

alpha.2 采用工作包与回执协议。产品 work_packet 模块只生成与校验数据；测试中的 `tests/mock-host.mts` 是独立宿主，由它创建模拟语义进程并调用 CLI 回执接口。测试宿主与 mock-agent 不随 npm 包分发。

## TypeScript 源码与构建产物

除 Rust 外的源码全部是 TypeScript（`.mts`）。发行运行时写在 `packages/cli/src/`，由 tsdown 构建到 `packages/cli/bin/*.mjs` 与 `packages/cli/lib/*.mjs`。这些 `.mjs` 是**提交进仓库的构建产物**，因为 Git 安装会直接从 clone 执行它们；不要手改，改完源码后运行 `bun run build:cli` 并提交产物，CI 会用 `git diff --exit-code` 校验二者是否一致。

`scripts/` 与 `tests/` 不构建，由 Node 的类型擦除直接运行 `.mts`。它们 import 的是 `packages/cli/lib/*.mjs` 产物而非 `src/`，因为 `../locales` 和 `../bin/provider-bridge.mjs` 等运行时资源路径是相对产物布局解析的；构建同时生成 `.d.mts`，这些 import 仍然受类型检查覆盖。

类型检查单独入口为 `bun run typecheck`（`tsc --noEmit`），已包含在 `test:all` 与 CI 中。

## 完整入口

```sh
. "$HOME/.cargo/env"
bun install --frozen-lockfile
node scripts/test-all.mts
```

等价命令为 `bun run test:all` / `npm run test:all`。Node 驱动执行 fmt、clippy、Rust unit/contracts、显式真实 OpenSpec 契约、Node launcher/package/publisher tests、debug CLI e2e 和 strict OpenSpec validation。它不调用真实模型，不发布 npm。

`test:all` 不包含 release build 与安装 smoke，CI 另行执行；本地用 `node scripts/pack-local.mts` 生成本机 tarball。

## 测试层与可注入边界

| 测试 | 主要验证 |
| --- | --- |
| discovery 原有单测 | 框架发现、Git 边界、歧义、symlink 与不执行脚本 |
| config_contracts | 配置优先级、非法字段、预算、敏感值不回显 |
| provider_contracts | 原生解析、CAS、ID、CRLF、路径、上下文、checklist/hooks、真实 OpenSpec 自定义 schema |
| plan_contracts | 图覆盖、循环、范围、稳定标签、冲突、父子任务和原生顺序 |
| runtime_contracts | Git/SQLite 原子性、版本备份、锁、明确验证子进程、结果协议和 100 个外部 fixture contexts |
| progress_contracts | 全 worktree join、unknown/stale/partial、只读、去重和输出语义 |
| host_protocol_contracts | 无 launcher、prepare 幂等、回执重放、owner/session、过期与撤销、旧 run 只读、规划不等于验收 |
| mcp_contracts | 初始化、错误协议、有限工具列表、输入上限、EOF、分页与参数校验 |
| skill_contracts | 自有文件升级/卸载、冲突预检查、宿主与命名空间绑定 |
| Node package tests | 平台选择、参数/信号、真实 tgz/skills、分包、离线发布 gate |

Rust runtime 的 external_test_host 仅在测试中执行子进程，生产模块不存在对应启动函数。Node fixture 中的旧 runner 配置用于验证兼容性和供外部测试宿主选择特殊故障脚本，产品明确忽略它。

## 真实 Git/进程 e2e

- autonomy：两种框架 goal→roadmap→原生工件→任务→审核、阶段范围、并行、失败修复、六个集成 crash windows、跨 worktree progress。
- budgets：无进展停止跨恢复保留，失败任务不阻止独立工作完成。
- recovery / verification：原生手工交接、证据失效、环境提示、宿主暂停/截止时间、变更树的验证拒绝、审核覆盖和源漂移。
- resources：大上下文 hash 引用、40 项任务规划分批、父任务拆分、进程证据与安全清理。
- hooks：原生幂等/非幂等 hook 的副作用协调与 plan 模式门槛。
- capabilities：结构化文档 CAS、roadmap、受控 Git commit、worktree、归档与修复。
- host-receipts：receipt 文件前后中断、archive candidate/FF 前后中断，均不重做已完成语义工作或重复移动来源。
- mcp：真实 stdio 子进程握手、八个默认入口、CLI/MCP 同语义、trap runner 不启动、外部 receipt 与配置所有权。
- scheduling：同样六项任务在 1/3 workers 下比较实际重叠、并发上限与交付等价；记录墙钟，不以速度阈值替代正确性。

外部宿主负责会话启动/停止、超时和宿主环境；CLI 只报告要求并处理回执。所有 Node 测试子进程清除 NODE_TEST_CONTEXT 等测试框架内部变量，避免嵌套 `node --test` 返回零却未执行断言。

## 保留一个可检查的 mock 仓库

```sh
node scripts/create-mock-repo.mts --framework openspec --output .artifacts/my-mock --fail-once add
node tests/mock-host.mts --path .artifacts/my-mock prepare --milestone M001 --max-workers 2
node packages/cli/bin/spec-autonomous.mjs progress --path .artifacts/my-mock --all-worktrees --json
```

目标目录必须不存在。测试 fixture 的 .mock/scenario.json 定义语义结果；验证使用真正的 Node assertions，集成使用真正的 Git，状态使用 SQLite。临时 e2e 仓库自动清理，以上持久 mock 保留 worktrees、输入、回执和证据。

## 单独运行

```sh
cargo test --workspace --locked
cargo test -p spec-autonomous-core --test host_protocol_contracts --locked
cargo test -p spec-autonomous-core --test provider_contracts \
  real_openspec_plans_then_accepts_skipped_specs_and_custom_tracking_artifact -- --ignored --exact
node --test packages/cli/test/*.test.mts scripts/test/*.test.mts
cargo build -p spec-autonomous-cli --locked
node --test --test-concurrency=2 tests/e2e/*.test.mts
```

不要使用宽泛 `--ignored` 启动 subprocess 辅助入口。Debug failpoints 通过 SPEC_AUTONOMOUS_TEST_FAILPOINT 选择，release binary 不启用它们。

## 验收边界

本机结果与数量见 [host-driven 验收](validation/host-driven.md)。旧 [alpha.1 验证](validation/autonomous.md) 是历史记录，其中 CLI 启动真实 Codex 的证据不能用于证明新协议。

CI 已配置多平台构建/测试及日志上传，真实结果必须来自对应 Actions run；本机 macOS 检查不能证明其他平台通过。npm ownership、Trusted Publisher、provenance 发布与 dist-tag 回退仍需真实远程环境。
