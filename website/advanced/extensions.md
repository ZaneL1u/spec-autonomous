# 扩展与集成

Spec Autonomous 的扩展点不是“把任意脚本塞进总控循环”，而是通过稳定能力和原生框架边界增加行为。

## 自定义宿主

任何宿主只要能做到以下事情，就能接入：

1. 调用本地 CLI 或 stdio MCP；
2. 为每个请求创建唯一、全新的工作上下文；
3. 让 Agent 在请求指定的 worktree 中工作；
4. 保持 token 与 owner 身份；
5. 按 `result.schema.json` 产生结构化结果；
6. 长任务发送 heartbeat；
7. 停止 Agent 后明确调用 revoke。

宿主可以使用任意模型。本项目不要求把模型身份写进 CLI 配置。

## 自定义自动化入口

团队可以在 CI、Issue bot 或内部平台中编排：

```text
inspect
  → prepare
  → discussion.next / discussion.apply
  → claim
  → 宿主 Agent
  → apply-result
  → next
```

建议通过 MCP schema 生成调用参数，不要解析 human 输出。

## 原生框架扩展

### OpenSpec

支持自定义 schema 和 tracking artifact，但适配器只会在能唯一定位任务与回写位置时自动推进。未知 glob、无 tracking file 或来源越界会明确降级，不猜测。

### Spec Kit

项目可以继续使用 constitution、preset、workflow 和 extensions。Mandatory hook 必须被执行或明确报告不支持；不会因为一段 prompt 打印了命令就当 hook 已完成。

## 能力目录

增加集成前先看现有工具：

```sh
spec-autonomous tools list --all --json --limit 200
```

常见扩展不需要碰 SQLite：

- 用 `roadmap.*` 管 milestone；
- 用 `document.*` / `frontmatter.*` 做 CAS 文档更新；
- 用 `state.*` 保存决定和阻塞；
- 用 `history.*` 构建进度界面；
- 用 `worktree.*` 审查受管理工作区；
- 用 `verify.*` 建立外部质量门。

## 新 provider

新增 SDD provider 不是只写一个目录探测器。至少需要：

- 明确的根目录与来源选择规则；
- 原生工件和任务身份；
- readiness 与严格校验；
- worker 上下文映射；
- 安全、幂等的任务状态回写；
- archive 生命周期；
- 固定版本 fixture 与真实 CLI contract test。

Provider adapter 与 Agent host 必须保持正交。

## 多团队协作

团队可以用 `workstream`、owner 元数据、phase 依赖和写集表达边界。新需求通常追加 work item 或 phase，不需要人为创建“milestone v2”。

同一仓库的多个 worktree 共享 common-dir 账本和锁。跨主机协调目前不在支持范围内；不要把本地 SQLite WAL 当作分布式队列。

## 当前不包含的扩展

- 云端 coordinator；
- HTTP daemon；
- 模型路由和 OAuth；
- TUI / Web 控制台；
- 跨仓库事务；
- 操作系统级沙箱；
- 自动发布 npm 或部署业务系统。

这些可以在稳定协议上另建产品，但不应塞回确定性 CLI 核心。
