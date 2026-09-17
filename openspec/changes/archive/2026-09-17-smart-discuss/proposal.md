## Why

GSD 的 discuss-phase 会在每个 phase 规划前识别灰区，展示问题和选项，再将决策写入上下文供后续规划使用。Spec Autonomous 需要同样的效果，但交互应是宿主可渲染的默认推荐/备选点击，而不是要求用户长篇回答。

## What Changes

- 为当前 phase 生成稳定的 gray-area cards：问题、默认推荐、备选项、依据、影响和可跳过标记。
- 支持 `discussion.next` 只读预览、`discussion.apply` 应用一个或多个点击选择，以及 `--auto` 全部采用推荐项。
- 决策写入运行账本和 Markdown context/decision log，plan-tasks 与 worker packet 可读取，原生 spec 不被静默修改。
- 交互结果可审计、幂等、带 phase/source CAS；已锁定决策不会被后续默认值覆盖。

## Capabilities

### New Capabilities
- `smart-discuss`: host-renderable phase discussion cards and decision persistence.

### Modified Capabilities
- `host-work-protocol`: discussion decisions become an explicit pre-planning input.

## Impact

Rust model/state/capability registry, CLI/MCP structured views, Skills, docs, tests and alpha.11 package.
