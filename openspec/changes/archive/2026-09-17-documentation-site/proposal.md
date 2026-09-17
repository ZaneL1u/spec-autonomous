# Proposal: 发布中文文档站

## Why

仓库现有信息分散在 `README.md`、架构文档、调研报告与能力清单中，新用户难以快速理解 Spec Autonomous、SDD、OpenSpec、Spec Kit 和 GSD 之间的关系，也缺少可持续发布的公开文档入口。

## What Changes

- 增加面向用户的中文 VitePress 多页面站点。
- 覆盖快速开始、安装、工作流、核心理念、生态对比、命令、Skills、MCP、扩展、贡献和 Changelog。
- 使用仓库锁定的 Bun 工具链构建站点。
- 在 Pull Request 中验证构建，在 `main` 更新后通过 GitHub Pages 发布。

## Impact

- 新增 `website/` 文档源和 `vitepress` 开发依赖。
- 新增文档开发、构建与预览脚本。
- 新增 GitHub Pages workflow。
- 不改变 CLI、运行账本或 host-driven 工作协议。
