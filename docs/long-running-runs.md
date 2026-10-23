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

同波次完全相同的 argv/cwd 只执行一次。跨波次的复用以「检查前后一致的 Git tree hash + argv + 相对 cwd + 运行环境 + 准备钩子 + 检出类别」为键：worker 候选与 combined 候选同树时，combined 直接复用 worker 的通过证据；phase、post-hook、milestone 在同一集成提交上也只执行一次。新建 worktree 与长期集成检出属于不同类别，互不复用，所以独立复验仍能发现集成环境问题。失败、超时、取消和改动源码树的结果从不复用；树、命令、环境或 `run.revise` 任一变化都会重新执行。复用的证据带 `reused_from`（原 wave、manifest、日志）且 `duration_ms` 为 0。`[execution] verification_reuse = "none"` 可恢复逐波次执行。after_implement hooks 没有改变已接受提交时不再跑 post-hook 波次。

## 续跑不清零

续跑先对账原始检出：若其 HEAD 只是吸收了已接受的集成历史（例如用户把原分支 fast-forward 到集成分支），只更新记录的原始提交，不合并、不重新规划、不重新验证。否则合并原始改动，并只按原始一侧新增的文件分类：命中某 phase 原生源目录的改动只重开该 phase（重新规划其新增任务），依赖它的 phase 保留已接受工作与计划，进入 `stale_phases`；其他文件的改动把所有已完成 phase 标为 stale。stale phase 在全部选中 phase 完成后、交付前只重跑命令门（`stale_phase_verification` 事件），同树命中复用则不实际执行；失败走既有修复路径。审计结论不因外部代码改动重做，因为原生任务与验收引用未变。handoff 后用户直接在集成分支提交的情况按同一规则处理。

### 源变化不丢弃在途工作

运行中若原始检出的规范文件发生变化，而宿主仍持有或正在集成工作，协调器记录 `source_drift_deferred` 并继续：在途 attempt 照常提交并对照当前集成版本验证集成，期间不签发新任务；全部回执落地后再合并原始改动（`source_reconciled`）并重新进入循环。只有在没有任何在途工作时，源漂移才会立刻暂停。续跑时若原始检出已前进而在途工作尚未结束，同样记录 `source_reconcile_deferred` 而不是要求撤销。

### 计划增量扩展

原生跟踪文件追加了新任务（收敛修复或用户编辑）时，不再整体重新规划：已接受的计划保留，只把未覆盖的新任务交给 planner（packet 的 `snapshot.tasks` 只含新增任务，`metadata.existing_plan` 列出已有任务），返回的任务追加到计划末尾并默认依赖原计划的叶子任务（`plan_extended`，提交 `sa: extend execution plan`）。只有当计划覆盖的源任务被删除时才会整体重新规划。

### 审计复用与重复失败

完全通过的审计以 `(phase, scope, 集成树哈希, 验收引用批次)` 为键缓存；同键再次出现时复用结论（`audit_reused`），不再派发审计 agent。修复轮按 `phase:失败签名` 计数：host-verification 失败以失败命令、cwd 和退出码为签名，审计失败以未通过的需求 ID 为签名；同一签名在一轮修复后再次出现，run 进入 `needs_input`（`repair_repeated`），不再消耗剩余修复预算。phase 完成时清除其签名计数。

### 分阶段检查与规划护栏

`[[task_verification]]`、`[[phase_verification]]`、`[[milestone_verification]]` 分别只在任务候选、phase 门槛和里程碑门槛执行；`[[verification]]` 保持原语义（每个阶段）。昂贵的全量套件放到 phase 或 milestone 级，worker/combined 只跑快速检查，combined 对已完成任务检查的累加也只涉及任务级命令。

检查 argv 中单个元素超过 1024 字节，或包含 base64 / b85decode / zlib.decompress 等编码载荷，会被 `invalid_command: inline_script_argv` 拒绝；规划指引要求脚本必须是任务 `writes` 中声明的文件。preflight 把 `writes` 中将创建的脚本（`check_script_deferred`）和可执行文件（`check_executable_deferred`，不再要求带目录前缀或执行位）视为延后而不是缺失；不在 writes 中且不存在的脚本操作数报 `check_script_missing`。

规划包的 `verification_environment` 现在包含实际运行时版本（node、python3、go、cargo、bun、npm、git）、`package.json` scripts、项目清单文件、已有测试文件（最多 100 条，附总数）以及常见缓存目录是否被 git 忽略；`previous_planning_failures` 列出该规划单元历史上全部被拒绝的候选（最多 10 条）及其诊断代码。

预算硬上限：`max_attempts ≤ 10`、`max_repair_rounds ≤ 4`、`run_timeout_seconds ≤ 72 小时`，超出需显式 `[execution] unsafe_budgets = true`；已有 run 的账本配置同样受限，可用 `prepare --run-id … --reload-config` 读取修改后的项目配置。doctor 对超过默认值两倍的 `max_attempts`、`max_repair_rounds`、`run_timeout_seconds`、`no_progress_limit` 以及 `unsafe_budgets=true` 输出 `budget_warning`。

## 失败诊断与进度

每个检查保存 `result.json`，包含阶段、准备/检查步骤、命令、目录、提交、耗时、退出码、失败类型，以及 stdout/stderr 文件和哈希。公共错误只提供诊断位置，避免把任意原始输出复制到会话。超时、取消、无法启动、日志限额、断言失败和源码变化会分开标注。

progress 的 overhead 展示尝试类型/状态、验证次数（`verification_checks`，含复用）、实际执行次数（`executed_checks`）、复用次数（`reused_checks`）、历史失败、修订次数，以及有记录部分的验证耗时。旧证据没有耗时，因此必须看 duration_coverage，不能把 partial 当完整总耗时。

显式包含独立执行仓库：

```sh
spec-autonomous progress --all-worktrees --related-path /path/to/executor --json
```

最多 16 个路径，以 Git common directory 去重，不递归寻找其他仓库。主仓与 related_repositories 分开计数，后者保留自己的分页和错误。MCP/能力输入为 `repository_paths`。

已声明但没有 run 的里程碑显示为 `not_started`，并读取原生 tasks.md 的勾选情况；勾选不推断已验证完成。自定义任务文件、不可读取源或归档源显示不可用，不启动原生工具补写状态。
