# MCP

Spec Autonomous 提供本地 stdio MCP server。它和 CLI 使用同一个 Rust 能力核心，不通过网络监听端口，也不请求 MCP sampling。

## 启动

```sh
spec-autonomous --path /path/to/project mcp
```

通常不需要手写配置。运行：

```sh
spec-autonomous init --agent codex --mcp
```

安装器会合并项目 MCP 配置，保留已有 server 和用户设置。

## 默认工具

Rust MCP 默认公开 8 个完整能力和一个高级 dispatcher：

| MCP 工具 | 用途 |
| --- | --- |
| `sa_inspect` | 读取 provider、原生工件和仓库状态 |
| `sa_progress` | 聚合所有 worktree 的可信进度 |
| `sa_prepare` | 准备工作包并推进 run |
| `sa_discussion_next` | 读取阶段灰区卡片 |
| `sa_next` | 只读预览下一动作 |
| `sa_apply_result` | 接收、验证并集成宿主回执 |
| `sa_archive` | 预览或执行 hash 绑定的归档 |
| `sa_doctor` | 检查配置、账本与恢复诊断 |
| `sa_tools` | 发现或调用细粒度能力 |

npm JS 层还会加入 `sa_providers`，用于查询或安装 OpenSpec / Spec Kit，因此通过正式 launcher 启动时默认共 10 个工具。

## `sa_tools`

列出目录：

```json
{
  "operation": "list"
}
```

调用细粒度能力：

```json
{
  "operation": "call",
  "capability": "state.get",
  "arguments": {
    "run_id": "run-..."
  }
}
```

只有明确使用 `mcp --all-tools` 时，细粒度能力才会全部展开为独立 MCP tools。默认保持工具列表小而稳定。

## 常用细粒度能力

- 工作协议：`work.claim`、`work.claim-batch`、`work.heartbeat`、`work.revoke`、`work.context`
- 讨论：`discussion.apply`
- 路线图：`roadmap.get`、`roadmap.select`、`roadmap.add`、`roadmap.insert`
- 状态：`state.get`、`state.decision`、`state.block`、`history.get`
- 任务：`task.list`、`task.ready`、`task.complete`
- 验证：`verify.source`、`verify.plan`、`verify.references`、`audit.open`
- Git / worktree：`git.inspect`、`git.commit`、`worktree.list`、`worktree.diff`
- 恢复：`run.revise`、`run.cleanup`、`hook.resolve`、`repair`
- 文档：`document.inspect`、`document.patch`、`frontmatter.patch`、`toml.patch`

调用前先读取 capability catalog 中的 schema。写能力会在发生修改之前校验未知字段和必需参数。

## Resources

MCP 还提供只读资源：

- `spec-autonomous://capabilities`
- `spec-autonomous://progress`
- `spec-autonomous://runs/<run-id>`
- `spec-autonomous://work/<run-id>/<request-id>`

URI 只接受注册格式和安全 ID，不是任意文件读取接口。

## 安全边界

- MCP 不创建 Agent，也不提供模型采样；
- token 不出现在普通 progress / next 中；
- 写操作由 Git common-dir 仓库锁串行化；
- 输入和输出有大小上限；
- stdio EOF 或 SIGTERM 可以干净退出；
- 不支持的字段和未知工具会返回明确协议错误。
