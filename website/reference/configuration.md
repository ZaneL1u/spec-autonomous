# 配置

项目配置位于 `.spec-autonomous/config.toml`。初始化会创建安全默认值；不认识的字段或拼错的关键字段会报错，不会静默变成无限预算。

## 最小示例

```toml
schema_version = 2
verification = [
  { argv = ["cargo", "test", "--workspace", "--locked"], cwd = "." }
]

[execution]
max_workers = 3
max_attempts = 3
attempt_timeout_seconds = 1800
run_timeout_seconds = 28800
max_repair_rounds = 2

[host]
max_concurrency = 3
lease_seconds = 1800
```

## `execution`

| 配置 | 含义 |
| --- | --- |
| `max_workers` | CLI 允许同时处于执行中的任务上限 |
| `max_attempts` | 每项工作最多尝试次数 |
| `attempt_timeout_seconds` | 单个工作包的时间预算 |
| `run_timeout_seconds` | 整个 run 的时间预算 |
| `max_repair_rounds` | 验证失败后允许的有界修复轮数 |
| `max_context_bytes` | 内联工作包上下文上限，超出时改用 hash 引用 |
| `max_source_bytes` | 原生来源快照预算 |
| `max_planner_tasks` | planner 单批原生任务上限 |
| `delivery` | `ff-original` 或保留交付分支 |

实际并发取 `execution.max_workers` 与 `host.max_concurrency` 的较小值，还会受 DAG 和写集冲突限制。

## `host`

`max_concurrency` 表示宿主确认能同时管理多少个 fresh session。没有声明时保守串行。

`lease_seconds` 用于判断工作包心跳是否过期。过期只代表状态未知，不代表 Agent 已停止，也不会自动转让所有权。

## 验证

验证命令必须使用 argv 数组：

```toml
verification = [
  { argv = ["bun", "run", "test"], cwd = "." },
  { argv = ["cargo", "clippy", "--workspace", "--locked"], cwd = "." }
]
```

不要把命令写成需要 shell 解析的一整段字符串。这样空格、引号和用户输入不会改变参数边界。

phase 和 milestone 还可以声明自己的 verification。任务计划也可有更小范围的检查。

## 环境变量

项目级 provider 和验证环境可以单独配置。敏感值不要提交到仓库；使用 CI secret 或宿主运行环境注入。

结构化输出会把环境内容标记为 redacted，不回显密钥。

## 旧 `runner`

历史 alpha.1 配置可能包含：

```toml
[runner]
profile = "command"
command = ["some-agent"]
```

当前版本不会执行它。Doctor 会报告旧字段，历史账本保持只读兼容，但不会重新激活 CLI agent launcher。

## 配置层级

有效配置按默认值、用户配置、项目配置合并。数组按更具体层完整替换，不做难以预测的拼接。

继续已有 run 时默认使用创建时保存的有效配置。只有明确使用 reload 选项，才会把新配置带入允许变化的续跑边界。
