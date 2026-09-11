## Context

见 proposal。JS commandIndex / option / stripOption 扫描原始 argv，和 Clap 的值解析、层级帮助存在分歧。Rust 的命令定义是原生语法的权威来源。

## Goals / Non-Goals

**Goals:** 用成熟框架消除手写 token parser；帮助与语法错误不会安装依赖；原生命令元数据从 Clap 导出；provider service 接受解析后的对象，与 argv 解耦。

**Non-Goals:** 不替换 Rust 工作流、安装引擎或 MCP 能力，不增加交互式向导或 agent launcher。

## Decisions

1. **Clap + Commander。** 保留已有 Clap；Commander 14.0.3 无传递依赖、支持当前 Node >=22，适合本项目有限 JS 子命令。oclif 的插件/生成器体系暂不需要。
2. **命令定义单一来源。** Rust 隐藏 cli-metadata 命令描述 Clap command tree，并使用同一 Cli parser 预检用户 argv。两种操作不读取项目、不写文件、不调用 provider。Commander 根据描述注册原生命令，另声明 providers 和 init 的 provider 选项；Clap 最终约束冲突、值类型和必选参数。
3. **解析与执行分离。** JS 使用解析后的 path/framework/agent/source 传给 provider context；无 commandIndex、option 或 stripOption。原生命令通过预检后才 ensure，真正执行时保留原始 argv；init 通过结构化参数生成其已知原生 argv。providers exec 的剩余参数由 Commander pass-through 管理。
4. **输出与退出。** Commander help 写 stdout，正常帮助 exit 0；语法错误 exit 2，业务错误 exit 1 或原生命令原 exit。JSON 模式错误使用 schema_version/error envelope，不混入终端错误文本。MCP 仍只输出 JSON-RPC。
5. **分发。** Commander 是固定版本 runtime dependency，Bun 管理 lockfile；npm 自行安装该依赖，无 lifecycle script。发布 gate 精确允许 wrapper 的 commander 14.0.3，拒绝任意其他 JS 依赖与所有平台包 JS 依赖。

## Risks / Trade-offs

- [两套框架默认差异] → Commander 从 Clap 元数据生成原生帮助；最终 parse-only preflight 与真实执行共用 Clap。
- [额外 native 启动成本] → 元数据与预检是本地只读短命令，不加载账本，不联网。
- [npm 安装新依赖需要 registry/cache] → 文档说明 Commander 随 npm 自动安装，固定版本并测试 --ignore-scripts 安装。
- [透传参数含选项名或空格] → 真实 CLI e2e 验证 --help / --json 等字面参数与 exit / signal。
