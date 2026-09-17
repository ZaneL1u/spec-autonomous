## Why

Rust CLI 已使用 Clap，但 JS 安装层仍手写参数扫描和帮助文本，导致子命令帮助混杂，且部分无效参数在安装之后才由 Rust 拒绝。用户需要基于社区 CLI 框架的统一、可维护入口。

## What Changes

- JS 使用 Commander.js 14.0.3；保持 Node 22 的既有最低版本，不引入 Commander 15 的 Node 22.12 门槛。
- Clap 继续定义和校验原生命令；提供无副作用的命令描述与参数预检，使 JS 不重复维护原生命令表。
- Commander 声明 providers 子命令和 init 扩展，统一 help、参数类型、未知选项和 JSON 错误输出。
- 所有语法预检先于依赖安装和初始化；原生 exec 的 `--` 参数、退出码、信号与 MCP 协议保持原契约。
- npm wrapper 固定一个 Commander runtime dependency；发布校验只允许该明确依赖，平台包仍无 JS 依赖。

## Capabilities

### New Capabilities

- `cli-command-interface`: 社区框架管理的 CLI 命令、帮助、预检及透传契约。

### Modified Capabilities

无；既有 native-cli-distribution 的平台、参数和退出码要求保留。

## Impact

Rust clap introspection / parse-only API，JS command registration / provider context，npm manifests / bun.lock，发行依赖校验与测试。版本 alpha.4；CLI 不启动 agent，安装逻辑仍在 JS。
