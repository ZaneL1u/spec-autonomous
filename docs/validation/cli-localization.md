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

## alpha.7 修正验收（2026-09-12）

用户反馈证实 alpha.6 的“完整本地化”范围不准确：缺少系统 UI 语言检测、Commander 标题及错误、初始化选项指引。本轮补齐这些内容，并删除当前产品介绍中的 Agent 启动免责声明。

实际验收平台：macOS arm64。Node 24.21.0 完整仓库检查通过：136 项 Rust 测试（包含显式执行的真实 OpenSpec 合约）、52 项 JS/发布测试、73 项 E2E；OpenSpec strict 8 项通过。macOS/Windows 首选语言读取有隔离测试，Windows 实机未验收。

最终共享目录包含 315 个消息键，其中 190 个产品错误键。发布二进制上的 16 项定向测试通过，覆盖全部原生命令/参数中文帮助、空仓库 init、JSON code、显式英文覆盖、参数错误和字面量 --help。进度测试证明 /tmp/completed/unknown 和任务 ID 不会被翻译。最后一次修改仅去掉参数错误的重复帮助提示，已用发布二进制重跑上述定向测试。

安装包实际通过 Node 22.23.2 和 Bun 1.4.2 的帮助、init、错误与无写入验收；源码和打包资源都包含中英文目录。

- macOS arm64 binary SHA256：`d13724579ee9a17ec496e8c10d6c56a8987d984b670cd8da8f2b9de12fd3df71`
- npm tarball SHA256：`3c47b9d75ad8ad4a48e6539d81d42558829c458187959d0669e891f2145fea6d`

上游 OpenSpec / Spec Kit、npm、gh 和操作系统自身诊断保留原文。结构化字段和用户内容保持原值。

私有发布 `v0.1.0-alpha.7` 已完成。使用独立 npm cache、安装 prefix 和空二进制 cache，通过 Git SSH 标签安装；首次运行从私有 Release 下载并校验 SHA256。Node 22.23.2 与 Bun 1.4.2 均通过安装后的版本、中文根/嵌套帮助、英文覆盖、C locale、init 文案/JSON、参数错误及无项目写入验收。Git 标签指向实现提交 `ee3e4202f8a6417891eb06450d92c85db92cf8a5`，远端 Release 的资产 digest 与本地 manifest 一致。
