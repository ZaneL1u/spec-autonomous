## Why

长时间运行积累数百工作树，验收产物路径被写死导致反复修订，失败日志和跨仓库进度不完整。需要在保留独立验收、所有权与历史证据的前提下减少重复编排，并为现有 run 提供安全升级路径。

## What Changes

- 显式允许运行中清理已关闭资源，保留活跃所有者、恢复所需工作树和证据；预览与应用受锁及哈希保护。
- 验证波次自动分配隔离的产物目录、环境变量和可选准备钩子，完整记录失败阶段、耗时及日志引用；合并检查去重。
- 进度显示未进入 run 的原生里程碑、重试/验证开销、工作树压力，允许显式关联独立执行仓库。
- doctor 校验 skills 内容漂移；提供保留原 run 的分阶段升级说明与本机包验证。
- 修复完整套件发现的 Homebrew npm 符号链接查找问题，安装与打包共用安全的 argv 入口。
- 复核现有 dogfood-scope-and-closeout 未提交修复，补充回归测试。

## Capabilities

### New Capabilities
- `long-run-maintenance`: 安全回收、隔离验证产物、失败诊断与跨仓进度。

### Modified Capabilities
- 无。

## Impact

Rust cleanup、verification、progress、doctor 与能力 schema，技能、中文文档和真实 Git/进程测试。不升级或修改正在运行的消费项目，不替换全局 CLI 或 MCP，不启动模型会话。
