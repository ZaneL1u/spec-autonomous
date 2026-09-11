# 从 alpha.1 迁移到 host-driven alpha.2

alpha.2 移除产品中的 agent launcher。`run` / `autonomous` / `auto` 是 prepare 的兼容别名，返回工作包，不再启动 Codex/Claude 或等待模型完成。原生规范、TOML roadmap 与 Git 验证能力继续使用。

## 配置

新配置使用 `schema_version = 2` 和 `[host]`。旧 schema 1 配置可以读取，但 `[runner]` 不再执行；doctor 显示 legacy_runner_ignored。可以使用 `repair` 的 `legacy-config` 预览，传回 expected_hash 后应用；原始配置备份保存在 Git common-dir runtime 中。不要把模型认证数据复制到工作包。

host.max_concurrency 是宿主明确声明的并发上限，默认 1；execution.max_workers 是项目希望使用的上限，实际取较小值。宿主会话标识由宿主分配，fresh_context 声明及跨请求唯一性由 CLI 检查；这是 host-reported 证据，不是 CLI 对宿主内部会话的观察。

## 调用协议

- 每次调用返回一个 JSON envelope；持久事件通过 history.get 查询。
- prepare 返回 awaiting_host 和 work 数组，零退出码表示请求成功准备，不表示里程碑完成。
- 宿主使用自己的 agent 工具处理 work，claim/heartbeat 记录身份与活性，apply-result 提交结果。
- 新的公开响应默认是摘要；status 兼容入口及 `--view full` 提供详细读取。
- pause/cancel 要求宿主停止外部执行并确认 revoke；不会从 CLI signal 外部 agent。
- receiving 表示回执事务需要恢复，重新提交相同结果；不创建新的语义工作。

## 数据与旧版本

SQLite user_version 升至 2，升级前对已有库创建一致性备份。新 CLI 可读取旧 run，但不能重新激活其旧 runner；检查原生文件和 worktree 后，把选定来源导入新 run。旧 CLI 拒绝较新账本，避免把 host 请求误解为旧执行循环。

不回滚删除新的请求/回执记录。若要运行旧版本，请使用升级前备份的独立副本进行只读调查；不要让两版 CLI 共同写同一 runtime。

## Skills 和 MCP

重新执行 `init --agent codex` 或 `init --agent claude` 更新未修改的自有 Skills；需要结构化工具时加 `--mcp`。已有同名第三方配置或人工改动不覆盖。MCP 绑定仍遵守宿主的项目信任规则，配置后重新加载宿主。

上轮 alpha.1 真实模型运行是历史证据，不计作 alpha.2 的 host 协议验证。本轮 mock host 是独立测试程序，产品 CLI 不依赖它。
