## Context

私有仓库 ZaneL1u/spec-autonomous 已创建，main 指向 alpha.4。SSH 与 gh 均为用户已授权账号。沿用 npm / Commander / Clap 与 OpenSpec 的既有边界；用户补充另一台机器也已有 gh 登录。

## Goals / Non-Goals

**Goals:** 所给 Git URL 可直接 npm 全局安装；二进制从私有 Release 获取，不要求本地编译，不把 native outputs 提交到 Git。

**Non-Goals:** 不新增 agent launcher；不自动索取或存储 token；不将私有仓库改公开；不伪造其他平台产物。

## Decisions

1. 根 facade 使用 spec-autonomous 名称并声明 `packages/cli/bin/spec-autonomous.mjs` 入口。Bun 在根目录管理依赖，子目录作为独立 npm 包装，Rust Cargo workspace 保留。Git facade 不登记 JS workspaces，也不使用 build / prepare / postinstall 等触发 npm Git preparation 的脚本；实际 npm 11 测试发现 preparation 会把全局入口链接到会被清理的 Git 临时目录。构建命令改为 build:native。显式 files 只包含运行文件与说明。
2. 根独有 git-install.json 固定 repository、version、tag、每个平台 asset / SHA256。仅识别符合根 facade 布局的 manifest；普通 npm wrapper 不意外启用私有 GitHub fallback。
3. Git 安装本身只安装 JS facade 与 Commander；第一次 CLI 调用获取 native binary。没有安装期脚本，因此源码依赖安装、CI 和 --ignore-scripts 均不会意外联网取二进制。
4. gh release download 使用用户现有凭据，所有日志走 stderr。先校验 SHA256，再运行 --version，最后原子发布到独立用户缓存；并发进程共享锁，失败不留下 ready binary。禁网络模式明确报错，已有正确缓存可复用。
5. 发布 raw native asset 与现有 tgz / checksum。先将新源码提交到远程，随后发布与该提交绑定的 alpha.5 Release，最后真实安装默认分支与版本 tag。首次 push 的源仓库 CI 不受尚未发布 Release 影响。

## Risks / Trade-offs

- 私有 Git 的 SSH 权限和 Release API 的 gh 权限不同；缺 gh 或未登录时给出明确指引，不自动开启授权。
- 其他平台尚无预编译产物；返回平台不可用而不自动编译。
- Git 安装不运行 Rust 构建；用户已登录 gh，首次启动需要访问私有 Release，后续正确缓存可离线使用。
