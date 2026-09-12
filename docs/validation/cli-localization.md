# CLI 国际化验收

日期：2026-09-11。交付版本：0.1.0-alpha.6。实际平台：macOS arm64。

## 设计调研结论

- POSIX / gettext 使用 `LC_ALL`、`LC_MESSAGES`、`LANGUAGE`、`LANG` 进行语言选择；CLI 保留该约定并增加显式 `--lang` 与 `SPEC_AUTONOMOUS_LANG`。
- Fluent 提供稳定 message id、资源分离、fallback 和变量格式化；Node `Intl` 提供 BCP 47/ICU locale 能力。[Fluent Rust API](https://docs.rs/fluent/latest/fluent/)、[Node Intl](https://nodejs.org/api/intl.html)
- 本项目用共享 JSON catalog 实现同样边界：Rust/Node/Bun 共用 key，避免两种运行时各自产生不一致文案。复杂复数/性别文案出现前再迁移 Fluent，不改变公共 key。

## 当前证据

- Rust locale 单元：显式覆盖、POSIX/BCP 47 归一化、catalog key parity、帮助和 error fallback。
- Node locale 单元：显式/环境优先级、中文别名、catalog parity、错误翻译和 `--` 透传隔离。
- CLI e2e：root/nested help、中文和英文、invalid JSON error、全局参数和 provider payload 均保持协议及无副作用。
- native `LC_ALL=zh_CN.UTF-8 --help`：命令描述、选项描述、帮助/版本文字中文；`--lang en` 切换英文。
- Node 22 / Bun 的 provider、CLI、MCP 测试保持通过；新增资源随 npm `bin/lib/locales/skills` 一起打包。

最终本地回归：131 项 Rust 测试及真实 OpenSpec 合约、49 项 JS/发布测试、70 项 E2E，全部通过；OpenSpec strict validation 8 项通过。alpha.6 tarball 包含 24 个文件和 `locales/en.json` / `locales/zh-CN.json`，SHA256 为 `77f0cc9e6d03533d610f87bf3e45d75af0fa5d41b4f9e48046e468e3e5e03082`。私有 Git alpha.6 真实安装在 Node 22 下通过中文环境与英文显式覆盖。

## 不变内容

命令 identifier、JSON/MCP schema、error code、状态枚举、Markdown spec、TOML、Provider 原生输出不翻译。未知语言和缺失翻译回退 English，不联网。
