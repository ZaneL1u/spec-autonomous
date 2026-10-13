# 长任务维护与安全更新

这些能力用于减少长任务的编排与验证开销，仍保留独立验收。已有 run 的 ID、已接受提交、原生需求和历史失败继续保留；安装新版本不会自动改写旧验证命令，也不会自动完成剩余业务。

## 更新正在运行的项目

1. 将新包安装到独立前缀，保留原 CLI。正在执行的 apply-result、验证命令和 MCP 继续使用原来的可执行文件。不要覆盖其二进制、清理其工作树或在另一终端同时 resume。
2. 等当前命令结束并确认协调器锁已释放。宿主保存 checkpoint；所有已领取工作要么提交回执，要么确认停止后 revoke。不要把心跳过期当成进程已退出。正在运行的进程不能直接迁移。
3. 在没有写入者时保存配置、原生规划、Git refs 与 SQLite 一致性备份（使用 SQLite backup API 或 `.backup`，不要只复制活跃的 state.db 而遗漏 WAL）。历史产物继续保留。
4. 使用独立安装的绝对路径执行 `doctor --json` 和 `progress --all-worktrees --json`。核对 run ID、accepted_head、已验证任务及账本版本；不要使用 prepare 作健康检查。
5. 在同一安全交接点刷新项目 skills/MCP 配置。skills 安装会检查所有权，遇到用户修改不会覆盖。已有 MCP 服务需要由宿主在交接后重新连接到新可执行文件；不能假定安装成功就替换了内存中的旧服务。
6. 由唯一宿主用新可执行文件继续**原 run ID**。不要新建里程碑、重置勾选、取消后重来，或混用新旧写入者。

安装示例（将变量换成本次交付的包和独立前缀）：

```sh
npm install --prefix "$SA_INSTALL_PREFIX" "$SA_PACKAGE"
"$SA_INSTALL_PREFIX/node_modules/.bin/spec-autonomous" --version
"$SA_INSTALL_PREFIX/node_modules/.bin/spec-autonomous" --path "$SA_PROJECT" doctor --json
"$SA_INSTALL_PREFIX/node_modules/.bin/spec-autonomous" --path "$SA_PROJECT" progress --all-worktrees --json
```

这是本机候选包的使用方式。只有对应提交和 Release 资产发布后，远端 Git 安装命令才会获得同一版本。不要仅凭 alpha 版本字符串判断二进制内容相同；保留包的 SHA-256 和提交号。回退前确认旧程序支持账本版本；不能盲目降级写入。

## 运行中回收

在安全交接点预览当前 run 已结束的资源：

```sh
spec-autonomous tools call run.cleanup --input '{"run_id":"RUN_ID","active":true}' --json
```

核对清单后，使用完全相同参数，加上 `apply:true` 和返回的 `plan_hash`。应用需要独占协调器锁；run、工作树或范围变化会拒绝旧预览。

- 保留集成工作树、当前目录、活跃持有者、未完成 intent、恢复用 accepted 工作树、未完成任务的失败尝试及当前工作包引用的资源。
- 保留脏、锁定、未知归属、共享工作树。活跃 run 中含 ignored 文件的工作树也保留，避免误删运行产物。
- 默认保留分支和所有账本、日志。`delete_branches:true` 只删除已安全合入的自有分支。运行中不允许 `delete_integration:true`。
- 预览给出 retained_details。大量 ignored 证据应由其产生者完成归档与生命周期管理，不能强删来获得空清单。

progress 的 resource_summary 给出工作树数量、状态分布和压力提示。它不递归扫描数百 GB 的目录，也不把可回收字节估算当作真实剩余磁盘空间。

## 每轮验证的产物与准备

协调器为每轮独立验证分配一个全新的目录，并给准备命令和该轮检查提供以下环境变量：

| 变量 | 含义 |
| --- | --- |
| `SA_VERIFY_OUTPUT_DIR` | 当前波次独占的报告/临时准备目录，位于 Git common runtime 中 |
| `SA_VERIFY_WAVE_ID` | 波次唯一 ID，可用于生成独立测试资源名称 |
| `SA_VERIFY_STAGE` | worker、combined、phase 等真实执行阶段 |
| `SA_VERIFY_REVISION` | 正在验证的源码提交 |
| `SA_VERIFY_CHECKOUT` | 当前验证工作区 |

验证程序应读取环境变量生成报告路径，避免在计划 argv 中写死 `final03/windows.json`。环境变量不会经过隐式 shell 展开；`["tool", "$SA_VERIFY_OUTPUT_DIR"]` 仍是字面字符串。通过读取环境的脚本构造路径。

可选准备钩子使用现有配置格式：

```toml
[hooks.verification_prepare]
argv = ["node", "scripts/prepare-verification.mjs"]
idempotent = true
```

钩子每波次执行一次，先于检查，使用同一组环境变量。它应只准备该波次拥有的资源，将连接信息或资源凭据放在受保护的波次目录中，检查读取这些文件；子进程修改环境不会自动传回协调器。以波次 ID 保证幂等，worker 与 combined 使用不同数据库或可重建夹具，禁止复用已被消费的状态。准备失败则不运行后续检查。不得修改被验证源码。

这是准备契约，不是通用数据库管理器。数据库销毁、VM 所有权及凭据保护仍由项目的准备脚本负责。本次不会替现有业务脚本自动重置数据库。

旧 run 需要在工作包停止后，通过 `run.revise` 正式修订固定路径；钩子采用配置受控刷新。旧验证门槛不自动删除。重新规划会保留已审查的任务验证修订，遇到新的不一致检查会明确拒绝，要求宿主核对原生要求。

同波次完全相同的 argv/cwd 只执行一次。worker 与 combined 仍分别执行，不跨阶段缓存 PASS；独立复验确实能发现环境和业务错误。

## 失败诊断与进度

每个检查保存 `result.json`，包含阶段、准备/检查步骤、命令、目录、提交、耗时、退出码、失败类型，以及 stdout/stderr 文件和哈希。公共错误只提供诊断位置，避免把任意原始输出复制到会话。超时、取消、无法启动、日志限额、断言失败和源码变化会分开标注。

progress 的 overhead 展示尝试类型/状态、验证次数、历史失败、修订次数，以及有记录部分的验证耗时。旧证据没有耗时，因此必须看 duration_coverage，不能把 partial 当完整总耗时。

显式包含独立执行仓库：

```sh
spec-autonomous progress --all-worktrees --related-path /path/to/executor --json
```

最多 16 个路径，以 Git common directory 去重，不递归寻找其他仓库。主仓与 related_repositories 分开计数，后者保留自己的分页和错误。MCP/能力输入为 `repository_paths`。

已声明但没有 run 的里程碑显示为 `not_started`，并读取原生 tasks.md 的勾选情况；勾选不推断已验证完成。自定义任务文件、不可读取源或归档源显示不可用，不启动原生工具补写状态。
