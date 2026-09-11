## Context

初始 Git 仓库无文件和提交。本机是 macOS arm64，原本有 Node/npm，缺 Rust/Bun；本次安装工具链并固定 Rust 1.98.1、Bun 1.4.2、OpenSpec 1.13.0。

## Goals / Non-Goals

交付可编译的 Rust 检测 CLI、可通过 npm 安装的本机 tarball、可复现上游研究和完整 OpenSpec 后续方案。本变更不含自主执行、模型集成、六平台验证或 registry 发布。

## Decisions

- Rust core/CLI 两个 crate，Node launcher 只选 binary 并转发进程；Bun 是开发工具。
- detect 只读直接 markers、最近根与 Git 边界，不执行仓库脚本；多来源返回 ambiguity。
- 本地 tarball 内含本平台 binary；正式发行组包为 wrapper + 六个精确版本 optional 平台包。
- Cargo 显式 target，从 JSON compiler-artifact 获取真实输出路径，兼容自定义 target-dir。
- npm 通过 Node 执行 npm-cli.js，避免 Windows shell 路径拆分；CI 在上传前 pack 成 tgz 保留执行权限。
- .references 不入 Git，只提交来源/commit lock 和分析；OpenSpec 使用原生 spec-driven schema。
- 已实现行为归档到主规范，未来 autonomous 行为留开放变更；不混淆规划和实现完成。

## Risks / Trade-offs

- 当前只有 macOS arm64 本地证明；GitHub workflows 和其他平台仍待实际运行。
- npm 名称未发布/预留；文档将 registry 安装标为发布后，当前提供真实本机 tarball。
- 只读检测是安装线索，不证明 upstream CLI 可用、规划完整或支持执行。

## Migration Plan

空仓无需迁移；后续扩展 core 模块，保持 detect JSON 与 wrapper 行为兼容。用户按 README 构建或安装本机 tarball。
