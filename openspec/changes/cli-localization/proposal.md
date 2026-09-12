## Why

CLI 的命令名和结构化字段需要稳定，但用户界面应遵循设备语言。当前帮助、状态 human 输出和错误全部是英文；中文环境下第一次使用不够自然，Skills 与 JSON/MCP 消费者也无法明确知道当前语言。

## What Changes

- 增加基于 BCP 47 的 locale 选择：`--lang`、`SPEC_AUTONOMOUS_LANG`，然后按 POSIX `LC_ALL`、`LC_MESSAGES`、`LANGUAGE`、`LANG` 和 Node Intl 回退。
- 提供共享、可审查的英文 / 简体中文消息 catalog，Rust 二进制嵌入同一 catalog，npm Commander 帮助复用 Rust 生成的本地化 schema。
- 本地化 root/子命令帮助、选项描述、provider 帮助、常见 CLI/Provider 错误和 human 进度标签；命令名、错误 code、JSON 字段、MCP 方法保持英文稳定。
- 未支持或缺失翻译回退英文；显式 `--lang` 优先于环境变量；`C`/`POSIX` 回退英文。
- 增加 locale 诊断、Node/Bun/Rust/安装包测试与 OpenSpec 验收文档，不改变 agent、provider 安装和原生工作流。

## Capabilities

### New Capabilities

- `cli-localization`: 双语 CLI 界面、locale 协商、catalog fallback 与稳定机器协议。

### Modified Capabilities

无。

## Impact

Rust Clap metadata/help/error 渲染，Node Commander 程序和 provider errors，`packages/cli/locales` 资源及 npm files，版本 alpha.6。JSON/MCP 的 key 和 enum 不翻译；Markdown spec、TOML 和源代码保持原格式。
