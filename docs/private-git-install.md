# 私有 GitHub 直装

需要 Node.js 22+、Git、仓库 SSH 访问权限，以及已登录并能读取私有仓库的 gh。当前用户仓库为 [ZaneL1u/spec-autonomous](https://github.com/ZaneL1u/spec-autonomous)。

```sh
npm install -g git+ssh://git@github.com/ZaneL1u/spec-autonomous.git
spec-autonomous --version
```

固定版本：

```sh
npm install -g git+ssh://git@github.com/ZaneL1u/spec-autonomous.git#v0.1.0-alpha.10
```

源码通过 SSH 拉取；二进制通过 gh 的现有 GitHub 登录读取私有 Release。首次调用会自动下载，校验根 git-install.json 固定的 SHA256 和 --version 后才放入用户缓存。无须编译 Rust 或安装 Bun，不会把 token 写入项目。`--ignore-scripts` 安装同样可以首次启动恢复。

目前提供 macOS arm64 预编译产物。未发布的平台明确报 git_binary_unavailable，不会自动编译或使用错误平台文件。普通平台 tgz 仍包含本机二进制，不依赖 gh。

安装后进入实际项目：

```sh
spec-autonomous init --agent codex --mcp
# 空仓库明确选择框架
spec-autonomous init --provider openspec --agent codex --mcp
# Spec Kit 对应 --provider speckit
```

## 缓存和故障处理

- gh 未安装：安装 GitHub CLI 后执行 `gh auth login`。
- 下载失败：用 `gh auth status` 确认账号和该私有仓库访问权限，然后重试 CLI。
- 校验失败：不会使用下载内容，下次调用可重新下载。
- `SPEC_AUTONOMOUS_BINARY_CACHE` 可指定缓存根目录。
- `SPEC_AUTONOMOUS_OFFLINE=1` 只允许使用已有正确缓存；缺失时明确返回错误。

默认缓存为 macOS 的 `~/Library/Caches/spec-autonomous/binaries`、Linux 的 `$XDG_CACHE_HOME/spec-autonomous/binaries`（默认 `~/.cache`）或 Windows 的 `%LOCALAPPDATA%/spec-autonomous/binaries`。每个版本、平台和 SHA256 使用独立缓存，多个进程通过锁共享安装结果。

## 开发和发行

根 package.json 是 Git facade，使用与 CLI 相同的 npm 名称，避免升级后全局 bin 冲突。根 dependencies 是固定 Commander；Bun 管理根依赖，Rust 仍使用 Cargo workspace。`packages/cli` 保持独立的常规 npm 包装。

根没有 build / prepare / postinstall 等 npm Git preparation 触发项，也不声明同名 JS workspace。实际 npm 11 全局 Git 测试发现 preparation 会产生指向临时 Git 目录的安装链接；无准备阶段的 facade 避开了该问题并减少安装依赖。原生构建命令为：

```sh
bun run build:native
node scripts/pack-local.mjs
```

先构建并验证各平台 native binary，再更新 git-install.json 中对应版本、asset 名称和 SHA256。源代码提交到 main 后，在同一私有仓库发布相同 tag / commit 的 Release，上传 native asset、普通 tgz 和 checksum。不要把 native outputs 放进 Git 历史，也不要修改已发布版本的二进制。

验证记录见 [Git 安装验收](validation/private-git-install.md)。
