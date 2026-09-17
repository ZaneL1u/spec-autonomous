# 安装方式

## Git 安装

这是当前对用户最直接的方式：

```sh
npm install -g git+https://github.com/ZaneL1u/spec-autonomous.git
```

需要：

- Node.js `^22.13.0 || >=23.5.0`
- Git
- 已认证的 `gh`

Git facade 本身不运行 npm lifecycle 安装脚本。第一次运行时才下载与版本、平台匹配的 Release 二进制，并校验摘要。

::: warning 平台边界
当前已发布并实际验证的是 macOS arm64。仓库已经具备六平台打包设计，但“workflow 中有矩阵”不等于对应产物都已经发布可用。
:::

## 从源码开发

开发者需要 Rust 1.98.1、Node.js、Bun 1.4.2 和 Git：

```sh
git clone git@github.com:ZaneL1u/spec-autonomous.git
cd spec-autonomous
. "$HOME/.cargo/env"
bun install --frozen-lockfile
bun run build:native
node scripts/test-all.mjs
```

本地打包：

```sh
bun run pack:local
npm install -g ./.artifacts/local/spec-autonomous-0.1.0-alpha.13.tgz
```

## OpenSpec / Spec Kit 要提前装吗

通常不用。JS 启动层会在需要执行原生流程时检查 provider：

```sh
spec-autonomous providers status --json
spec-autonomous providers ensure openspec --json
spec-autonomous providers ensure speckit --json
```

缺少 OpenSpec 时，会安装锁定版本到用户拥有的隔离目录。缺少 Spec Kit 时，会管理 `uv`、Python 和 `specify-cli`，不会要求项目依赖全局 Python 环境。

离线时不会假装安装成功。你需要先联网执行一次 `providers ensure`。

## 初始化已有项目

```sh
# 自动检测已有 SDD provider
spec-autonomous init --agent codex

# 同时绑定 MCP
spec-autonomous init --agent codex --mcp

# Claude Code
spec-autonomous init --agent claude --mcp
```

如果同时检测到多个框架，命令会要求明确选择，不猜“哪个更新”：

```sh
spec-autonomous init --provider openspec --agent codex --mcp
```

## 初始化空项目

```sh
spec-autonomous init
```

或在 CI 中：

```sh
spec-autonomous init \
  --provider speckit \
  --agent codex \
  --mcp \
  --non-interactive
```

初始化会调用所选框架自己的原生初始化器，然后添加 Spec Autonomous 的配置和宿主入口。它不会用私有模板冒充 OpenSpec / Spec Kit。

## 卸载项目绑定

```sh
spec-autonomous skills uninstall
```

卸载只删除安装清单中仍与原始版本一致的文件。被用户修改过的 Skill、已有的其他 MCP server 和无关配置会保留。
