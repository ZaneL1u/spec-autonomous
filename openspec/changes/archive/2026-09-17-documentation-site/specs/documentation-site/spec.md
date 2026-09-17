## ADDED Requirements

### Requirement: 提供面向用户的中文文档站

系统 SHALL 提供可静态构建的中文文档站，以通俗语言说明产品定位、快速开始、安装、工作流、核心理念、SDD 生态差异、CLI、Skills、MCP、扩展、贡献和版本变化。

#### Scenario: 新用户寻找入门路径

- **WHEN** 用户进入文档站首页
- **THEN** 用户可以直接进入快速开始、核心概念、接口参考和 GitHub 仓库

#### Scenario: 用户核对产品边界

- **WHEN** 用户阅读架构或生态对比
- **THEN** 文档明确说明 CLI 不创建 Agent、原生规范保留权威，并区分已实现能力与尚未发布的平台

### Requirement: 在 CI 中验证并发布 GitHub Pages

系统 SHALL 使用锁定依赖在相关 Pull Request 中构建文档，并在 `main` 的相关变更后把同一 VitePress 产物发布到 GitHub Pages。

#### Scenario: Pull Request 修改文档

- **WHEN** Pull Request 修改站点、依赖或发布 workflow
- **THEN** CI 安装冻结依赖并执行 VitePress production build，但不部署 Pages

#### Scenario: 主分支修改文档

- **WHEN** 相关变更进入 `main`
- **THEN** CI 构建、上传 Pages artifact，并通过 `github-pages` environment 部署
