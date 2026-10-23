# OpenGSD 二次调研：针对长任务反复验证问题，哪些机制值得借鉴

调研日期：2026-10-23。配套 [long-run-churn-analysis.md](long-run-churn-analysis.md)（根因 R1~R8、方案 P0~P3）；
2026-09-10 的首轮调研见 [gsd.md](gsd.md)，当时基于 Core 1.13 / Pi 1.19，本轮重新核对当前源码。
由 5 个独立 agent 分维度完成：Core 执行工作流、Pi 自动循环与验证、Pi worktree 与合并、GSD 运维工具与 skill、
以及同一用户在 code-wrapper 项目上 GSD 时期的真实运行记录对照。只读源码，没有运行上游任何程序。本文件是草稿，未提交。

| 来源 | 版本 | commit | 位置 |
| --- | --- | --- | --- |
| open-gsd/gsd-core | 1.18.0 | 6ddd0ce | 本次 shallow clone（scratchpad，不入库） |
| open-gsd/gsd-pi | 1.22.0 | 9eb5375 | 同上 |
| 用户本机安装的 Core | 1.11.0 | — | `~/.codex/gsd-core`，code-wrapper GSD 时期使用 |

## 0. 一句话结论

GSD 两个实现都把「已验证」当作**绑定到源码内容哈希的持久事实**，续跑时按这个事实路由，从不清零；
子代理异常只触发「先对账再分类」，有产物和匹配提交就算完成；修复循环用「同一失败签名出现两次即停」封顶。
这三条正是我们 R1、R2、R4、R5 的反面。GSD 做得比我们差的地方（无隔离、共享 HEAD 的 amend 竞争、
Markdown 当真相源导致五轮 stale 漏洞修复）我们已经避开了，不必回退。

## 1. 同一项目、同一用户的真实对照（code-wrapper）

| | GSD Core 1.11（v0.5.2，最好的一次） | Spec Autonomous（v0.5.5，run-59674a11） |
| --- | --- | --- |
| 产出 | 211 个 plan、1329 次提交、7 天 | 303 次 attempt、271 小时；193 小时 implement 只有 53 小时被集成 |
| phase 验证次数 | 每 phase 1 次，50 个 phase 里只有 6 个在补缺后复验过第二次 | 每次合法完成 ≥ 3 轮全量；每次 handoff 后 10~16 次 |
| 重新规划 | 规划一次；续跑时 `has_summary: true` 的 plan 直接跳过 | MN-03 单个 phase 起了 42 次规划器，全 run 67 次 |
| 提交构成 | 640 test / 639 feat（TDD 粒度） | 397 次里 187 次是 `sa:` 账本提交，feat 14、fix 117、test 9 |
| 存储 | `.planning` 20 MB，1 个 worktree | `.git/spec-autonomous` 47 GB，243 个 worktree 条目 |
| 执行记录 | 450 个 PLAN 对应 448 个 SUMMARY，基本一对一 | 同一任务最多 15 次 implement |

用户在 GSD 时期记录的痛点（`.planning/RETROSPECTIVE.md`、Claude 记忆、codex 摘要）：共享文件导致 wave 被迫串行；
executor「报告完成但从未提交」；API 403 配额中断需要人工续跑；共享 HEAD 上 `git commit --amend` 吞掉队友提交；
状态工具与 STATE.md 互相矛盾；原生包装脚本假绿。记录的优点：plan-checker 在执行前拦住真问题；
对抗式 code review 找到安全缺陷；原生门槛保持 BLOCKED / human_needed 从不伪装通过；Smart Discuss 自动接受默认值零打扰。

## 2. 可借鉴机制（按我们的根因编号）

### R1 / R2 续跑清零与每次往返重验 → 「验证是绑定源码哈希的持久事实」

- **Core**：VERIFICATION.md 前言写 `covered_files` + `covered_digest`；读取时按内容哈希重算，不等则派生出只读的 `stale` 态，
  唯一动作是从验证门重跑 verifier。续跑进入 execute-phase 时按 `verification.status` 路由：`passed` 且 roadmap 已勾 → 退出；
  `passed` 未勾 → 只更新 roadmap，「不重做已经跑过的门」；`stale` / `missing` → 跳过全部 wave 直接到验证门。
  写方只能写 `passed | gaps_found | human_needed`，`stale | missing` 是只读派生态，词汇封闭。
  1.15/1.16 连修了四轮「永久 stale」漏洞，原因是 digest 把 ROADMAP、报告自身等全局文件算进去了。
- **Pi**：`tested_source_revision` = 对 `git ls-files --cached --others --exclude-standard` 全部路径的模式+内容做 sha256，
  检查前后各取一次，不一致即 `inconclusive`。重入时若存在 `pass` 结论且当前哈希相等，直接返回 `continue`，一条命令都不跑。
  可选的 exec-run 复用按「命令契约摘要 + 源码版本 + 输出哈希」复用单条命令结果。
  中断在 verify 阶段的派发，重启后**从 verify 续**而不是从 execute 重来。
- **映射**：`engine/lifecycle.rs` `drive()` 内层循环在 `verify(phase)` 前查 `(phase, integration tree hash, checks hash)` 的 PASS 记录，
  命中则直接进入取审计结果；`engine/recovery.rs` 的 `reconcile_source` 与 handed_off 分支不再 `completed_phases.clear()`，
  改为按 phase 标 `needs_reverify`；`reopen()` 去掉 depends_on 级联。digest 要排除 milestone.toml、ROADMAP、`.spec-autonomous/plans`。
  这是 P0-1 和 P0-4 的具体形态，我们用 git tree hash 比 GSD 的文件级 digest 更可靠，不要照抄 Markdown 前言。

### R5 在途工作被丢弃 → 「先对账，再分类」

- **Core** `completion-reconciliation.md`：子代理异常结束（包括编排器自己被打断）不算失败；SUMMARY 存在**且**匹配到提交 → 完成、不重派；
  只有其一 → 不算完成；探针不可用 → 停下问人，不读成「没有提交」。`safe_resume_gate`：有提交无 SUMMARY 时给三个选项，绝不盲目重派。
  between-wave reset：executor 基底与 `EXPECTED_BASE` 不符则 exit 42，编排器**降级为串行**而不是报废工作。
- **Pi**：崩溃单元先做快照提交，attempt 标 `interrupted` 并链接到新 attempt（lineage），从记录的阶段续；
  用户中途改规划产物只会「暂停派发」直到重新渲染，不会撤销在途单元。
- **映射**：`engine.rs` `check()` 的 `source_drift` 不再中止，记录后延到本批 receipt 提交；`recovery.rs:226-239` 对 issued/claimed/receiving attempt
  先看有没有 result.json / patch 证据，有就按 input_hash 判是否过期，没有才 revoke。对应 P0-5。

### R4 修复 / 收敛 / 审计循环 → 确定性上界

- **Core**：自动 gap closure 默认只做 **1 轮**（plan --gaps → execute → 复验，仍有 gap 则 defer 并停）；同一 blocker 修 3 次失败 → `needs_human`；
  plan-checker 修订 3 轮且问题数不下降就提前升级；里程碑审计只在全部 phase 完成后做一次，`--only/--to` 不触发；
  deferred phase 写进 STATE，再入时跳过不重验；gap 修复计划顺序追加编号，不删原计划集。
- **Pi**：ADR-047 活性后备：每个「没有推进」的结果按 `(scope, guard, unit, sha256(input))` 计数，≥ 2 即判卡死，持久化到 DB、跨重启有效，
  退出码 10，需显式 `--resume-wedge` 确认；验证重试 `command#exit` 签名重复 → 暂停；成本飙升到滚动均值 3 倍 → 暂停。
- **映射**：`lifecycle.rs` `repair_audit` 把全局 `repair_rounds` 换成失败签名计数；`execution.rs` `converge` 不再 `plans.remove`，
  追加任务到现有计划；审计结果按 `(phase, source_hash, tree hash, 验收 id 批次)` 缓存；增加 `deferred_phases`。对应 P0-3、P0-6、P1-10。

### R3 多阶段重复验证 → 「每层一次门」

- **Core** 的层级：plan 级由 executor 自己跑并写进 SUMMARY `coverage`；wave 合并后一次 build+test 门；phase 结束**一次** verifier（含只跑前序 phase 测试文件的回归门）；
  里程碑一次审计。没有 worker/combined 双跑，也没有 post-hook 重跑。
- **映射**：worker 与 combined 同树时复用、post-hook 只在 hooks 改了 HEAD 时跑、`Check.scope` 拆分 task/phase/milestone。对应 P0-1、P0-2、P1-7。

### R7 / R8 资源与账本 → 「状态在数据库，worktree 只是投影」

- **Pi**：每个 milestone 一个 worktree，并行最多 `max_workers`（默认 2，上限 4）；分支已存在时**复用不重置**，仅在严格祖先且未被检出时 fast-forward。
  脏 worktree 改名进 `.gsd/quarantine/` 隔离而不是阻塞清理；分支只在目录删除成功后才删；清理结果分 `removed / preserved / partial`。
  合并写「结算回执」到 DB，重跑时识别「已合并」不再合第二次；用户 reset 掉合并提交时保留分支直到工作回到集成分支。
  账本完全规范化：每个 attempt 一行、预算/重试/阶段为卫星表、心跳只 `UPDATE workers SET last_heartbeat_at`。
  `/gsd doctor` 有 `orphaned_running_attempt`、`worktree_stale/dirty/unpushed`、`snapshot_ref_bloat`、`activity_log_bloat` 等编码诊断。
- **映射**：attempt 终态且 patch 落盘后自动 `git worktree remove`，脏则隔离；`deliver()` 记录结算回执；账本 schema 3→4 分表、增量 UPSERT；
  doctor 增加编码规则。对应 P3-16~19、P2 doctor。

### 宿主协议与 skill → 「skill 是路由，完成由状态判定」

- **Core** 的 skill 只有 47~70 行：`<objective>` + `<execution_context>`（@ 引用 workflow）+ `<process>`，逻辑在 workflow 里按 `section_manifest` 条件加载。
  一次 `init` 查询拿到全部存在性字段后分支，「不重复取已有的状态」。几条可直接改写进我们 skill 的句子：
  「Completion is decided by artifact reconciliation (SUMMARY + matching commits), not by the session's terminal state」；
  「Never block indefinitely waiting for a signal — always verify via filesystem and git state」；
  「A working executor is never steered」；「A blocker that survives 3 fix attempts is an operator gate, not an executable gap」；
  「Skip plans where has_summary: true」。
- 宿主能力按 `capabilities/{claude,codex,cursor}/capability.json` 声明（background、nested、isolation、maxConcurrency），
  派发点通过类型化查询 `dispatch-should-flatten` 决定，fail-closed，「never branch a dispatch site on a runtime id」。
- Smart Discuss：CONTEXT 已存在就跳过；基础设施型 phase 整体跳过；一张表 3~4 个灰区、预选答案、「Accept all」在最前；
  `auto_select` 的决策自动选，`gate="blocking-human"` 永不绕过；`human_verify_mode=end-of-phase` 把人工检查批量到 phase 末尾，
  因为「每次中途停下都要付一次 executor 冷启动」。
- 运维：`health` 31 条编码规则带 `Fix:` 与 `repairable` 标记；`forensics` 只读取证（同文件连续 3 次提交、>2 小时无进展、产物缺失）；
  `progress --forensic` 7 项完整性检查；`pause-work` 写 HANDOFF.json，`uncommitted_files` 「measured, never asserted」。
- **映射**：skill 重写为 ≤ 60 行路由 + `docs/host-protocol.md`；doctor 每会话一次；prepare / apply-result 返回下一批工作就不轮询；
  `discussion.next` 增加 skip-when-resolved、`auto_select`、`blocking_human`、随 prepare 返回；新增只读 `run.forensics`；
  `init --agent` 写入宿主能力声明。对应 P2-11~15。

## 3. 明确不照搬的部分

- Markdown / `.planning` 作为运行态真相源与 mtime 或手工前言——我们有 SQLite 账本，用 tree hash；GSD 为此修了 #2348 / #4155 / #4623 / #4817 / #5095 五轮漏洞。
- 共享 HEAD、每 milestone 一个 worktree 的无隔离并行——用户自己记录了 amend 竞争和 main 被污染；我们保留 per-attempt 隔离，改为按 worker 槽位复用。
- 文件重叠只告警（Pi parallel-eligibility）——保留 AGENTS.md 的「未知写集串行」。
- 自动嗅探语言和构建命令（Core post-merge-gate）——与 preflight 显式 argv 原则冲突，会制造 R6 类畸形命令。
- 几乎所有失败都 AskUserQuestion 的对话门——我们需要确定性预算与终态，而不是把循环上界交给提问。
- Pi 的会话/turn/wakeup 机制、milestone 租约与 PID fencing（我们是单协调器）、40 张表的权威切换 schema、投影渲染、Telegram/dashboard。
- 把用户根目录 stash 起来再自动提交 worker 脏文件进集成提交——违反「exit zero ≠ integrated」。
- `rmSync` / 直接删 `.git/worktrees/<name>` 的兜底，以及 `--repair` 重新生成 STATE。

## 4. 对 P0~P3 优先级的修正

调研没有推翻 churn 分析的排序，但补了三点：

1. **P0-1 的缓存键应采用 Pi 的「前后两次整树哈希一致」**而不是只记一次 HEAD，否则检查过程中被 hook 改写的树会留下假 PASS。
2. **P0-6 的失败签名计数应持久化并跨重启生效**（Pi ADR-047），否则宿主一次 resume 就把计数清零。
3. **新增 P2 项**：宿主能力声明文件 + skill 路由化 + `run.forensics`。GSD 时期用户能在一两天内靠 health / forensics 发现卡死，
   而我们的 run 跑了 11 天才被人工发现，差距主要在这里。
