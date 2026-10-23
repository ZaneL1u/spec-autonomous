# 长运行开销与 GSD 对照

本次隔离修复以本机 alpha.14 源码为基线；未升级全局 CLI/MCP，未修改 money-printer 的账本、配置、工作树或正在执行的命令。消费项目脚本修复另附未应用补丁。

## 66 个目录从哪里来

2026-10-16 北京时间 00:06 左右，对 money-printer 全量分页的只读快照：

| 类型 | 数量 | 原因 |
| --- | ---: | --- |
| 规划、审计、实施尝试 | 47 | 每次语义请求都建新树；包括重试、重规划、复审 |
| 集成候选 | 15 | 即使串行且基线没变，也从 accepted_head 再建一个同内容候选 |
| 外部代码合并协调候选 | 2 | 主分支中途合入另一批功能，其中一次冲突 |
| 运行集成树 | 1 | 保存本次 run 已接受版本 |
| 用户主工作区 | 1 | 用户原始检出 |
| 合计 | 66 | 累计保留目录，并非 66 个并发 worker |

50 是历史工作尝试数，与现存 47 个尝试目录不同。640 是工具记录的验证检查数，累计约 44 分钟；构建、表单、严格规格和 diff 检查各执行 69 次。它不包含所有 agent 内部测试，也不能解释全部约 9 小时墙钟耗时。主分支合并导致五个文件冲突、数据库迁移编号冲突，属于真实返工；单 worker 和串行依赖属于配置/计划，不是简单调高并发就能消除。

## 和本机 GSD 的比较

本次直接读取本机安装的 GSD 主流程与被引用步骤；并非声称网上所有版本都相同。

| 方面 | 本机 GSD | 旧 Spec Autonomous | 此次实现 |
| --- | --- | --- | --- |
| 执行粒度 | 一个 plan 包含多个 tasks，按 wave 推进 | 支持任务 DAG，但每个语义请求独立目录 | 保留 DAG；提示 coherent deliverable，复用关闭的只读检出 |
| 执行后检查 | SUMMARY/提交/文件 spot-check，随后 post-merge gate | worker 与同内容 candidate 各跑一套门禁 | 真正集成 candidate 上运行一次完整权威门禁 |
| 阶段验收 | executor 自检、阶段 verifier；不是没有独立验收 | 每次审计交接重新进入 phase gate，无 hook 也跑 post-hook | 完整同输入证据可复用；真实 hook 后仍重验 |
| UAT | structured coverage 中全 PASS 的自动化条目可自动通过；人工判断另呈现 | native acceptance 引用分批独立审计 | 保留独立语义审计，以已有完整执行证据为依据，不要求重复整套测试 |
| 缺口修复 | gap_id 关联修复计划，--gaps-only，已解决缺口不重复诊断 | append repair tasks；外部代码改动常清空全部阶段完成记录 | 声明读写范围命中的阶段与依赖闭包失效；未知覆盖仍全审 |
| 工作树清理 | manifest 记录，wave 后合并/清理 | 独立显式 cleanup，accepted 目录偏向保留 | handoff 自动安全回收，保留分支/证据；可关闭 |

GSD 本身也有多层验证与复杂恢复，并非零开销。部分本机步骤含 force 清理示例，本次没有照搬；Spec Autonomous 始终保留 dirty、locked、活跃 owner、ignored 产物和未知依赖。

本机依据：

- `/Users/zaneliu/.codex/gsd-core/workflows/execute-phase.md`：wave、spot-check、cleanup-wave、post-merge build/test、verify_phase_goal。
- `/Users/zaneliu/.codex/gsd-core/workflows/execute-phase/steps/executor-isolation-dispatch.md`：按 manifest 的隔离与生命周期。
- `/Users/zaneliu/.codex/gsd-core/workflows/execute-plan.md`：完整 plan 自检与结构化 coverage。
- `/Users/zaneliu/.codex/gsd-core/workflows/verify-work.md`：coverage-aware UAT、gap_id、已解决缺口协调。

## 实现与边界

1. plan-tasks/audit 可复用已关闭、安全的只读检出。它仍是 fresh-context 请求，token/session/不可变 input/result 全部独立。过去的文件版本通过 base_commit 和记录的上下文读取，不能用当前目录内容冒充历史证据。
2. 串行、base_commit==accepted_head 的 worker 本身作为集成候选；并发造成基线改变时仍建隔离合并候选。删除的是重复 worker gate，实际 combined gate、原生 checkbox CAS 与 durable intent 仍保留。
3. 验证成功后保存完整波次收据；复用要求同 checkout、提交、stage、检查列表、配置、继承环境、验证可执行文件及协调器二进制内容，并校验全部波次证据文件。源码、环境、checks、工具或证据改变就重新执行。不复用失败或未完成波次。`execution.reuse_verification=false` 用于外部服务、会变化的夹具/依赖或需要每次实测的场景；Git 无法描述这些外部状态。
4. 只有实际执行 after_implement hook 后才跑新的 post-hook gate；这次 gate 不走复用。
5. 外部代码变动按 task.reads/writes 与阶段依赖判断。声明不完整、未知文件、全局配置变化都保守失效；不能承诺每次主分支合并只查一个文件。
6. `execution.automatic_cleanup=true` 默认在协调器持锁交接时回收安全关闭目录。native-planning/converge 的产物先保存候选提交，分支仍保留。ignored 产物不会自动删除；maintenance.json 记录实际移除和保留原因。`false` 可保留目录取证。
7. 资源提示预算为 `1 + 2×活跃run数 + 2×宿主有效容量 + 2`。单 run/单 worker 是 7 个。它是本产品的积压提示策略，不是 Git 行业标准，也不是强制删除上限。顺序流程结束时通常仅剩主工作区和 integration。

## 项目脚本问题

历史 P04 的 37 个浏览器套件均 PASS，但总入口仍 FAIL：

`AssertionError: prior wave modified: check-984ddef197af45ba8f05d82d31fe5bcd/stderr.log`

项目递归哈希了整个继承目录，误把协调器正在追加的 stderr.log 当作历史波次。框架原先又把报告目录与进程诊断放在同一个根目录，这两个问题叠加导致误报。

框架现在给消费者独立 artifacts 目录，进程日志在其外；准备钩子与检查共享这个 artifacts 目录。项目侧补丁 [money-printer-panel-acceptance.patch](patches/money-printer-panel-acceptance.patch) 仅保护本 harness 已结束的 manifest 波次。回归实际执行补丁中的函数，验证活跃日志追加不会误报、旧证据改变仍可检测。该补丁没有应用到消费项目。

## 验证

准确结果在同目录的 runtime-overhead-validation.md。`scripts/benchmark-overhead.mts` 可在全新临时仓库对照两个显式指定的二进制；它使用确定性测试 agent，但真实 Git、Node 断言、账本、集成和回收均走生产代码。不可据此推算真实模型耗时减少比例。


本次优化纳入 alpha.15 发行；交付以发布 tag、提交和二进制 SHA256 区分。新字段写入 run 后，旧版严格配置解析器不能读取该记录。后续切换需先备份、固定唯一写入者，不能仅凭版本号相同混用新旧程序。本次没有进行该切换。
