# Skills

Skills 是给宿主 Agent 看的操作说明。它们不包含另一套执行引擎，而是告诉宿主何时调用 CLI / MCP、如何创建 fresh context、怎样处理阻塞和回执。

初始化会根据宿主写入对应入口：

- Codex：`$autonomous`、`$auto` 等；
- Claude Code：`/autonomous`、`/auto` 等。

## `autonomous`

完整自治入口。可以：

- 从一句 goal 创建 milestone；
- 接手已有 milestone、OpenSpec change 或 Spec Kit feature；
- 保留 `from` / `to` / `only` 范围；
- 渲染 Smart Discuss 选择；
- 领取并派发工作包；
- 并行处理独立请求；
- 提交回执，持续到完成或明确阻塞；
- 在终态预览 cleanup。

它必须使用宿主现有的 Agent 能力。宿主无法创建 fresh context 时，Skill 应报告能力缺口，不能退回 CLI 内嵌 runner。

## `auto`

`autonomous` 的精确别名。两者走相同协议，不存在一套“简化但不验证”的隐藏流程。

## `milestone`

用于创建或导入里程碑：

- 把目标拆成 phase；
- 每个 phase 绑定一个原生 change / feature；
- 建立稳定 ID、依赖、verification 和可选 workstream；
- 区分 roadmap revision 与产品版本。

适合先规划、暂不执行，或向既有 milestone 追加阶段。

## `progress`

只读进度入口：

- 聚合所有 Git worktree；
- 区分原生 checkbox、host claim 和已验证进度；
- 保留 stale、unknown、partial、external 状态；
- 根据需要读取 `next`、`state.get`、`history.get`。

它不会因为用户问进度而隐式安装 provider、清理资源或重新派发任务。

## `resume`

恢复已有 run：

- 先读 `next` 和 `progress`；
- 用原 run ID 调用 `prepare`；
- reconcile 回执文件、Git intent 和账本；
- 保留原 provider、范围、已验收工作和历史证据；
- 避免接管仍 claimed / stale 的宿主会话。

旧 alpha.1 run 只读，不会重新激活历史 runner。

## 命名冲突

如果目标仓库已有同名 Skill，可以初始化时使用前缀，例如 `sa`。安装器不会覆盖用户文件；清单记录每个受管理文件的 hash，升级和卸载都先做所有权检查。

## 自定义 Skill

你可以写自己的入口，但应保持以下边界：

1. 通过 `sa_inspect` / `sa_next` 获取事实；
2. 通过 `sa_prepare` 生成工作包；
3. 宿主分配唯一 session 并 claim；
4. Agent 只在工作包指定项目中工作；
5. 结果交给 `sa_apply_result`；
6. 终态之外不要擅自声称完成；
7. 归档、cleanup 和 repair 先预览再应用。

不要在 Skill 中重新实现 Git 合并、SQLite 状态或 checkbox 回写。
