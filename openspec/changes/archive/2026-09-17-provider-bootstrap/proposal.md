## Why

另一台机器安装 Spec Autonomous 后，缺失 OpenSpec / Spec Kit 会阻断里程碑工作。npm 包应自行补齐所选原生工具，而不要求用户安装 Rust、Bun、uv 或 Python，也不替换已有 SDD 工作流。

## What Changes

- 增加 Bun / Node 兼容的 JS provider status、ensure、exec 能力，自动安装缺失工具及 Spec Kit 的 uv / Python 前置依赖。
- init 自动补齐已检测框架；空仓库通过显式 provider 选择调用原生初始化，保留已有文件。
- npm CLI 和 MCP 的需要原生工具的操作自动准备依赖；只读 progress / doctor 不触发下载。
- 用户目录隔离安装，固定受支持版本，校验 uv 下载，锁定并发安装并在成功验证后发布就绪记录。
- 打包、Skills、文档和 mock / 真实安装验收覆盖新的安装层；不引入 agent launcher。

## Capabilities

### New Capabilities

- `provider-bootstrap`: JS 层的原生工具安装、探测、执行、初始化和恢复契约。

### Modified Capabilities

无。

## Impact

packages/cli 的 JS launcher、lib、Skills；少量 Rust 原生命令环境桥接；包测试、e2e 和安装说明。固定 OpenSpec 1.13.0、Spec Kit 1.0.6（PyPI 正式版）、uv 0.12.13；首装需要访问 npm / PyPI / GitHub，之后复用已验证安装。
