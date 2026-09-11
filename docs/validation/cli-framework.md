# CLI 框架迁移验收

日期：2026-09-11。目标版本：0.1.0-alpha.4。实际平台：macOS arm64。

## 最终结果

| 检查 | 结果 |
| --- | --- |
| Rust fmt / clippy / 单元与合约 | 通过；131 项测试，另有 1 项真实 OpenSpec 合约 |
| JS / npm 组包与发布合约 | Node 22.23.2 下 39 项通过 |
| 全仓 CLI / Git / SQLite / MCP e2e | 67 项通过 |
| 已安装 alpha.4（--ignore-scripts，Node 22） | 11 项 CLI / provider e2e 通过 |
| Bun 1.4.2 CLI 和真实 OpenSpec 调用 | 通过 |
| OpenSpec 严格校验 | 6 项通过 |
| tarball、源码与安装后文件 | 20 个文件逐字节一致；Commander 14.0.3 实际安装成功 |

最终安装包：`.artifacts/local/spec-autonomous-0.1.0-alpha.4.tgz`，仅 macOS arm64。
SHA256：`a795f9b97779014891d9e0ccfb43949f6286da6ff04030f32b4c588dfdef4eed`。
安装证据：`.artifacts/cli-framework-installed.log`、`.artifacts/cli-framework-package-final.json`。

## 实现

Rust 保留 Clap，新增无项目副作用的命令元数据与 parse-only 接口。JS 使用 Commander.js 14.0.3 注册命令、选项、参数、别名和帮助。删除 commandIndex / option / stripOption；provider context 接收结构化对象，原生操作经过 Clap 预检后才准备依赖。CLI 不启动 agent。

## 验证证据

- Rust CLI 合约验证同一 Clap grammar、无效路径下的只读 metadata、参数类型/冲突拒绝与帮助 exit 0。
- 6 项新增 CLI e2e 覆盖 root/嵌套帮助、JSON 语法错误且无安装、全局参数位置/equals 形式、别名及原样 argv、原生 exec payload、结构化 source 与 init 参数。
- 原 provider CLI/MCP e2e 保留，覆盖首次自动安装、Markdown 保留、离线无下载、显式选择及失败状态。
- 39 项 JS / npm 组包及发布合约在 Node 22.23.2 通过。发布 gate 仅允许 wrapper 的 commander 14.0.3，拒绝范围、额外依赖、平台包依赖和 lifecycle scripts。
- Bun 1.4.2 在独立临时目录实际执行根/嵌套帮助、无效枚举、native 类型错误、status 和 MCP help，并通过新入口调用真实 OpenSpec 1.13.0。所有输出/退出状态正确，查询没有触发安装。证据 `.artifacts/cli-framework-bun.json` / `.log`。
- Bun install / frozen lockfile 通过；实际增加的是 CLI workspace 的精确 Commander 依赖，没有升级其他依赖。

主要日志：`.artifacts/cli-framework-full.log`、`.artifacts/cli-framework-node22.log`、`.artifacts/cli-framework-e2e-final.log`、`.artifacts/cli-framework-package.log`。

## 边界

只实测 macOS arm64。Windows / Linux / macOS Intel 的实际运行仍需对应机器或 CI。npm 包首次安装需能访问 Commander 所在 registry 或使用已有缓存；原生框架安装仍由显式使用触发。没有发布 npm 或上传远程仓库。
