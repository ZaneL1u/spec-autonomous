# 快速开始

下面走最短路径：安装、初始化、从已有规范开始运行。

## 1. 安装

当前推荐从 GitHub 仓库安装。机器需要 Node.js 22.13+（或 23.5+）、Git，并已登录 GitHub CLI：

```sh
npm install -g git+https://github.com/ZaneL1u/spec-autonomous.git
spec-autonomous --version
```

第一次执行会通过 `gh` 下载并校验当前平台的原生二进制，后续直接使用缓存。当前实际发布验证范围是 macOS arm64。

## 2. 初始化项目

进入已经使用 OpenSpec 或 Spec Kit 的 Git 仓库：

```sh
cd your-project
spec-autonomous init
```

交互界面会询问：

- 使用 OpenSpec 还是 Spec Kit；
- 宿主是 Codex 还是 Claude Code；
- 是否写入项目 MCP 配置。

自动化环境建议明确传参：

```sh
spec-autonomous init --provider openspec --agent codex --mcp --non-interactive
```

初始化不会替换你已有的 SDD 框架，也不会覆盖别人的 MCP 配置。

## 3. 从现有规范开始

OpenSpec change：

```sh
spec-autonomous prepare --change add-team-invites --json
```

Spec Kit feature：

```sh
spec-autonomous prepare --feature specs/001-team-invites --json
```

返回 `awaiting_host` 是正常的，意思是“工作包准备好了，等宿主派一个新上下文来做”，不是任务已完成。

## 4. 在宿主中使用 Skill

通常不需要手写 claim 和回执。初始化后直接在宿主中调用：

```text
/autonomous
```

或：

```text
/auto
```

Codex 风格宿主使用 `$autonomous` / `$auto`。Skill 会读取 CLI 返回的工作包，调用宿主自己的 Agent 能力，然后把结果交回 CLI 验证。

## 5. 查看进度

```sh
spec-autonomous progress --all-worktrees --json
spec-autonomous next --run-id <run-id> --json
```

你真正要关注的是：

- `awaiting_host`：还有宿主工作；
- `needs_input`：需要选择或修正；
- `scope_completed`：指定阶段范围完成；
- `completed`：整个里程碑完成；
- `delivery_pending`：候选已验证，但目标分支变化，需要处理交付。

## 6. 中断后继续

```sh
spec-autonomous prepare --run-id <run-id> --json
```

或者在宿主里调用 `resume` Skill。恢复会先核对持久化状态和 Git，不会直接把旧副作用重做一遍。

## 接下来

- 还没有规范：看[完整工作流](./workflow)。
- 想选安装方式：看[安装方式](./install)。
- 想了解为什么 CLI 不创建 Agent：看[核心理念](../concepts/principles)。
