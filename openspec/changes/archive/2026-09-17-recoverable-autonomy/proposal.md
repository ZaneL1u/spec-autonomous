## Why

真实 wordcount 试跑因验证命令错误中断，已经通过的任务无法在修正后继续；过细规划、重复修复和残留工作树也放大了成本。需要在保留原生规范、已验证成果及恢复门禁的前提下补齐修订、预检、清理和批量操作。

## What Changes

- 添加 run.revise 预览/哈希应用，允许修正当前 run 待完成任务、阶段、里程碑的验证命令，保留已接受提交、检查点与失败证据。
- 明确拒绝 prepare(run_id,plan) 的静默忽略；同一 run 的修订路径可发现。
- 验证命令环境/路径预检、未来文件 deferred 与 Node 目录参数警告；失败计划可以返回 fresh planner 修正。
- 区分错误验证配置与实现失败；无修改且没有声明写集的失败验证要求修订，避免无效重试。
- 规划强调完整交付单元与多 source_ids 覆盖，添加原子 work.claim-batch；改善终态清理与保留原因。

## Capabilities

### New Capabilities
- `recoverable-autonomy`: 运行中验证修订、预检、批量认领与可审阅清理。

### Modified Capabilities
无。

## Impact

Rust 协调器/账本/能力 schema、CLI 与 MCP 公共服务、Skills、共享翻译、真实 Git/进程/Node 回归；alpha.9 私有发行。
