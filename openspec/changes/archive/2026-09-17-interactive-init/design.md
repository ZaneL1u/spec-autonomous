## Context

复用现有 provider bootstrap，在临时目录运行原生初始化，冲突预检后合并。Rust 保持 Clap 语法权威；交互发生在 npm JS 入口。采用 @inquirer/prompts 8.7.2 的 select/confirm 与可注入输入输出，提示写 stderr。

## Decisions

1. 完整参数保持不提问。TTY 的缺失选择启用向导，--interactive 可补问 MCP；--non-interactive、CI、JSON/TOML 输出禁用自动提问；显式交互与结构化输出组合拒绝。--yes 仅在没有任何框架证据时默认 openspec，宿主默认 codex，MCP 默认开启；不解决已有框架歧义或覆盖文件。
2. 先以 Clap/Commander 校验参数，再发现原生框架与现有宿主，然后收集所有选择。Ctrl-C/EOF 以本地化取消消息和 130 退出，不创建 Git、不安装 provider、不写项目。未交互且选项缺失则保留稳定错误 code。
3. JS 解析后的 provider/agent/mcp 生成与参数模式一致的原生 argv。原生 init --check 预检 config、Skills 与 MCP 所有权和目录类型，不要求框架已安装。原生工具在 staging 生成，目标冲突检查后导入；成功后为未加入 Git 的目录执行 git init（不创建提交），现有 Git/worktree 保持不变。
4. Rust 初始化创建 config.toml、Skills 所有权清单、plans/milestones/archives 目录、gitignore 与可选 MCP。模板没有虚构验证结果或 roadmap。已有配置逐字保留；human 成功输出给出目录、框架、宿主、生成结构与入口，JSON 保持结构化 envelope。
5. 安装网络失败、文件冲突明确报告。已下载工具可复用；不删除用户文件。重复 init 保留原生规范与用户配置，不新建 Git 或覆盖其他宿主配置。

## Validation

隔离 prompt 测试覆盖默认/参数/检测/CI/JSON/取消。真实 PTY 验证键盘选择、取消与本地化；真实 OpenSpec/Spec Kit 空目录参数初始化检查结构及重复执行。完整仓库回归、Node/Bun 安装与私有 Git 发行校验。Windows 只报告模拟读取/跨平台代码，实际验收为 macOS arm64。
