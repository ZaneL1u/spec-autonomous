## Context

现有 CLI 是 Rust Clap + Node Commander；Commander 的帮助由 Rust `cli-metadata describe` 生成。Rust human 输出集中在 progress，业务错误使用稳定的 snake_case code。双运行时必须共享翻译内容，避免 npm 和 native 各自产生不一致文案。

## Goals / Non-Goals

**Goals:** 设备中文环境自动显示简体中文；英文及未知语言显示英文；用户可显式切换；帮助、错误和 human 状态一致；结构化协议保持可编程稳定。

**Non-Goals:** 不翻译命令 identifier、JSON 字段、MCP tool 名、Markdown spec 内容、Provider 原生 stdout 或第三方错误全文；不引入自动翻译服务或联网下载语言包。

## Decisions

1. **消息 catalog + stable keys。** 使用随包分发的 `locales/en.json` 和 `locales/zh-CN.json`，key 为 `command.*`、`arg.*`、`error.*`。这是 gettext/Fluent 的同一条业界原则：稳定 message id、locale fallback、翻译资源与代码分离；选择 JSON 是因为 Rust 与 Node 共享且无需新增运行时依赖。未来可机械迁移为 FTL，不改变 key。
2. **locale 协商。** 优先 `--lang`，再 `SPEC_AUTONOMOUS_LANG`、`LC_ALL`、`LC_MESSAGES`、`LANGUAGE`、`LANG`，最后 Node `Intl.DateTimeFormat().resolvedOptions().locale`；只要语言子标签是 `zh` 就选择 zh-CN，其他语言选择 en。`C`/`POSIX` 为英文。Commander 在调用 metadata 前做一次轻量 bootstrap 读取 `--lang`，Rust 仍是最终解析权威。
3. **Rust 为帮助 schema 权威。** Clap `Cli::command()` 经过 locale mutator 更新 command/arg about，再由 metadata 输出 `locale`。直接 native binary 的 `--help` 也用同一 mutator；native parse error 用稳定 code + locale generic message，保留 detail 时不泄漏协议字段。
4. **Node 只本地化展示。** Commander 使用 metadata 的已本地化描述；provider/help 自己的新增文案从同一 JSON catalog 读取。语法错误显示本地化通用标题并保留可诊断 detail；执行的 Provider stdout/stderr 原样透传。
5. **机器协议不翻译。** JSON/MCP 的 `schema_version`、`error.code`、状态值和命令 identifier 永远英文；`error.message` 和 human format 随 locale 改变。MCP initialize 可说明 supported locales，但工具 schema 不随语言变形。

## Risks / Trade-offs

- [翻译 key 遗漏] → 启动时只允许 catalog fallback English；测试比较两份 key 集合并在 CI 失败。
- [环境变量格式复杂] → 仅做 BCP 47/POSIX 语言前缀归一化，未知值安全回退英文，并提供 `--lang` 覆盖。
- [帮助输出与 commander 不同步] → Commander 不复制 native 命令文案，始终消费 Clap metadata；provider 专属文案也集中在 catalog。
- [Unicode 终端] → stdout/stderr 使用 UTF-8 原样输出；macOS/Linux/Windows 仅根据平台验证，不能把未实测终端编码当作保证。
