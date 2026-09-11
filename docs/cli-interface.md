# CLI 框架与接口

alpha.4 使用 **Clap + Commander.js 14.0.3**。Clap 定义原生工作流的完整语法；Commander 管理 npm 入口、provider 子命令、init 扩展和帮助。参数解析不再依靠 commandIndex / option / stripOption 扫描原始 argv。

选择 Commander 14 保持原来的 Node >=22 兼容范围；当前 Commander 15 要求 Node >=22.12。项目需要的是有限子命令、帮助和校验，暂不需要 oclif 的插件体系。Bun 继续管理 workspace 和依赖，JS 也支持由 Bun 执行。

## 命令与帮助

```sh
spec-autonomous --help
spec-autonomous prepare --help
spec-autonomous providers --help
spec-autonomous providers ensure --help
spec-autonomous help providers ensure
spec-autonomous help tools call
```

根帮助包含七项主要工作流能力及 tools、init、mcp、providers。每一级帮助由框架生成，保留原生命令别名。既有 Skills / MCP 工具和 CLI 参数无需迁移。

全局参数可放在命令前后；值包含空格时使用 shell 的正常引用：

```sh
spec-autonomous --path "/path/with spaces" providers status --json
spec-autonomous providers status --path="/path/with spaces" --json
spec-autonomous --framework openspec prepare --goal "一个里程碑" --json
```

provider status/ensure 输出 JSON；原生能力保留 human/json/toml。使用 `--json` 或 `--format json` 时，语法错误输出为一个 `schema_version/error` envelope。帮助 exit 0，语法错误 exit 2；业务错误与原生程序退出状态按相应契约传播。未知选项、无效 host/provider、缺失参数、原生冲突和类型错误均在安装前被拒绝。

原生工具参数放在 `--` 后：

```sh
spec-autonomous providers exec openspec -- --help
spec-autonomous providers exec speckit -- version
spec-autonomous providers exec openspec --managed -- --version
```

此后的 `--help`、`--json`、路径和字符串属于原生工具；npm 入口不拦截，不经过 shell 拼接。

## 架构与测试

- Rust `cli_metadata` 从同一 Clap 定义导出 command tree，并提供 parse-only 校验。两项操作不读取项目或账本，不调用 provider，不安装依赖。
- JS `cli-program` 用 Commander 注册命令，预检通过后才向 provider service 传入结构化参数。真正运行原生命令时保留原始 argv，init 的新增 provider 选项转换为原生 framework 选项。
- provider context 不再理解 argv；CLI、真实安装测试和 MCP 均传入解析后的对象。
- `cli-process` 统一传递原生 stdio、退出码和信号。MCP 仍使用原有 JS transport 与 Rust capability 服务。
- npm 正常安装固定 Commander 依赖；不会改成安装时下载脚本或 CLI agent launcher。

验证包含 Clap 的无副作用预检合约、root/嵌套 help、别名、全局位置、拒绝参数时无安装、JSON 错误、原生透传和信号、provider / MCP 回归，以及安装后的 npm 包测试。实际结果见 [验收记录](validation/cli-framework.md)。

上游依据：[Commander 文档](https://github.com/tj/commander.js/tree/v14.0.3)、[Clap derive 文档](https://docs.rs/clap/latest/clap/_derive/)、[oclif 简介](https://oclif.io/docs/introduction/)。
