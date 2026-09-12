# 私有 Git 安装验收

日期：2026-09-11。版本：0.1.0-alpha.6。实际预编译平台：macOS arm64。

## 本地验证

完整本地 gate 已通过：131 项 Rust 测试及另 1 项真实 OpenSpec 合约、46 项 JS/打包测试、69 项 e2e、7 项 OpenSpec strict validation。Node 22.23.2 / npm 10.9.8 的 Git binary 与 Git-local 安装 9 项测试通过。

- 根 facade 使用正式 spec-autonomous 名称，npm 全局安装不会与原 tarball 的 bin 名称冲突。
- 两项真实 npm Git-local e2e 覆盖默认安装和 --ignore-scripts。安装产物不包含 Cargo.toml / native 源副本，首次启动通过 gh fixture 获取正确 binary；之后缓存可离线使用。
- 七项 Git binary 合约覆盖并发、校验失败、版本失败、下载失败、重试、缺 gh、离线、平台缺失、facade 识别与 manifest 校验。
- 源码安装不执行下载脚本；Bun frozen lockfile 通过。
- 原 CLI、provider、MCP、tarball 与发行约束保留。完整测试日志为 `.artifacts/git-install-full.log`，Node 22 Git 测试为 `.artifacts/git-install-node22.log`。

npm 11 测试曾发现 Git preparation 将全局包链接到临时 clone。最小复现证明带 build 脚本会触发，去掉准备阶段的根 facade 可正常复制安装。因此使用 build:native，并保留纯 JS 首次启动下载；不使用临时链接、额外安装参数或本地编译规避。

## 发行产物

- `spec-autonomous-darwin-arm64`：alpha.6 SHA256 `ee9f624348bbf2bf40abee92d08e733f3d27ed8de9678b0a7a8403670d078ed9`。
- `spec-autonomous-0.1.0-alpha.6.tgz`：最终本地包哈希记录在生成后的验收 JSON 中。
- 根 git-install.json 固定 repository、tag 和 binary SHA256；仓库与 Release 均要求保持 private。

## 私有 GitHub 实测

私有仓库 [ZaneL1u/spec-autonomous](https://github.com/ZaneL1u/spec-autonomous) 的上一版 Release 为 alpha.5；alpha.6 会在本次源代码提交后创建新的 prerelease，并绑定同一提交的二进制 asset。API 验证 private=true。

alpha.5 已在 Node 22.23.2 / npm 10.9.8 下完成真实 Git SSH 安装；alpha.6 将在发布后复用同样的全新 prefix、gh 下载、SHA256、离线 help 和 detect 验收。安装产物不是临时目录链接，不含 Rust 源码或本地 native 副本。证据分别见 `.artifacts/private-git-real.json` / `.log` 与 alpha.6 发布记录。

本机未构建其他平台的 alpha.6 二进制，因此不将那些平台标为可安装。此前 alpha.4 的 GitHub macos-14 CI 成功；Linux / Windows CI 有失败，不能作为跨平台交付证据。
