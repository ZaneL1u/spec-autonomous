## 1. M1 foundations — 明确自主里程碑契约

- [ ] 1.1 定义 Milestone/SourceSnapshot/TaskSpec/Attempt/Readiness/result schema 和版本策略；验证 JSON fixtures 可往返、未知字段兼容、缺必需字段拒绝。
- [ ] 1.2 实现项目 policy、CLI inspect/plan/run/status/resume/pause/cancel 参数与退出码；验证错误选择、已有授权恢复和缺权限诊断。
- [ ] 1.3 建立单写入者 SQLite 账本、Git common-dir lock、基本 migrations 与 durable intent；验证 linked worktree 双启动被拒绝和事务回滚。
- [ ] 1.4 实现 command runner 与 deterministic fixture runner，fresh session/结果身份/日志大小/超时/本机进程树取消；验证 100 个 fixture tasks 会话唯一且主摘要有界，超时无孤儿 worker。
- [ ] 1.5 实现 M1 最小 run/attempt/repair-round 预算和无进展停止；验证真实 agent 运行前，fixture 已证明预算耗尽和重复失败能停止且 policy 恢复不丢失。

## 2. M1 OpenSpec autonomous vertical slice — 一次启动完整开发

- [ ] 2.1 实现 OpenSpec CLI capability probe、JSON inspect/snapshot 及 worker 路径映射；用 1.13.0 固定 fixtures 和真实临时 change 验证 ready/all_done/planning 不混淆、skip_specs/skipped 合法且不读写原 checkout。
- [ ] 2.2 实现单 tracking file、稳定 source identity、父子 task 覆盖与 CAS writeback；验证重排、重复文本、空 checkbox、CRLF、源漂移和未知 tracks。
- [ ] 2.3 实现 fresh planner 及保守 DAG 校验；验证所有源 task 覆盖、缺依赖/循环/越界拒绝，拆分子任务不能提前完成父任务。
- [ ] 2.4 实现 Git preflight、受管理 integration/worker worktree、实际 diff 收集和单 worker 集成；验证 dirty/unborn 仓库拒绝且不 stash/reset。
- [ ] 2.5 实现 task host verification、accepted/candidate 组合 revision 和证据索引；验证 worker exit 0 但测试失败时不解锁依赖，普通 worker 不读未通过候选。
- [ ] 2.6 串联 run --autonomous 导入→派发→验证→范围内 repair→集成→下一任务；在三任务且中间一次编译失败 fixture 中无需用户继续即可完成。
- [ ] 2.7 实现 milestone scope audit、全部旧 task 已勾时审核、最终 ff-original/branch delivery 和 report；验证全体任务完成但验收失败仍会 repair，原 checkout 改变进入 delivery_pending。
- [ ] 2.8 配置至少一种真实非交互 agent launcher 并完成小型真实仓库里程碑；记录 CLI 版本、新会话证据、最终 commit、验收和实际未验证项，不能用 fixture 冒充。

## 3. M2 reliable parallel autonomy — 并行、恢复与无进展

- [ ] 3.1 实现读写集规范化、目录/glob/公共接口/未知写集互斥与就绪队列；验证独立任务并行、同文件和未知范围串行。
- [ ] 3.2 实现 max_workers 容量、planner/verifier 共享配额、持续补位和确定性优先级；比较 1/3 workers fixture 墙钟和 dispatch trace。
- [ ] 3.3 实现每个并行写 worker 独立 worktree、依赖摘要和最新 integration base；验证第三个依赖任务只能读到前置已集成结果。
- [ ] 3.4 实现超出写集拒绝、组合冲突后的隔离 repair task 与有限重试；验证不 force merge、不以旧 revision 冒充新验证，失败候选期间独立任务仍基于 accepted_head。
- [ ] 3.5 扩展 M1 预算为并发共享计量和有限退避；验证同时完成/失败的 workers 不突破 run 上限、同 fingerprint 无进展停止、token/cost unavailable 不计为零。
- [ ] 3.6 实现完整 intent/trailer/tree-hash reconciliation；在 Git 提交前后、source writeback 前后、DB finalize 前后逐点 kill，验证 resume 不重复应用副作用。
- [ ] 3.7 实现 PID+启动身份核对、stale lease 协调、信号传播和完整进程树清理；在各声称支持的操作系统上实测超时/取消没有孤儿 worker。
- [ ] 3.8 完成源人工编辑、规则快照、超上下文拆分、ledger 升降级和受管理 worktree 保留/清理；验证不丢用户数据且恢复报告可操作。

## 4. M3 Spec Kit parity — 相同闭环驱动完整 feature

- [ ] 4.1 实现 current-directory profile 的 feature 选择、monorepo/project root、环境及 feature.json 解析；验证有空格/绝对路径/无 Git/多 feature/旧 branch profile 能力边界。
- [ ] 4.2 实现 tasks parser、长 ID、phase/story/[P]、Foundational/Polish/checkpoint 依赖；验证示例与注释不会被执行、并行候选仍受文件冲突约束。
- [ ] 4.3 实现 spec/plan/constitution/ignored 规则上下文与 worktree 路径重映射；验证 worker 不写原 feature.json、不漏必需规则、不复制 secrets。
- [ ] 4.4 实现 source CAS 回写与父子 task 聚合，复用同一 runner/scheduler/verification；完成 Spec Kit feature 的全自动 fixture 和一次真实 agent 验收。
- [ ] 4.5 实现 checklist 只读门与 hook capability/intent/result；验证 mandatory hook 不支持时阻塞，崩溃结果不明进入 hook_outcome_unknown，幂等协调不重复逻辑调用，after hook 改代码使验收失效。
- [ ] 4.6 实现有界 converge/scope audit 桥接及新任务图 revision；验证只补原始 scope、无差距时源字节不变、超 repair rounds 停止。

## 5. M4 npm alpha 与公开兼容声明

- [ ] 5.1 确定 npm 包名/scope 和首个官方 runner profile，统一 Cargo/npm 版本及兼容说明；验证名称所有权、命令示例和 schema 版本没有矛盾。
- [ ] 5.2 在六个平台运行原生构建、测试、二进制 linkage 和最低 OS 验证；将真实通过范围写入支持矩阵，未运行平台不得声称已验证。
- [ ] 5.3 对真实 release tarballs 在空环境测试 npm/Bun 安装、Node 22、--ignore-scripts、optional deps、信号与空格路径；确认上传/下载仍保留 binary 可执行权限。
- [ ] 5.4 配置受信任 npm 发布和 provenance，先平台包再 wrapper，next 安装 smoke 后提升 tag；验证失败时不会发布缺依赖 wrapper，并演练 dist-tag 回退。
- [ ] 5.5 运行整里程碑验收矩阵、记录单/并行性能、上下文上限和恢复结果；完成用户文档与 spec validation 后再归档本变更。

## 依赖与可并行开发的边界

任务默认按本节依赖和章节顺序推进；以下是本仓库实现时的并行建议，仍须检查实际写集。

| 工作包 | 前置 | 主要文件所有权 | 可并行性 |
| --- | --- | --- | --- |
| 1.1 契约 | 无 | core model/schema | 先统一接口 |
| 1.3 state | 1.1 | core/state | 与 1.4、2.1 并行 |
| 1.4 runner | 1.1 | core/runner、fixture runner | 与 state/adapter 并行 |
| 2.1–2.2 OpenSpec | 1.1 | core/adapters/openspec | adapter 内顺序 |
| 2.3 plan | 1.1、2.1 | core/plan | 与 2.4 workspace 并行 |
| 2.4 workspace | 1.1、1.3 | core/workspace | 与 planner 并行 |
| 2.5–2.8 闭环 | 1.x、2.1–2.4 | supervisor + CLI + e2e | 单 integrator 汇总 |
| 3.x 并行与恢复 | M1 验收 | scheduler/state/workspace | 共享状态修改先协调 |
| 4.1–4.3 Spec Kit 只读层 | 1.1 稳定 | core/adapters/speckit | 可与 M2 独立开发 |
| 4.4–4.6 Spec Kit 闭环 | M2、4.1–4.3 | adapter/e2e | 不另造 scheduler |
| 5.x 发行 | 对应能力验收 | scripts/packages/workflows/docs | 包装可早做，发布等矩阵 |

主 agent 维护接口、任务状态与集成，子 agent 只领限定包。共享 Cargo.toml、Cargo.lock、CLI 分派和同一 task 清单由 integrator 统一修改。每个工作包提交简短结果、测试证据与未决项，完整过程留工作目录，不注入主上下文。
