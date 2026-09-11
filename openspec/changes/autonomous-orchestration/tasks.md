## 1. M1 foundations — 明确自主里程碑契约

- [x] 1.1 定义 Milestone/RoadmapPhase/NativeAction/SourceSnapshot/TaskSpec/Attempt/ProgressSnapshot/result schema；验证 TOML/JSON 语义往返、MD provenance、未知字段兼容和缺必需字段拒绝。
- [x] 1.2 实现 policy、CLI milestone/roadmap/inspect/plan/run/progress/status/resume/pause/cancel 和 native/autonomous 模式；验证选择/参数冲突、授权继承与原生 handoff 无隐式自动执行。
- [x] 1.3 建立单 coordinator SQLite 账本、common-dir lock、TOML registry、migrations 与规划/集成 intents；验证 linked worktree 双启动被拒绝、事务回滚和中断不重复创建原生单元。
- [x] 1.4 实现 command runner 与 deterministic fixture runner，fresh session/结果身份/日志大小/超时/本机进程树取消；验证 100 个 fixture tasks 会话唯一且主摘要有界，超时无孤儿 worker。
- [x] 1.5 实现 M1 最小 run/attempt/repair-round 预算和无进展停止；验证真实 agent 运行前，fixture 已证明预算耗尽和重复失败能停止且 policy 恢复不丢失。
- [x] 1.6 编写并分发 autonomous/auto 同义入口及 milestone/progress/resume skills，实现 init/installer；验证现有 SDD 检测、宿主绑定、别名等价、同名命令冲突保留、升级 hash、卸载保留原生流程。
- [x] 1.7 实现 goal→研究/约束→完整 TOML roadmap 和 phase→原生单元映射，生成 ROADMAP.md 视图；验证多 phase 覆盖目标、单来源不能被双重管理、规划文件可提交且 runtime 仍忽略。
- [x] 1.8 实现 NativeWorkflowBridge.next_action 和阶段级 fresh planning worker；验证缺工件会调度原生规划、保留质量门、现成工件复用、决策不足才暂停。
- [x] 1.9 实现 from/to/only 范围、稳定端点与 roadmap revision 重读；验证闭区间、反向/不存在/冲突参数、范围外依赖、插入 phase 和 scope_completed 不触发 milestone lifecycle。
- [x] 1.10 实现 native↔autonomous 的 checkpoint/handoff/resume；验证返回的原生操作位置最新、用户改动被导入、过期图/证据作废且无双写。
- [x] 1.11 实现统一 human/JSON/TOML 查询和 MD frontmatter/标题/checkbox/ID 提取视图；验证格式语义一致、source hash/span/profile、unknown 字段与原 MD 字节保留。
- [x] 1.12 实现 progress --all-worktrees 的 Git inventory + ledger/registry join；验证从任一 worktree 都看到全仓、无 DB 时仍可读、外部状态 unknown、逻辑 task 去重与下一动作。
- [x] 1.13 补齐 TOML 声明/MD 原生规范/SQLite runtime 的权威边界与只读访问；验证 progress 不修改文件、不执行仓库脚本、不因坏 registry 越界读文件。

## 2. M1 OpenSpec autonomous vertical slice — 一次启动完整开发

- [x] 2.1 实现 OpenSpec capability probe、new change 与 schema-driven planning bridge、JSON/MD snapshot 和路径映射；用 1.13.0 fixtures 验证从无工件到 apply-ready、skip_specs、自定义 artifact IDs 和无原 checkout 越界写。
- [x] 2.2 实现单 tracking file、稳定 source identity、父子 task 覆盖与 CAS writeback；验证重排、重复文本、空 checkbox、CRLF、源漂移和未知 tracks。
- [x] 2.3 实现 fresh planner 及保守 DAG 校验；验证所有源 task 覆盖、缺依赖/循环/越界拒绝，拆分子任务不能提前完成父任务。
- [x] 2.4 实现 Git preflight、受管理 integration/worker worktree、实际 diff 收集和单 worker 集成；验证 dirty/unborn 仓库拒绝且不 stash/reset。
- [x] 2.5 实现 task host verification、accepted/candidate 组合 revision 和证据索引；验证 worker exit 0 但测试失败时不解锁依赖，普通 worker 不读未通过候选。
- [x] 2.6 串联目标→roadmap→原生规划→实现→repair→验证→下一 phase；用两 phase 且中间编译失败 fixture 证明无需逐任务继续，并证明已有 plans 可接入。
- [x] 2.7 实现 milestone scope audit、全部旧 task 已勾时审核、最终 ff-original/branch delivery 和 report；验证全体任务完成但验收失败仍会 repair，原 checkout 改变进入 delivery_pending。
- [x] 2.8 配置至少一种真实 agent launcher，通过本产品 skill 从目标规划并完成小型里程碑；另验 from/to 局部运行和 native→autonomous 交接，记录实际 CLI/skill/进程/代码证据。

## 3. M2 reliable parallel autonomy — 并行、恢复与无进展

- [x] 3.1 实现读写集规范化、目录/glob/公共接口/未知写集互斥与就绪队列；验证独立任务并行、同文件和未知范围串行。
- [x] 3.2 实现 max_workers 容量、planner/verifier 共享配额、持续补位和确定性优先级；比较 1/3 workers fixture 墙钟和 dispatch trace。
- [x] 3.3 实现每个并行写 worker 独立 worktree、依赖摘要和最新 integration base；验证第三个依赖任务只能读到前置已集成结果。
- [x] 3.4 实现超出写集拒绝、组合冲突后的隔离 repair task 与有限重试；验证不 force merge、不以旧 revision 冒充新验证，失败候选期间独立任务仍基于 accepted_head。
- [x] 3.5 扩展 M1 预算为并发共享计量和有限退避；验证同时完成/失败的 workers 不突破 run 上限、同 fingerprint 无进展停止、token/cost unavailable 不计为零。
- [x] 3.6 实现完整 intent/trailer/tree-hash reconciliation；在 Git 提交前后、source writeback 前后、DB finalize 前后逐点 kill，验证 resume 不重复应用副作用。
- [ ] 3.7 实现 PID+启动身份核对、stale lease 协调、信号传播和完整进程树清理；在各声称支持的操作系统上实测超时/取消没有孤儿 worker。
- [x] 3.8 完成源人工编辑、规则快照、超上下文拆分、ledger 升降级和受管理 worktree 保留/清理；验证不丢用户数据且恢复报告可操作。
- [x] 3.9 在并发创建/销毁/崩溃 workers 时验证全 worktree progress；覆盖 stale heartbeat、orphaned/prunable、不可读账本、partial snapshot、计数去重和不阻塞执行。

## 4. M3 Spec Kit parity — 相同闭环驱动完整 feature

- [x] 4.1 实现 current-directory profile 的 feature 选择、monorepo/project root、环境及 feature.json 解析；验证有空格/绝对路径/无 Git/多 feature/旧 branch profile 能力边界。
- [x] 4.2 实现 tasks parser、长 ID、phase/story/[P]、Foundational/Polish/checkpoint 依赖；验证示例与注释不会被执行、并行候选仍受文件冲突约束。
- [x] 4.3 实现 spec/plan/constitution/ignored 规则上下文与 worktree 路径重映射；验证 worker 不写原 feature.json、不漏必需规则、不复制 secrets。
- [x] 4.4 实现 source CAS 回写与父子 task 聚合，复用同一 runner/scheduler/verification；完成 Spec Kit feature 的全自动 fixture 和一次真实 agent 验收。
- [x] 4.5 实现 checklist 只读门与 hook capability/intent/result；验证 mandatory hook 不支持时阻塞，崩溃结果不明进入 hook_outcome_unknown，幂等协调不重复逻辑调用，after hook 改代码使验收失效。
- [x] 4.6 实现有界 converge/scope audit 桥接及新任务图 revision；验证只补原始 scope、无差距时源字节不变、超 repair rounds 停止。
- [x] 4.7 实现 Spec Kit 从目标到 feature 规划的 native bridge，读取安装后的模板/preset/extension 契约；完成 roadmap→specify/plan/tasks→开发 fixture，并验证缺 Python/bridge 能力时保留只读/原生 handoff。

## 5. M4 npm alpha 与公开兼容声明

- [ ] 5.1 确定 npm 包名/scope 和首个官方 runner profile，统一 Cargo/npm 版本及兼容说明；验证名称所有权、命令示例和 schema 版本没有矛盾。
- [ ] 5.2 在六个平台运行原生构建、测试、二进制 linkage 和最低 OS 验证；将真实通过范围写入支持矩阵，未运行平台不得声称已验证。
- [ ] 5.3 对真实 release tarballs 在空环境测试 npm/Bun 安装、Node 22、--ignore-scripts、optional deps、信号与空格路径；确认上传/下载仍保留 binary 可执行权限。
- [ ] 5.4 配置受信任 npm 发布和 provenance，先平台包再 wrapper，next 安装 smoke 后提升 tag；验证失败时不会发布缺依赖 wrapper，并演练 dist-tag 回退。
- [ ] 5.5 运行整里程碑验收矩阵、记录单/并行性能、上下文上限和恢复结果；完成用户文档与 spec validation 后再归档本变更。

## 本轮验收状态（2026-09-11）

36/42 项完成。证据见 [本地验收](../../../docs/validation/autonomous.md)。3.7 的 macOS PID/进程树/取消与崩溃恢复已通过，其他平台原生结果尚不可得，整项保留未勾选。M4 的 Rust/npm 版本、官方 Codex profile、本机 npm/Bun/Node 22 tarball 安装、发布脚本与 14 项离线发布测试已实现；名称所有权、远程 CI、全平台下载后的安装、npm Trusted Publisher/provenance/dist-tag 回退需要真实远程环境，因此 5.x 未整体勾选，也没有归档。

## 依赖与可并行开发的边界

任务默认按本节依赖和章节顺序推进；以下是本仓库实现时的并行建议，仍须检查实际写集。

| 工作包 | 前置 | 主要文件所有权 | 可并行性 |
| --- | --- | --- | --- |
| 1.1 契约 | 无 | core model/schema | 先统一接口 |
| 1.3 state | 1.1 | core/state | 与 1.4、2.1 并行 |
| 1.4 runner | 1.1 | core/runner、fixture runner | 与 state/adapter 并行 |
| 1.6 skills | 1.1、1.2 | packages/cli/skills、installer | 与 workflow 独立，复用 CLI 协议 |
| 1.7–1.10 roadmap/流程 | 1.1、1.3、1.4 | core/milestone、workflow | 共用状态写入由 integrator 汇总 |
| 1.11–1.13 progress/读取 | 1.1、1.3 | core/progress、结构化 views | 可与 planning bridge 并行 |
| 2.1–2.2 OpenSpec | 1.1 | core/adapters/openspec | adapter 内顺序 |
| 2.3 plan | 1.1、2.1 | core/plan | 与 2.4 workspace 并行 |
| 2.4 workspace | 1.1、1.3 | core/workspace | 与 planner 并行 |
| 2.5–2.8 闭环 | 1.x、2.1–2.4 | supervisor + CLI/skills + e2e | 证明完整目标到交付与局部范围 |
| 3.x 并行与恢复 | M1 验收 | scheduler/state/workspace | 共享状态修改先协调 |
| 4.1–4.3 Spec Kit 只读层 | 1.1 稳定 | core/adapters/speckit | 可与 M2 独立开发 |
| 4.4–4.6 Spec Kit 闭环 | M2、4.1–4.3 | adapter/e2e | 不另造 scheduler |
| 5.x 发行 | 对应能力验收 | scripts/packages/workflows/docs | 包装可早做，发布等矩阵 |

主 agent 维护接口、任务状态与集成，子 agent 只领限定包。共享 Cargo.toml、Cargo.lock、CLI 分派和同一 task 清单由 integrator 统一修改。每个工作包提交简短结果、测试证据与未决项，完整过程留工作目录，不注入主上下文。
