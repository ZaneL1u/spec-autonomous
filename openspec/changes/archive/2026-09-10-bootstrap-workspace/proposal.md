# Bootstrap the Rust and npm workspace

## Why

空仓库需要可构建、可测试、可安装的工程基础，以及基于真实上游源码的 OpenSpec 实施方案，才能开始自主里程碑产品的后续开发。

## What Changes

- 初始化 Rust core/CLI、Bun workspace 和薄 Node npm launcher。
- 提供只读 OpenSpec/Spec Kit marker 检测、JSON 输出与错误诊断。
- 提供本机 npm tarball、平台发行组包脚本及 CI 配置。
- 克隆并锁定 OpenSpec、Spec Kit、GSD Pi/Core，完成中文调研。
- 使用官方 OpenSpec 初始化仓库并形成 autonomous-orchestration 完整方案。

## Capabilities

### New Capabilities

- `repository-detection`: 只读、带边界与歧义诊断的框架检测。
- `native-cli-distribution`: Rust binary + npm launcher、平台组包与本机安装入口。

### Modified Capabilities

无。

## Impact

新增 Cargo workspace、Bun locks、packages/cli、scripts、GitHub workflows、docs、OpenSpec 与生成的 Codex skills。参考源码、编译产物和运行状态 gitignored。本次不实现或宣称已交付 autonomous runtime，不发布 npm 包。
