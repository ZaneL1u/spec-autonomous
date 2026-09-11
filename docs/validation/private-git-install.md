# 私有 Git 安装验收

日期：2026-09-11。版本：0.1.0-alpha.5。实际预编译平台：macOS arm64。

## 本地验证

完整本地 gate 已通过：131 项 Rust 测试及另 1 项真实 OpenSpec 合约、46 项 JS/打包测试、69 项 e2e、7 项 OpenSpec strict validation。Node 22.23.2 / npm 10.9.8 的 Git binary 与 Git-local 安装 9 项测试通过。

- 根 facade 使用正式 spec-autonomous 名称，npm 全局安装不会与原 tarball 的 bin 名称冲突。
- 两项真实 npm Git-local e2e 覆盖默认安装和 --ignore-scripts。安装产物不包含 Cargo.toml / native 源副本，首次启动通过 gh fixture 获取正确 binary；之后缓存可离线使用。
- 七项 Git binary 合约覆盖并发、校验失败、版本失败、下载失败、重试、缺 gh、离线、平台缺失、facade 识别与 manifest 校验。
- 源码安装不执行下载脚本；Bun frozen lockfile 通过。
- 原 CLI、provider、MCP、tarball 与发行约束保留。完整测试日志为 `.artifacts/git-install-full.log`，Node 22 Git 测试为 `.artifacts/git-install-node22.log`。

npm 11 测试曾发现 Git preparation 将全局包链接到临时 clone。最小复现证明带 build 脚本会触发，去掉准备阶段的根 facade 可正常复制安装。因此使用 build:native，并保留纯 JS 首次启动下载；不使用临时链接、额外安装参数或本地编译规避。

## 发行产物

- `spec-autonomous-darwin-arm64`：SHA256 `288086a189852d67957f827f84975382d55a22e96da03c9cc2138f209281f7dc`。
- `spec-autonomous-0.1.0-alpha.5.tgz`：SHA256 `5075ab4f9ac0aeafc3b98416621165ea55bf9fb200069edb5d2d01ce6eef1728`。
- 根 git-install.json 固定 repository、tag 和 binary SHA256；仓库与 Release 均要求保持 private。

## 私有 GitHub 实测

私有仓库 [ZaneL1u/spec-autonomous](https://github.com/ZaneL1u/spec-autonomous) 已发布源代码；[v0.1.0-alpha.5 Release](https://github.com/ZaneL1u/spec-autonomous/releases/tag/v0.1.0-alpha.5) 绑定 `5a7466d42c21a498827f1e805f5cce174df1ea1c`，标记 prerelease。API 验证 private=true，三个附件的服务端 digest 与本地一致。

使用全新全局 prefix 和 binary cache，在 Node 22.23.2 / npm 10.9.8 下实际运行私有 Git SSH 安装，通过已登录的 gh 获取 binary。版本返回 0.1.0-alpha.5，SHA256 与 manifest 一致；离线 help 和真实项目 detect 通过。安装产物不是临时目录链接，不含 Rust 源码或本地 native 副本。证据 `.artifacts/private-git-real.json` / `.log`。

本机未构建其他平台的 alpha.5 二进制，因此不将那些平台标为可安装。此前 alpha.4 的 GitHub macos-14 CI 成功；Linux / Windows CI 有失败，不能作为跨平台交付证据。
