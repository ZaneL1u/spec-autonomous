# 长任务反复验证 / 反复验收 / 反复全量重跑：根因分析与修复方案

调查日期：2026-10-23。样本：本机三个使用 spec-autonomous 0.1.0-alpha.14 的仓库
（code-wrapper 的 v0.5.5 执行仓、share-your-terminal、money-printer）及对应 codex 会话。
第一轮由单人分析，第二轮由 5 个独立 agent 分维度复核（根因反驳、验证流水线、宿主协议与 skill、
存储与资源、规划与 preflight）。本文件是分析草稿，未提交。

## 0. 结论先行

问题确定在 spec-autonomous 引擎和 skill，不在 codex。核心是四个「乘数」叠在一起：

1. **进度被清零再重做**：续跑时 `recover()` 两条路径（origin 变化的 `reconcile_source`、handed_off 后 integration HEAD 变化）
   把已完成 phase 清掉，已完成 phase 被重新验证、重新审计。share-your-terminal 8 个 phase 完成了 28 次，
   code-wrapper 7 个 phase 完成了 21 次，最终 `completed_phases` 都是空。
2. **每次宿主往返都重跑一遍 phase 验证**：审计（audit）结果要宿主回传，续跑时 `drive()` 先重新进入
   `execute_phase → verify(phase)` 再去取已提交的审计结果，所以每个合法的 phase 完成至少跑 3 轮全量验证。
3. **验证阶段天然重复、不复用 PASS**：worker/combined 同树、phase/post-hook 同 HEAD，一条 `go test` 在同一 commit 上重复 63%（v0.5.5）/ 30%（share-your-terminal）。
4. **在途 worker 工作被丢弃**：源文件漂移或 run.revise 都要求先 revoke 全部在途工作。
   v0.5.5 的 193 小时 implement 时间里 84 小时被 revoke、56 小时失败，只有 53 小时产出被集成。

外层再乘上宿主协议的高仪式感（每个任务 10~14 次协调器调用、令牌一丢就只能读 state.db）和
账本/worktree 的无限增长（47 GB、399 个 worktree）。

## 1. 现象数据

| 仓库 / run | 墙钟 | attempts | 验证命令 | implement 失败/撤销 | 规划 agent | 审计 agent | `.git/spec-autonomous` | worktree |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| code-wrapper `v055-executor` run-59674a11 | 271 h | 303 | 1975 | 37 / 91 | 56 | 41 | **47 GB** | 98 |
| share-your-terminal run-908603df | 357 h | 280 | 2238 | 50 / 41 | 67 | 35 | 10 GB | 399 |
| money-printer run-dc47704f | — | 51 | 370 | 6 / 5 | 8 | 5 | 16 GB | 69 |

时间去向（按 attempt 起止时间，含宿主等待）：

| run | implement 合计 | 其中 revoke | 其中失败 | 审计 agent | 规划 agent | CLI 验证命令实测 | 停止→续跑之间的空闲 |
| --- | --- | --- | --- | --- | --- | --- | --- |
| code-wrapper v0.5.5 | 193 h | 84 h | 56 h | 7.7 h | 15.8 h | 2.1 h（覆盖不全） | — |
| share-your-terminal | 174 h | 41 h | 87 h | 39.5 h | 18 h | 18.8 h（估算总计约 44 h） | 约 258 h / 356 h |

share-your-terminal：`go test -p=1 ./...` 在 worker / combined / post-hook / phase 四阶段分别跑了 20 / 18 / 17 / 42 次，
`make race`（8.5 分钟）38 次。code-wrapper v0.5.5：同一任务最多发了 15 次 implement（MN-03 T016），
MN-03 单个 phase 起了 42 次规划器，MN-02 起了 25 次审计。

codex 侧（单会话一周）：prepare 128、doctor 182、run.revise 235、work.revoke 90、work.claim 145、apply-result 75、
`tools call` 217、直接用 python 读 `state.db` 21+ 次、shell exec 4964 次。
用户 10-14 的原话：「之前那个验收之类的太啰嗦了，包括创建了几百个 worktree……不要反复验收重复测试」。

## 2. 引擎侧根因（复核后）

### R1. 续跑时清零已完成 phase —— 两条路径

- `engine/recovery.rs:217` `reconcile_source()`：origin HEAD 变了就把 origin 合并进 integration。
  改动全在 phase 源目录内 → `reopen(phase)`（`lifecycle.rs:406` 沿 `depends_on` 级联，share-your-terminal 所有 phase 都依赖 P01，
  改一下 `specs/002-*/tasks.md` 就重开全部 8 个）+ `plans.remove`；有非 native 改动 → 清空 completed_phases / phase_hashes / plans / source_revisions。
- `recovery.rs:194-209` handed_off 分支：integration HEAD ≠ accepted_head 就同样全清。
  **code-wrapper 的主因是这一条**：28 次 handoff→reconciled 里 25 次 accepted_head 变了，每次跟着 10~16 次 phase_verification、2~7 次 phase_completed、0 个任务集成。
- 复核纠正：「非 native 改动」判定用的是 origin_head..current 的树 diff，**用户把 origin fast-forward 到 integration 分支这种正常操作**
  也被判成外部改动（一次 599 个文件）→ 全清。share-your-terminal 两段纯重验（10-13 五个 phase、10-16 七个 phase）正对应这类 reconcile。
- 复核纠正：`plans.clear()` 不是规划器重跑的主因。`execute_phase` 在任务全勾选时直接返回（`execution.rs:16`），
  67 次重新规划都是因为当前 phase 的 tasks.md 变了（source_hash 不同）。规划器重跑的真正来源是 R4 的 converge 和宿主改 tasks.md。

### R2. 审计往返触发重复 phase 验证（第一轮漏掉，复核新发现）

`lifecycle.rs:142-200` 内层循环：`execute_phase → verify(phase) → audit_snapshot → invoke(audit)` 发出审计包后 `awaiting_host` 停止。
宿主回传后续跑，`drive()` 重新进入循环，**先** `execute_phase`（秒回）再 `verify(phase)` 全量跑一遍，**然后**才在 `invoke` 里取到已提交的审计结果。
验收引用超过 32 条还要分批，每批一次往返。share-your-terminal 28 次完成里每次 phase_verification ≥ 2，
11 次在 wave 中途被 stop 后从零重来。

### R3. 验证阶段重复且无 PASS 复用

| 阶段 | 位置 | 检查集 | 树 |
| --- | --- | --- | --- |
| worker | execution.rs:290 | config.verification + task.verification | base_commit + patch |
| combined | execution.rs:330 | 同上 + 所有已完成任务的 verification（O(n²) 增长） | accepted_head + patch；base_commit == accepted_head 时与 worker **同树**（share-your-terminal 43/43 次都是） |
| phase | lifecycle.rs:177 | config.verification + phase.verification | integration |
| post-hook | lifecycle.rs:199 | 与 phase 完全相同 | 同 HEAD；无 `.specify/extensions.yml` 时 hooks 是空操作，纯重复 |
| milestone | lifecycle.rs:238 | config.verification + milestone.verification | 同 HEAD |

`unique_checks` 只在单 wave 内去重；`Wave::new` 要求全新目录，`verify()` 不查历史 PASS。
其他：8 MiB 日志上限超出直接杀掉整个套件并触发修复轮；每条检查后两次 `git status --untracked-files=all`；每条 `save("verification")` 整体重写 run。

### R4. 修复 / 收敛 / 审计循环

phase 验证失败 → `repair_failure` → `repair_audit` → `converge` 追加任务并 **`plans.remove(phase)`**（`execution.rs:472`）→ 重新规划 → 重新实现 → 重新审计 → post-hook 再验一次。
`repair_rounds` 全局累加不重置；一条写错的 phase 级 argv 永远无法通过 converge 修好，却会烧掉整个修复预算。
审计无缓存：每次循环都重跑，share-your-terminal 审计 agent 花了 39.5 小时。

### R5. 在途工作被丢弃

`engine.rs:504` `check()` 对 origin_sources 哈希变化直接 `source_drift`；`recovery.rs:226-239` 有 issued/claimed/receiving 就拒绝继续并要求 revoke；
`run_revision.rs:288` 任何在途 attempt 都 `revision_busy`。67 次 host_revoked 里 39 次紧跟 source_reconciliation_intent。

### R6. 规划器被逼出畸形验证命令

v0.5.5 的计划把整段测试脚本 zlib+base85 塞进 `python3 -c` argv。原因链：
preflight 要求 argv[0] 现在就可执行（`verification_preflight.rs:282-297`，future 文件必须含 `/` 且在 writes 里且有执行位）；
Python/shell 操作数从不检查，`python3 -c` 畅通无阻；worker 结果上限 64 KiB（`work_packet.rs:238`）解释了为什么要压缩；
`validate_check`（`config.rs:216`）只拒绝空 argv。规划器拿到的环境信息只有可执行文件路径和一句「inspect selected runtime」，没有版本、package scripts、测试目录。

### R7. 资源永不回收

worker / candidate / reconcile worktree 在引擎里**从未**被删除，只有用户手动 `run.cleanup`（`cleanup.rs:363`）；
cleanup 对活跃 run 里含 ignored 文件的 worktree 一律保留（`cleanup.rs:236-245`），attempt 带着 2.3 GB `tmp/`、275 MB `dist/`。
失败 attempt 的 worktree 只被重试指纹读（`execution.rs:217-231`，还会 `git add --all` 改树）。

### R8. 账本写放大

`state.rs:109` 每个事件整体重写 run 载荷（3.3~5.8 MB），含每次心跳和每条验证；6000 事件 ≈ 27 GB 逻辑写入，WAL 再翻倍。
`validate_global_owner` 每次 claim 反序列化全部 run；`progress` 读 16 MB `context-full.json`。

## 3. 宿主协议与 skill 侧根因

- **令牌只出现一次**：token 只在 prepare/claim 输出里，`next` 剥掉（`host.rs:166`），progress 剥掉 `host`；而 revoke/heartbeat/apply-result 都必须带 token。
  宿主上下文一长丢了令牌，就既交不了结果也撤不了工作 → 读 state.db → doctor 看到 stale_outstanding → 再 prepare。直接解释 182 次 doctor、21 次直读数据库。
- 用 `prepare --run-id` 当轮询：每次都走完整协调器（锁、recover、supersede、drive、provider 子进程、discussion、preflight）只为再返回一次 awaiting_host。
- 锁是 try-lock 不等待（`state.rs:25`），并发调用立刻 `project_in_use` / `run_already_active`，宿主只能重试。
- `lease_stale` 在提交时拒绝（`host.rs:351`），逼宿主在每个长任务里发心跳，但所有权从不转移，这个检查保护不了任何东西。
- skill 要求每次 prepare 前 doctor+inspect、每个 phase 前 discussion.next、revise/cleanup 都要预览+plan_hash 两步。实测每个任务约 13 次协调器调用。
- handoff 到 native 后 codex 在主会话手工跑测试「验收」，是用户感知到的另一半重复验收。
- 用户配置放大循环：code-wrapper `max_attempts=20`、`max_repair_rounds=8`、`run_timeout_seconds=878400`（10 天）；默认 3 / 2 / 8 小时；`validate` 无上限，doctor 无告警。

## 4. 修复方案（复核后重排）

### P0 —— 去掉乘数

> 实施状态（2026-10-23，OpenSpec change `verification-evidence-reuse`）：P0 六项全部实现并有契约测试覆盖——整树哈希 PASS 复用、post-hook 跳过、审计缓存、recover 不清零（fast-forward 识别 / 非级联 reopen / stale 复验）、源漂移延后合并且不撤销在途工作、`run.revise` 只被相关在途工作阻塞、计划增量扩展、同一失败签名重复即 `needs_input`。


1. **PASS 证据复用**（R2、R3）：以 `(tree hash, argv, 相对 cwd, 去掉 SA_VERIFY_* 的环境, verification_prepare argv, 超时/日志限额, checkout 类别 fresh-worktree|integration)` 为键，
   在 `HostState.verification_cache` 记录通过的检查；命中时写入新的 Evidence 并标 `reused_from`；只缓存 PASS。
   这同时解决审计往返重验（R2）、worker/combined 同树、phase/post-hook 同 HEAD、milestone 子集。progress 增加 executed/reused 计数。
2. **post-hook 只在 hooks 真的改了 HEAD 时跑**（把 `lifecycle.rs:199` 移到 `accepted_head != audit_head` 分支内）。
3. **审计缓存**：键 `(phase, source_hash, accepted_head tree, 验收 id 批次, 指令, scope)`；packet 的 input_hash 要排除 `execution_evidence`，否则永不命中。
4. **recover 不清零**：reconcile 后按 phase 记录「已验证树」，只把源目录或代码真正变化的 phase 标 needs_reverify；
   取消 `reopen` 的 depends_on 级联，改为 milestone 阶段统一终验；origin 包含 integration 祖先的 fast-forward 不算外部改动；handed_off 分支同理。
5. **在途工作不丢**：`check()` 的 source_drift 改为记录并延后到本批 receipt 提交后处理；`reconcile_source` 不要求先 revoke；
   `run.revise` 只对未开始的检查生效，已在途 attempt 在提交时按 input_hash 判定是否过期。
6. **converge 不删计划**：追加任务到现有计划；同一失败签名第二次出现直接 `needs_input`；phase 级 argv 失败走 revision 而不是 converge。

### P1 —— 配置与规划模型

> 实施状态（2026-10-23）：第 7~10 项已实现——改为 `task_verification` / `phase_verification` / `milestone_verification` 三个数组而非 `Check.scope` 字段（零迁移）；`inline_script_argv` 拒绝 + preflight 对 writes 内脚本/可执行文件 deferred；规划包携带运行时版本、package scripts、测试文件、忽略目录与全部历史拒绝；预算硬上限（10 / 4 / 72h，`unsafe_budgets` 放开）与 doctor `budget_warning`。64 KiB 结果上限保持不变。


7. `Check` 增加 `scope = task|phase|milestone|all`（默认 all，向后兼容），combined = task 范围 + phase 范围，不再 O(n²) 累加。
8. `validate_plan` 拒绝内联脚本 argv（元素 > 512 字节、含 base64/b85decode/exec(/zlib、`-c`/`-e` 长内联体）；
   preflight 把 writes 里将创建的普通文件视为 deferred，Python/shell 操作数同样检查；指引明确「测试脚本必须是 writes 里的文件」。
9. 规划包带真实运行时版本、package scripts、已有测试文件、ignore 状态，以及历史全部 preflight 诊断（按代码汇总）；提高 64 KiB 结果上限或把计划单独存。
10. 预算硬上限：`max_attempts ≤ 10`、`max_repair_rounds ≤ 4`、`run_timeout ≤ 72h`（除非显式 unsafe），doctor 对 > 2× 默认告警；每 phase 规划器/审计次数封顶。

### P2 —— 宿主协议与 skill

> 实施状态（2026-10-23）：第 11~15 项已实现——`prepare`/`next` 接受 `host`，`prepare` 与带 `claim_next` 的 `apply-result` 自动按派生会话认领并返回令牌；`next`/`request.get` 向拥有者找回令牌；`work.revoke` 可用拥有者身份；协调器锁有界等待 10 s；心跳过期不再拒收；claim/heartbeat 响应不算 readiness；`run.revise`/`run.cleanup` 非破坏范围一步 apply；discussion 卡片随 prepare 返回并支持 `discussion_auto`；三份 skill 改写为 ≤ 35 行的路由式说明，细节在 docs/host-protocol.md。宿主能力声明文件（P2 新增项）与 `run.forensics` 未做。


11. `prepare` / `apply-result` / `next` 接受可选 `host`，返回的 work 直接以派生 session_id 认领并带 token（保留现有 claim 接口、字段名、错误码）；
    `next --host` 可按所有者找回 token；`work.revoke` 允许 `host_stopped:true` 代替 token。
12. 锁改为有界等待（约 10 s）再报 `run_already_active`；提交时去掉 `lease_stale` 拒绝，`stale` 只作信息；`lease_seconds` 默认等于 attempt_timeout。
13. 所有视图默认紧凑可分页，`verification_readiness` 只在 prepare/next 计算并按 `(source_hash, accepted_head)` 缓存；新增 `request.get`。
14. revise/cleanup 在没有 active_requests 时允许一步 `apply:true`；discussion 卡片随 prepare 的 needs_input 一起返回。
15. skill 改写：doctor 每会话一次；不要预先 discussion.next；不要单独取 readiness；禁止读 state.db；「apply-result 已返回下一批工作，不要轮询」。

### P3 —— 存储与资源

> 实施状态（2026-10-23）：第 16~19 项已实现——attempt 终态且 patch 落盘后自动 `worktree remove`（已接受候选与对账检出同样回收，`reclaim_worktrees` / `retain_failed_worktrees` 可调）；cleanup 不再因 ignored 文件保留已关闭 worktree；重试指纹读 `patch_hash`；账本 schema 4 分表（run_attempts / run_evidence 按行增量写入，首次写入迁移并备份）；`has_outstanding_host_work` 去掉 O(n²)、progress 用 HashMap 索引 worktree 归属。P2 遗留的 `run.forensics` 与 `host_capacity`（宿主能力声明）也已补上。


16. attempt 到 integrated/failed/superseded/revoked 且 `patch.diff + patch_hash` 已落盘后自动 `git worktree remove`（不 force，脏/锁则记录保留原因）；
    intent accepted 后删 candidate；`reconcile-*` 在 source_reconciled 后删。配置 `reclaim_worktrees`、`retain_failed_worktrees`、`clean_ignored_before_remove`。
17. cleanup 的 ignored 规则只对非终态 attempt 生效；重试指纹改读持久化的 patch_hash。
18. 账本 schema 3 → 4：runs 核心 + attempts / evidence / intents / leases / plans 分表，按行哈希增量 UPSERT；心跳从 5 MB 写入降到一行；JSON/MCP 输出保持字节一致。
19. 热路径：owner 唯一性用 leases 表；progress 用 HashMap 索引 worktree；`has_outstanding_host_work` 去掉 O(n²)。

## 5. 对现有三个仓库的立即处置

- **code-wrapper 主仓**：run-0de3acad 自 09-22 stale（心跳 31 天），账本 v2，milestone 文件已不存在。`cancel` 后 `run.cleanup`。
- **code-wrapper `.git/task-artifacts/v055-executor`（47 GB）与 share-your-terminal（399 worktree）**：在协调器空闲时
  1. `sqlite3 .git/spec-autonomous/state.db ".backup <安全位置>"`；
  2. `spec-autonomous tools call run.cleanup --input '{"run_id":"<id>","active":true}' --json` 预览；
  3. 对 retained_details 里原因为 ignored_artifacts_retained、且 attempt 已 integrated/superseded/revoked 的 worktree 执行 `git -C <worktree> clean -fdX`；
  4. 重新预览后用返回的 plan_hash 执行；`PRAGMA wal_checkpoint(TRUNCATE); VACUUM;`。
  不可删：integration worktree、有租约或 issued/claimed/receiving/submitted 的 attempt、prepared/verified 的 intent、`runs/<id>/attempts/*/result.json`、被 evidence 引用的 verify-* 目录、state.db/-wal/registry/coordinator.lock；不要 `rm -rf` 已注册的 worktree。
- P0 落地前不要再对这两个 run `prepare --run-id`，否则会再次全量重验。三个项目都把 `max_attempts`、`max_repair_rounds`、`run_timeout_seconds` 恢复默认。
