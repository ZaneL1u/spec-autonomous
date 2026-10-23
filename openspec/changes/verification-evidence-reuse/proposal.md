## Why

真实长任务（share-your-terminal、code-wrapper v0.5.5）显示：续跑时 `reconcile_source` 与 handed_off 分支会把已完成 phase 和计划清零；审计结果的每次宿主往返都重新进入 phase 验证；worker/combined 与 phase/post-hook 在完全相同的源码树上重复执行同一组命令。一个 run 里 63% 的验证记录是在同一提交上重复同一条命令，8 个 phase 被"完成"了 28 次。这些重复不是独立复验，而是同树同命令的重跑。

## What Changes

- 验证 PASS 证据以「检查前后一致的 Git tree hash + 命令 + 相对 cwd + 运行环境 + 准备钩子 + 检出类别」为键持久化到 run 账本；同键的后续检查直接复用并记录 `reused_from`，不再执行。失败从不缓存，树变化、环境变化、`run.revise` 改命令都自然失效。
- 无 hook 改变已接受提交时跳过 post-hook 波次。
- 续跑对账不再清空 `completed_phases` / `plans`：origin 仅吸收了 integration 历史（用户 fast-forward）时不合并不重置；外部代码改动把已完成 phase 标记为 `stale`，在交付前只重跑其命令门；原生源目录改动只重开对应 phase，不再沿 `depends_on` 级联，依赖它的 phase 转为 stale。
- progress 的 overhead 区分 `executed_checks` 与 `reused_checks`。
- 源漂移不再丢弃在途工作：记录待合并，继续接收并集成回执，不签发新任务，在途工作全部落地后再合并。
- 原生跟踪文件追加任务时增量扩展已接受的计划（只规划新增任务），不再整体重新规划；converge 不再删除计划。
- 完全通过的审计按 phase/scope/集成树/验收引用缓存并复用。
- 修复轮按 `phase:失败签名` 计数，同一失败在一轮修复后再次出现即 `needs_input`（`repair_repeated`）。
- `run.revise` 只被修订范围内的在途工作阻塞（`blocking_requests`），其他在途工作继续。
- 新增 `task_verification` / `phase_verification` / `milestone_verification`，让昂贵套件只在对应门槛执行；`verification` 语义不变。
- 检查 argv 拒绝内嵌编码脚本与超长元素；preflight 把 writes 中将创建的脚本和可执行文件视为 deferred。
- 规划包携带真实运行时版本、package scripts、清单、已有测试文件、忽略目录状态和全部历史规划拒绝诊断。
- 预算硬上限（`max_attempts ≤ 10`、`max_repair_rounds ≤ 4`、`run_timeout_seconds ≤ 72h`），`unsafe_budgets = true` 才可超出；doctor 对超默认两倍的预算告警。
- 宿主协议：`prepare` / `apply-result` 带 `host` 自动认领并返回令牌；`next` / `request.get` 向拥有者找回令牌；`work.revoke` 可用拥有者身份代替令牌；协调器锁有界等待；心跳过期不再拒收回执；claim/heartbeat 响应不计算 readiness；`run.revise` / `run.cleanup` 在非破坏范围一步应用；discussion 卡片随 prepare 返回并支持 `discussion_auto`。
- skills 改写为 ≤ 60 行的路由式说明，协议细节移入 docs/host-protocol.md。
- attempt 终态后自动回收 worktree（先落盘 patch + hash），已接受候选同样回收；cleanup 不再因 ignored 文件保留已关闭 worktree；`reclaim_worktrees` / `retain_failed_worktrees` 配置。
- 账本 schema 4：attempts / evidence 分表、按行增量写入、首次写入时从 2/3 迁移并备份；旧 CLI 拒绝写入。
- 新增只读 `run.forensics`；`[host.capabilities]` 以 `host_capacity` 返回给宿主。
- 新增 `[execution] verification_reuse = "same-tree" | "none"`，默认 `same-tree`。

## Capabilities

### New Capabilities
- 无。

### Modified Capabilities
- `verified-integration`: 新增同树证据复用要求。
- `run-recovery`: 续跑保留已完成 phase，改为 stale 复验；源漂移延后合并。
- `task-planning`: 已接受计划的增量扩展。
- `recoverable-autonomy`: 修订只被相关在途工作阻塞。
- `milestone-autonomy`: 重复失败签名即停；预算硬上限与告警。
- `verified-integration`: 分阶段项目检查。
- `task-planning`: 内联脚本拒绝、future 脚本 deferred、规划包环境事实。
- `host-work-protocol`: 自动认领、令牌找回、信息性心跳、有界锁等待、一步应用、discussion 随 prepare、host_capacity。
- `run-recovery`: 自动回收 worktree、账本分表、run.forensics。

## Impact

Rust `engine/{lifecycle,recovery}.rs`、`model.rs`、`config.rs`、`git.rs`、`progress.rs`；账本 JSON 仅新增可选字段，ledger schema 不变；JSON/MCP 字段、状态值、错误码保持稳定，仅新增字段与事件种类。文档 `docs/long-running-runs.md`。不启动 agent，不修改消费项目。
