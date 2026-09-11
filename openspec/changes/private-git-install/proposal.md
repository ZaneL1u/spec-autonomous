## Why

用户需要直接通过 `npm install -g git+ssh://git@github.com/ZaneL1u/spec-autonomous.git` 安装。目标 Mac 已登录同一 GitHub 账号的 gh；当前仓库根目录只是 workspace，没有可安装 CLI 入口。

## What Changes

- 根目录成为私有 Git 安装 facade，暴露同一个 spec-autonomous bin 和固定 Commander 依赖。
- 使用现有 gh 授权从私有 GitHub Release 下载当前平台预编译二进制；固定版本与 SHA256，不需要 Rust/Bun。
- Git 入口不依赖生命周期脚本，首次调用准备二进制；源码依赖安装不下载，--ignore-scripts 安装也可用。
- 保留已有嵌套 CLI tarball / npm 平台包分发；native outputs 不进入 Git 历史。
- 发布 alpha.5 私有 Release，并验证真实 Git SSH 全局安装。

## Capabilities

### New Capabilities

- `private-git-install`: 私有 Git 直装、gh 二进制下载、校验和缓存恢复。

### Modified Capabilities

无。

## Impact

根 package.json、JS binary resolver、新的 Git delivery manifest / installer、打包和 e2e 测试、私有 GitHub Release。当前实际预编译产物为 macOS arm64；其他平台明确报告未发布产物。
