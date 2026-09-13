## Why

空目录运行 init 只报缺少框架，用户必须提前知道参数；原生框架初始化和项目编排配置也缺乏统一完成反馈。需要一次交互或参数调用完成初始化，并保留原生框架工作流。

## What Changes

- TTY 下补问缺失的框架、宿主与可选 MCP；完整参数、CI、JSON 和 non-interactive 模式不等待输入。
- 提供 --interactive、--non-interactive、--yes 默认选项；中英文提示和取消处理。
- 空目录初始化 Git、原生框架、Spec Autonomous 配置/目录/Skills，已有项目保留原文件与配置。
- 安装前预检本项目配置与所有权冲突；测试取消、重复执行、真实框架和发布安装链路。

## Capabilities

### New Capabilities
- `interactive-init`: 交互与参数初始化、完整基础结构及错误/取消语义。

### Modified Capabilities
无。

## Impact

JS Commander 与 Inquirer prompts、共享翻译目录、Rust init 预检与结构生成、npm 依赖、单测/终端 E2E 与私有 alpha.8 发行。
