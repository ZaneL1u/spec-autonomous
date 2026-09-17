## Context

现有 engine 已将 native planning、任务 DAG、验证、集成和恢复分开，但 invoke/execute_phase 直接运行 agent。其余 Git/SQLite/provider 代码可以复用。需求见 proposal；这里固定新的执行边界，避免双调度器。

## Goals / Non-Goals

**Goals:** CLI 是无模型调用的确定性工作流内核；相同业务能力可由终端、Skills、MCP 调用；保留细粒度工具及完整复合操作；无任何内置 agent 启动路径。

**Non-Goals:** 不实现模型 SDK、后台 agent daemon、自动探测/启动 Codex 或 Claude。MCP transport 不独立维护状态。用户自行配置的 Git/框架/验证命令属于明确操作，不是 agent launcher。

## Decisions

1. **可恢复的有限推进。** prepare/run/resume 从账本和原生工件推进到下一个需要语义工作的节点，创建不可变 work request 后返回 awaiting_host。next 只预览，重复 prepare 复用未完成请求。apply-result 在一次命令内验证身份、接收结果、执行检查/集成/CAS，再返回后续工作。宿主无需编排底层文件/Git命令。
2. **外部 work receipt。** 每个请求拥有 attempt ID、随机领取凭据、input hash、worktree/base/source revision、宿主身份/会话与心跳。CLI 不创建会话；领取/提交声明新上下文和唯一 session。结果重复提交同 hash 幂等，不同 hash 拒绝。过期或取消工作需宿主明确协调，不能仅因心跳过期就复制执行。
3. **单一状态机。** 复用现有 source/planner/audit 协议与集成事务。同步 invoke 变为请求/消费 receipt；执行阶段按 DAG 和写集预备多个独立工作包。规划批次、审核批次、修复轮数及验证 revision 在跨进程调用中持久有效。每个写命令持有 common-dir lock，等待 LLM 期间不持锁。
4. **有限默认接口。** 默认提供 inspect/progress/prepare/next/apply-result/archive/doctor。高级 tools catalog 提供 schema、读取/修改属性、参数与输出；doc/frontmatter、roadmap、task、state、worktree、history、repair 使用同一 Rust service。
5. **原生归档。** 先生成包含来源 hash、目标、变更文件与阻塞项的 dry-run 计划，执行时重新核对。OpenSpec 通过其原生 archive 命令；Spec Kit 保留 feature 内文档结构搬入显式归档目录，保存 manifest 和当前 feature 指针协调，不声称上游存在 archive 命令。活跃 work 或未完成任务阻止归档；不同阶段来源不能误归档全 milestone。
6. **结构化文档与生命周期工具。** 文档 patch 必须提供 expected hash，保留正文和非目标字段。roadmap 通过 typed model 修改并重生成视图；被人工修改的视图不覆盖。读写状态仍经账本，不暴露直接 SQL。worktree remove 保留 dirty/locked/external 工作，merge 经过显式检查。
7. **MCP。** stdio JSON-RPC 工具与资源复用 capability service；固定完整能力作为默认 tools/list，高级工具通过 catalog/显式选择可见。每个请求传结构化参数，无 shell 字符串拼装，无新 agent。输出限制、字段选择、分页、source references、unknown/partial 和版本契约统一。
8. **兼容。** 新协议在 Run 内增加带版本的 host state，旧记录仍可查询；旧自动 runner run 不重新启动。更新 skills 与 init，旧 CLI run 名称作为 prepare 兼容入口。版本提升 alpha.2。宿主能力缺失时保守串行；并发由显式配置或宿主声明限额。

## Risks / Trade-offs

- 宿主中断 → 请求和凭据落盘；心跳过期只报告 stale，显式撤销后才重新分派。
- apply-result 中断 → 复用 Git intent reconciliation，结果文件及回执 hash 防重复副作用。
- 原生文档人工修改 → source CAS 拒绝，resume 导入提交后使过期计划/审核失效。
- 细粒度工具误绕过门槛 → 状态完成、集成与归档仍调用统一业务校验。
- 验证命令修改代码 → 树和 revision 检查维持原有拒绝语义。
- 对外接口过多 → 默认完整能力，细粒度工具通过可发现目录访问；CLI/MCP 不各自实现一次。

## Migration Plan

保存旧验证记录为历史。替换配置与 skills，更新外部 mock host 驱动现有 e2e；新增无 agent 进程启动探针、跨 CLI/MCP 协议、重复回执/并发/过期/归档/文档 CAS 测试。完成实际 npm tarball 安装 smoke。旧版真实模型调用结果不作为新协议通过证据。
