# Host-driven alpha.2 验收

日期：2026-09-11。版本：`0.1.0-alpha.2`。本轮将产品改为宿主驱动的确定性能力层，移除 CLI agent launcher。CLI 生成工作包、处理结构化回执与确定性操作；宿主负责实际 agent 会话。没有执行真实 npm publish。

## 交付能力

- 默认 7 项完整能力：inspect、progress、prepare、next、apply-result、archive、doctor。
- 当前共 54 项完整/细粒度能力，具备参数 schema、读写属性与统一输出协议；CLI 与 MCP 共用服务。
- MCP 默认 8 个工具：7 项完整能力和 sa_tools；可显式公开全部细粒度工具，并读取受限的版本化 resources。
- 工作包、claim/heartbeat/revoke、host session 唯一性、input hash、canonical receipt intent、幂等回执与完成门。
- 两种原生 SDD 的规划、DAG/并行、from/to/only、验证/修复、原生手工交接、来源修改与恢复。
- 文档/frontmatter/TOML CAS、roadmap、状态/决策/阻塞/检查点、历史、引用和来源校验、显式 Git commit、worktree 生命周期。
- 单一来源与整里程碑归档预览/执行；隔离 candidate、原生 OpenSpec archive、Spec Kit feature 移动、分支/HEAD 检查与中断恢复。
- 五个 Skills 更新，支持可选 Codex/Claude 项目 MCP 配置；冲突在写入 Skills 前预检，保留其他配置和人工改动。

## 本机验证

机器：macOS arm64。Rust 1.98.1，Node 24.21.0 / 22.23.2，npm 11.19.0，Bun 1.4.2。OpenSpec 固定为 1.13.0。

| 检查 | 结果 |
| --- | --- |
| Rust fmt / clippy | 通过，warnings 作为错误 |
| Rust unit / contracts | 128 项通过；另有显式真实 OpenSpec 契约 1 项通过 |
| Node launcher / package / release tests | 24 项通过 |
| Git/host/process/MCP e2e | 56 项通过，语义工作由独立测试宿主完成 |
| OpenSpec strict validation | 两个活动 change、两个 main specs，共 4 项通过 |
| Skills | 五个 quick_validate 通过 |
| 实际 npm 安装 | alpha.2 本机 tarball 在含空格 prefix、ignore-scripts 下安装并运行 |
| 安装包 MCP | 真实 stdio 握手、工具调用、外部回执与安装/卸载所有权测试通过 |
| Node 22 / Bun | Node 22 包测试通过；Bun 1.4.2 在临时目录以 ignore-scripts 安装真实 alpha.2 tarball 并运行 version 通过 |

完整入口为 `node scripts/test-all.mjs`；等价于 npm/Bun 的 test:all。最终原始日志保留在 `.artifacts/alpha2-final-checks.log`、`.artifacts/alpha2-installed-mcp.log`、`.artifacts/alpha2-node22.log`。早一轮完整 e2e 为 53 项，随后加入 2 项信号清理和 1 项安装冲突预检；最终入口包含全部 56 项。

## 本轮实际发现与修复

- 回执文件和 SQLite 不可能一起原子提交：增加 receiving intent，验证文件前/后中断都能重交同一结果，不再需要另一轮语义执行。
- 规划来源版本与阶段验收版本曾混用：改为 host.source_revisions 与 completed phase hashes 分离，规划不能建立已完成证据。
- 人工修改来源时，未采纳旧回执不能继续使用：先停止宿主工作，保留旧回执为 superseded，再导入新来源并重建图。
- Agent 查询结果不能总是完整账本：默认摘要、字段选择与分页；测试消费完整历史时显式遍历分页。
- CLI 的 SIGTERM 也必须停止它自己拥有的验证子进程：新增公共中断信号并验证进程树回收；idle MCP 即使 stdin 未关闭也能退出。
- MCP 配置冲突必须在其他安装写入之前检查：新增共享 preflight，确认 foreign server 不触发半套 Skills 安装。

## 无 agent 启动的证据边界

生产 work_packet 只生成 input/prompt/schema 和校验结果，不存在 agent 执行方法。依赖中没有模型 SDK；prepare、doctor、MCP 即使面对配置的 trap runner，也不执行它。

外部宿主测试程序是 `tests/mock-host.mjs`，不是 CLI 的子模块或依赖。它负责启动测试 worker、分配会话标识、超时/停止/撤销，并向 CLI 提交结果。部分回归夹具沿用旧 runner 字段选择特殊测试脚本，仅被这个外部测试宿主读取；产品忽略该字段并给出诊断。

新的 host session 证据是宿主声明和回执一致性，不能宣传为 CLI 已观察真实模型内部会话。旧 alpha.1 的真实 Codex 测试保留在 [历史验收](autonomous.md)，不算本轮新协议的模型验收。本轮没有自动调用真实模型账户。

## 兼容与发行边界

数据库 schema 升级为 2 并备份旧库；新配置 schema 2，公开 envelope/WorkerResult schema 1。旧 run 可查询，不允许重启其旧 launcher。参见 [迁移说明](../migration-alpha2.md)。

npm 按平台分发二进制的方式保留，正式分包仍需六个平台的真实构建。当前机器仅验证 macOS arm64；其他平台的 CI 文件不是运行证明。npm 名称所有权、Trusted Publisher、真实 registry 发布/provenance/dist-tag 回退不在本地完成声明中。

## 安装产物与可检查 demo

- 本机 tarball：`.artifacts/local/spec-autonomous-0.1.0-alpha.2.tgz`，3,086,916 bytes，SHA256 `d40870c90d3c5bea3eea121d8d211c6652227ef45d55255f7c957d33794a9574`。
- darwin-arm64 binary：6,465,952 bytes，SHA256 `b206f0fd1c092c9fcd12dab65ca340ec9c44d8cc21dc75e773735c0b3bf3f176`。
- 安装 prefix：`.artifacts/alpha2 installed cli`；其中的 release CLI 通过 5 项 MCP/安装 e2e。
- 持久 mock 仓库：`.artifacts/alpha2-demo`，run `run-22b68f0ae8344556a01aac93515ad91a`，状态 completed；独立宿主处理 15 个语义工作单元，两个阶段与原生任务全部验证交付。原始记录 `.artifacts/alpha2-demo-run.jsonl`。
- 本轮最终完整测试：128 Rust + 1 显式 OpenSpec + 24 Node + 56 e2e 全部通过；不包含真实模型调用或真实发布。
