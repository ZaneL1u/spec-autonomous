# Bootstrap 验证记录

日期：2026-09-10（America/Los_Angeles）。机器：macOS arm64。
版本：Rust 1.98.1、Bun 1.4.2、Node 24.21.0、npm 11.19.0、OpenSpec 1.13.0。

## 已实测

| 检查 | 结果 |
| --- | --- |
| bun install + lock | 成功安装锁定的 OpenSpec；生成 bun.lock |
| cargo fmt --check / clippy -D warnings | 通过 |
| cargo test --workspace --locked | 6 项 core tests 通过 |
| Node launcher / release tests | 6 项通过，含参数原样传递、exit 7、SIGTERM、缺 binary、musl 与缺失平台产物 |
| 四个 reference clones | commit、origin 与 clean status 全部与 upstreams.lock.json 一致 |
| 官方 openspec init / new change | 成功；spec-driven，6 个 Codex skills，两个 change |
| OpenSpec strict validation | 初始 2 个 change 通过；bootstrap 归档后 autonomous change + 2 个主规范再次校验，3 passed / 0 failed |
| cargo release / local npm pack | 成功；macOS arm64 二进制约 728 KB，本机 tarball 约 363 KB |
| 自定义 CARGO_TARGET_DIR | 在含空格的 `.artifacts/cargo output` 构建成功；通过 Cargo JSON 找到实际 artifact |
| 本机 npm 安装 | `--global --prefix <含空格路径> --ignore-scripts` 安装成功，version/help/detect 可运行 |
| 分离式 wrapper + native 平台包 | 离线、禁 scripts、显式安装 macOS 平台 tarball 与 wrapper 后运行成功 |
| release tgz 内容/权限 | wrapper 无 native 副本；原生 binary 在 tgz 中保留可执行权限 |
| 已安装 CLI 场景 smoke | 空仓、Spec Kit、双框架、显式选择、无效路径均通过 |
| 本地文档链接 | 17 个相对文件链接全部可解析 |

本机 tarball 位于 `.artifacts/local/spec-autonomous-0.1.0-alpha.0.tgz`。
本地完整包安装测试 prefix 为 `.artifacts/install smoke`；分离平台包安装测试 prefix 为 `.artifacts/release install`。这些都 gitignored，不改 npm registry。

分离平台组包测试使用一个**真实 macOS arm64 binary**和其他五个**不可执行的 packaging fixture**，仅验证六包结构、精确依赖、SHA256 清单、npm pack 和本机 dependency resolution。它不是其他五个平台构建或运行的证据，也不可用于公开发布。fixture 位于 `.artifacts/package contract`。

## 本次修正的 review 问题

- Cargo 不再猜固定 target/release 输出，而是显式 target + compiler-artifact 路径。
- npm pack 通过 Node 执行 npm-cli.js，不用 Windows shell 拼接带空格路径。
- GitHub release job 在上传前生成 tgz，避免 artifact 传输清除 Unix binary 执行权限。
- 自主方案区分 accepted_head / candidate HEAD，避免新任务继承未通过验证的集成候选。
- 最小预算/无进展停止/本机进程树取消前移 M1；OpenSpec 补 skip_specs 与 worktree 路径映射；hooks 补未知结果恢复契约。

## 未验证或未交付

- Autonomous runtime、真实 agent 后端、任务图调度、SQLite 恢复尚未实现；开放变更的任务继续未勾选。
- Linux、Windows、macOS Intel、Windows arm64 等未在本机运行；GitHub workflows 已配置但尚未触发远程 CI。
- Node 22 和 Bun 全局安装未做本次端到端运行；当前实测 Node 24/npm 11。
- registry 名称/组织所有权、npm publish、provenance、最低 OS/glibc 版本仍待发行阶段验证。
- 不将 OpenSpec 的 isPlanningComplete=true 或任务 all_done 当作 runtime 完成证据。

完整 roadmap 见 [autonomous tasks](../../openspec/changes/autonomous-orchestration/tasks.md)。

工程初始化变更已由官方 CLI 归档到 `openspec/changes/archive/2026-09-10-bootstrap-workspace/`，8 条已交付行为要求进入主规范。归档时 autonomous-orchestration 有 32 项未完成任务；后续追加的 skills、roadmap、范围执行与全 worktree progress 以当前 tasks.md 为准。本记录不代表这些新增能力已实现。
