## Why

CLI 的职责是为 LLM、Skills 和宿主提供确定性的能力，而非启动 agent 或持有模型会话。当前 run 内嵌模型执行循环，需要改为由宿主接收工作包、执行语义工作并交回结果，同时保留原生 SDD 和已经实现的验证、集成、恢复能力。

## What Changes

- **BREAKING** 移除 CLI 的 agent launcher；run/resume 改为有限推进并返回 awaiting_host 工作包，旧式 runner 配置只用于历史记录读取与迁移诊断。
- 提供少量完整能力 inspect/progress/prepare/next/apply-result/archive/doctor，并通过同一能力注册表提供细粒度工具。
- 宿主工作协议包括不可变上下文、领取凭据、会话身份、心跳、结果 CAS、重复提交幂等、取消与失效协调；独立宿主自行调用原有子 agent 能力。
- 保留结构化 Markdown、TOML roadmap、SQLite 事务、DAG、写集互斥、验证、候选隔离、原生回写和范围完成语义。
- 提供文档/frontmatter、roadmap、任务/状态、worktree、历史摘要、健康检查与修复，以及原生归档预览/执行工具。
- 提供 stdio MCP 工具和资源接口；默认公开完整能力，高级入口查询并使用细粒度工具；CLI/MCP 复用相同实现。
- 更新 skills、mock 宿主、单测/e2e、迁移和 npm 安装验收；按平台分发机制保留。

## Capabilities

### New Capabilities

- `host-work-protocol`: 无模型调用的有界推进、宿主工作请求与结果生命周期。
- `capability-tools`: 结构化查询、复合操作和细粒度工具共享稳定契约。
- `native-lifecycle`: 原生归档、worktree 生命周期、健康检查与显式修复。
- `host-interface`: CLI/MCP 接入、有限公开、skills 与向后迁移。

### Modified Capabilities

无。既有 repository-detection 与 native-cli-distribution 主规范继续满足。活动 autonomous-orchestration 方案中的 CLI agent 启动设计被本变更明确替代。

## Impact

影响 core engine/model/config/runner/progress/state、CLI 分派、skills、mock driver 和测试。新的 host 工作协议是 alpha 破坏性变更；历史 run 仍只读可查，不重新激活旧 launcher。子进程仅用于明确的 Git、原生框架与验证操作。用户已经授权本次规划与实现一起完成。
