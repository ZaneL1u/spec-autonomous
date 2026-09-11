# 调研结论

调研日期：2026-09-10（America/Los_Angeles）。源码已 clone 到 `.references/`，具体版本、commit 和地址保存在 [upstreams.lock.json](upstreams.lock.json)。`bun run references:clone` 可以核验并复现；现有副本若发生变化，脚本停止，不覆盖它。

## 产品选择

核心承诺是：**给定里程碑目标，沿原生 OpenSpec / Spec Kit 流程生成 roadmap 并自主开发；已有规划可直接接入，原生手动与自主方式可交接。** skills 调用 CLI，from/to/only 选择 roadmap 阶段，progress 汇总同仓所有 worktree。TOML 管理编排声明，Markdown 保留规范及可解析的原生结构。

| 上游 | 固定源码版本 | 借鉴或适配的内容 | 需要本项目补齐的执行闭环 |
| --- | --- | --- | --- |
| [OpenSpec](openspec.md) | 1.13.0 | CLI JSON、工件依赖、规范与 tasks 生命周期 | task DAG、worker、验证后回写、里程碑恢复 |
| [Spec Kit](spec-kit.md) | 1.0.7.dev0（开发快照） | spec/plan/tasks、阶段与 story、workflow、converge | 细粒度隔离调度、统一自主推进、任务级恢复 |
| [GSD Pi](gsd.md) | 1.19.0 | host 驱动 auto loop、独立 session、attempt/result、恢复 | 借鉴执行原则；不依赖其整套 harness |
| [GSD Core](gsd.md) | 1.13.0 | 轻 coordinator、worker 自读磁盘、摘要与单写入者 | 将提示词约定落实为可验证的 Rust 状态机 |

这四份源码来自 [OpenSpec 官方仓库](https://github.com/Fission-AI/OpenSpec)、[Spec Kit 官方仓库](https://github.com/github/spec-kit)、[Open GSD Pi](https://github.com/open-gsd/gsd-pi) 和 [Open GSD Core](https://github.com/open-gsd/gsd-core)。GSD 身份按用户给出的 [opengsd.net](https://opengsd.net/) 确认。官网展示版本和搜索索引可能落后，具体技术结论以各报告中的固定 commit 源码链接为准。

## 影响架构的发现

1. OpenSpec 的工件 DAG 不是实现任务 DAG；ready/all_done 不是代码验收证据。
2. Spec Kit 已有自动 workflow 和并发，项目不能只把 slash commands 串起来；完整任务自主推进、隔离和可恢复集成才有价值。
3. Spec Kit 当前 feature 定位已改为目录上下文；旧式“根据 Git branch 猜 feature”会在 worktree 中出错。
4. GSD Pi 的运行时账本已用 SQLite，Markdown 是投影；它的不同并行路径并不都具有每任务 worktree 隔离。
5. GSD Core 的上下文分工值得保留：主线程读小摘要，worker 读任务所需文件；不能把全部子 agent 输出注回主会话。
6. GSD Core 的 new-milestone skill 先生成 requirements/roadmap，再由 autonomous skill 以 from/to/only 调度既有 phase 流程并重读状态；本项目也需要这条完整入口链，详见 [GSD 报告补充](gsd.md#8-补充skillroadmap-与-autonomous-的-fromto)。

## 方案落点

采用 Rust supervisor、NativeWorkflowBridge、独立 spec adapter/runner、TOML 编排声明和 SQLite 运行账本。CLI 统一输出 human/JSON/TOML，结构化 MD 读取带来源信息；每写 worker 独立 worktree，progress 关联 Git inventory 与运行状态。npm 分发独立 binary 和配套 skills，不携带完整 GSD runtime。Spec Kit 原生创建/模板 bridge 如需 Python/uv，单独声明依赖；已有文档适配保持独立。先实现 OpenSpec 从目标到交付的完整路径，再复用到 Spec Kit，见 [设计](../../openspec/changes/autonomous-orchestration/design.md)。
