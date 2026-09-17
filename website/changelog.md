# Changelog

这里记录面向用户的主要变化。完整提交和 Release 产物请看 [GitHub Releases](https://github.com/ZaneL1u/spec-autonomous/releases)。

## Unreleased

- 新增 VitePress 中文文档站和 GitHub Pages 自动发布。
- 归档已完成的 OpenSpec changes，并把 host-driven 边界同步到主规范。
- 对齐 Smart Discuss 后的 batch claim、host protocol 和 E2E fixture。
- 补全主规范 Purpose，使 OpenSpec strict validation 全部通过。

## 0.1.0-alpha.13

- 明确敏捷 milestone、phase/spec、workstream 和多团队协作模型。
- 区分 milestone 内部 revision 与产品发布版本。
- 固化 host-driven 架构：CLI 生成工作包、验证宿主回执，不创建 Agent。
- 增加可恢复验证修订、批量领取和终态资源清理能力。

## 0.1.0-alpha.12

- Auto 把 Smart Discuss 作为每个 phase 的自动前置门。
- 无灰区时继续，有用户决策时明确暂停。

## 0.1.0-alpha.11

- 增加可点击的 Smart Discuss phase 推荐与替代选项。
- 决策通过 source hash CAS 持久化。

## 0.1.0-alpha.10

- 构建并打包 alpha.10 macOS 二进制。
- 完善 dogfood 诊断与验收记录。

## 0.1.0-alpha.9

- 支持从错误 verification argv / cwd 恢复同一个 run。
- 保留已验收工作和历史失败证据。
- 降低宿主上下文与重复规划开销。

## 0.1.0-alpha.8

- 新增交互式初始化。
- 空项目可以选择 OpenSpec / Spec Kit、Codex / Claude Code 和 MCP。
- 自动化环境支持完整非交互参数。

## 0.1.0-alpha.7

- 补全简体中文 CLI 展示。
- 增加系统语言检测和显式语言覆盖。

## 0.1.0-alpha.6

- CLI 支持 English / 简体中文本地化。
- 保持命令名、JSON 字段、错误代码和状态值稳定。

## 0.1.0-alpha.5

- 支持从私有 Git SSH 地址安装 npm facade。
- 首次运行通过 GitHub Release 下载并校验原生二进制。

## 版本说明

当前仍是 alpha：

- GitHub Release 安装的实际发布范围是 macOS arm64；
- 公开 npm registry 尚未发布；
- 六平台发行 workflow 是构建能力，不代表所有平台已有可下载产物；
- 上游 OpenSpec / Spec Kit 兼容性按仓库锁定版本和 contract tests 声明。
