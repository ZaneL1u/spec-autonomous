# Design: 中文文档站

## 信息架构

站点按用户问题组织，而不是照搬源码目录：

1. 入门：是什么、快速开始、安装和完整流程。
2. 概念：SDD、生态关系、GSD 对比和核心理念。
3. 参考：CLI、Skills、MCP 和配置。
4. 进阶：扩展、恢复、清理与贡献。
5. 版本：面向用户的 Changelog。

正文以中文大白话解释，命令、标识符与专有名词保留英文。

## 构建

站点位于 `website/`，使用 VitePress 默认主题与少量仓库内 CSS。`base` 固定为 `/spec-autonomous/`，对应项目 GitHub Pages 路径；生成目录不进入 Git。

## 发布

独立 workflow 在相关 Pull Request 中构建站点，在 `main` 的相关路径变化后上传 Pages artifact 并部署。依赖通过 Bun 1.4.2 和 `bun.lock` 冻结安装，不使用发布密钥。

## 内容权威

用户文档以 `README.md`、`AGENTS.md`、能力目录、CLI help、已发布 Skills、OpenSpec 主规范和 contract tests 为事实源。文档明确区分已实现能力、已有构建路径和实际发布平台。
