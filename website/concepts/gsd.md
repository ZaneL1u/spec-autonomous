# 什么是 GSD

这里说的 GSD 是 [Open GSD](https://opengsd.net/) 体系。名字来自 “Get Stuff Done”，目标是让 coding agent 不只回答一次问题，而是按阶段持续推进工作。

Open GSD 有两条容易混淆的路线：

- **GSD Core**：嵌入现有宿主 Agent 的 prompt / Skill 工作流；
- **GSD Pi**：包含自己 host、会话和执行循环的独立 harness。

## Spec Autonomous 借鉴了什么

- 用 milestone 和 phase 管理长任务；
- 主上下文只保留状态与摘要；
- 子任务使用新的、边界清晰的上下文；
- 根据依赖和写入冲突分批执行；
- 每轮后重新读取状态，而不是一次生成固定队列；
- 完成后验证，中断后先 reconcile 再 retry；
- 检测“同样输入、同样失败、没有进展”并停止。

## 最关键的区别

| 维度 | GSD Core | GSD Pi | Spec Autonomous |
| --- | --- | --- | --- |
| 产品形态 | Skills / prompts | 独立 Agent harness | 无模型的 CLI / MCP 能力层 |
| 规范体系 | 自有 `.planning` 工作流 | 自有状态与运行时 | 继续使用 OpenSpec / Spec Kit |
| 谁创建 Agent | 宿主 | Pi host | Codex / Claude 等外部宿主 |
| CLI 是否启动模型 | 不适用 | 会管理会话 | **不会** |
| 运行状态 | 主要由工作流工件维护 | 自有 SQLite/runtime | Git common-dir SQLite + 原生 Markdown |
| 交付门 | GSD 自己的 review/verify 流程 | host verification | 回执、实际 diff、Git、验证、CAS 共同成立 |
| 适合谁 | 想在现有 Agent 中采用 GSD 方法 | 想使用完整 GSD 运行时 | 已用 OpenSpec / Spec Kit，想补可靠交付 |

## 为什么不直接复制 GSD

直接再引入 `.planning/`，会让项目同时拥有：

1. OpenSpec / Spec Kit 规范；
2. GSD 规划工件；
3. Spec Autonomous 运行数据。

三套事实源会互相漂移。本项目选择只吸收可靠机制，不复制整套命名和模板。

## “轻量”不是少做验证

本项目的轻量来自：

- 不内置模型和认证；
- 不做 TUI、云同步、浏览器 Agent 或向量记忆；
- 用小而稳定的工作包 / 回执协议连接宿主；
- 把确定性能力集中在 Rust 核心。

它没有通过删掉恢复、幂等、取消和验证来变轻。恰恰相反，这些是长任务真正可靠所必需的部分。

::: info 调研边界
仓库中的 GSD 对比基于固定源码快照和静态研究，不代表对所有上游版本的实时评价。详细证据见仓库的 `docs/research/gsd.md`。
:::
