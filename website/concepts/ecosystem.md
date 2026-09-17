# OpenSpec、Spec Kit 与本项目

这三个项目不是互相替代的同类产品。

## OpenSpec

OpenSpec 是规范与变更生命周期工具。它擅长：

- 用 proposal、specs、design、tasks 描述一个 change；
- 通过 schema 组织工件依赖；
- 给 Agent 提供原生 instructions；
- 校验规范；
- 在完成后归档 change，并同步主规范。

它回答的是：**这个变更要做什么，相关工件是否齐全，原生任务是什么。**

OpenSpec 的任务 checkbox 和工件状态不是代码已经测试、集成的证明。

## Spec Kit

GitHub Spec Kit 是一套规范驱动工作流。它擅长：

- constitution、spec、plan、tasks 等完整规划路径；
- feature 级目录和用户故事；
- 原生 workflow、hooks、preset 与扩展；
- 标记任务阶段、story 和 `[P]` 并行候选；
- 通过不同 coding agent integration 执行官方流程。

它回答的是：**一个 feature 如何从需求走到计划和实现工作流。**

Spec Kit 已经有自动化和 resume，不能把它描述成“只有 Markdown”。但其原生流程不等于本项目的任务级 worktree、统一回执、跨 provider 账本和 Git 集成门。

## Spec Autonomous

本项目位于两者上层：

- 读取并尊重它们的原生工件；
- 把一个或多个 change / feature 放进 milestone roadmap；
- 生成有来源映射的执行任务图；
- 为宿主提供 fresh-context 工作包；
- 管理 worktree、并发冲突、回执、验证、恢复和集成；
- 验证成功后再回写原生任务；
- 用同一套 CLI / MCP 协议支持两种 provider。

它回答的是：**这些规范如何可靠地持续交付。**

## 选择关系

| 你的情况 | 应该怎么选 |
| --- | --- |
| 只想写和管理 OpenSpec change | 使用 OpenSpec |
| 喜欢 Spec Kit 的 feature、constitution 和官方工作流 | 使用 Spec Kit |
| 想让多个阶段或 Agent 可验证、可恢复地交付 | 在 OpenSpec / Spec Kit 之上加 Spec Autonomous |
| 想换掉现有规范格式 | 本项目不是为此设计的 |

## 一个具体例子

“团队邀请”在 OpenSpec 中可能是 `openspec/changes/team-invites`，在 Spec Kit 中可能是 `specs/003-team-invites`。

Spec Autonomous 不复制一份 `team-invites.sa-spec`。它只记录：

- phase 指向哪个原生目录；
- 当时的源 hash；
- 原生任务如何映射到执行任务；
- 哪些 attempt 做过什么；
- 哪个 Git revision 通过了哪些验证；
- 哪些原生 checkbox 可以安全回写。

这样团队仍可继续使用自己熟悉的 SDD 工具。
