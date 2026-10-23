# Changelog

这里记录面向用户的主要变化。完整提交和 Release 产物请看 [GitHub Releases](https://github.com/ZaneL1u/spec-autonomous/releases)。

## 0.1.0-alpha.15

- 串行且基线未变化时直接验证写入 worker 的集成候选，减少相同代码的重复门禁。
- 完整同输入验证收据可在审计等待中复用；源码、环境、工具或证据变化时重新验证。
- 安全交接点自动回收已结束的干净工作树，保留活跃任务、未提交改动、分支和证据。
- 复用关闭的只读规划/审计检出，按声明范围判断受影响阶段，未知范围仍保守重验。
- 修复消费者产物目录与协调器日志混用的问题；项目侧示例补丁保持单独提供。
- 现有 run 可保留进度继续执行；升级需统一切换写入器，不能混用新旧程序。

## 0.1.0-alpha.14

- 长任务可在安全交接点预览并清理已结束工作树，保留活跃所有者、恢复证据与未完成任务补丁。
- 验证波次提供独立产物目录、准备钩子、耗时与失败诊断，合并检查去重。
- 重新规划保留已审查的验证修订；修复写入范围误判、悬挂回执和心跳参数兼容。
- 进度支持显式关联执行仓库，并显示原生清单、重试开销和资源压力。
- doctor 检测同版本 skills 内容漂移；修复 Homebrew npm 查找和 CLI 本地化参数解析。
- 升级继续原 run，已运行的命令保留旧可执行文件；详细步骤见[长任务维护](https://github.com/ZaneL1u/spec-autonomous/blob/main/docs/long-running-runs.md)。


- 启动层、脚本与测试全部改用 TypeScript 编写，发行产物仍是 JavaScript：`packages/cli/src/*.mts` 经 tsdown 构建为提交在仓库中的 `packages/cli/{bin,lib}/*.mjs`，CI 校验产物与源码一致。
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
