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
node scripts/test-all.mts
```

本地打包：

```sh
bun run pack:local
npm install -g ./.artifacts/local/spec-autonomous-0.1.0-alpha.16.tgz
```

## 从本地源码目录安装（没有 Bun 时）

`npm install -g <源码目录>` 只会建一个指向该目录的软链，**不会**安装 `dependencies`（`commander`、`@inquirer/prompts`），第一次运行会报 `ERR_MODULE_NOT_FOUND`；目录里也没有本机二进制。只有 Node、npm、Rust 时按下面做：

```sh
cd /path/to/spec-autonomous
npm install --omit=dev                 # 把运行时依赖装进源码目录的 node_modules
node scripts/build-native.mts          # cargo build --release，并复制到 packages/cli/native/<平台>/
npm install -g .                       # 全局软链，依赖从上一步的 node_modules 解析
spec-autonomous --version
```

- `build-native` 需要 Rust 1.98.1；启动层优先使用 `packages/cli/native/<平台>/` 里的二进制，也可以用 `SPEC_AUTONOMOUS_BINARY=/绝对路径/spec-autonomous` 指定。
- 更新源码后重新执行后两步；`git pull` 改了 `package.json` 依赖时再跑一次 `npm install --omit=dev`。
- 有 Bun 时仍推荐上面的 `bun run pack:local` + 安装 tgz：tgz 安装会正常装依赖，且不依赖源码目录继续存在。

## MCP 命令与 PATH

`spec-autonomous init --mcp` 写入的 MCP 配置原本固定为 `command: "spec-autonomous"`。宿主（尤其从桌面启动的 IDE / Claude Code）继承的 PATH 里没有它时，MCP 进程直接 `ENOENT`，会话里所有 `sa_*` 工具不可用，而 doctor 也无从提示。现在：

- 通过 npm 启动层（`spec-autonomous init ...`）运行时，若 `spec-autonomous` 在当前 PATH 上，仍写可移植的裸命令；不在 PATH 上时写绝对路径：`command` 为 Node 可执行文件，`args` 以启动脚本 `bin/spec-autonomous.mjs` 的绝对路径开头，再接 `--path <项目> mcp`。
- 直接运行原生二进制且 PATH 上没有 `spec-autonomous` 时，仍写裸命令，并在结果的 `warnings` 中给出 `mcp_command_not_on_path`。
- `spec-autonomous doctor` 检查 `.mcp.json` 与 `.codex/config.toml` 里 `spec_autonomous` 的命令：裸命令不在 PATH 上报 `mcp_command_not_on_path`，绝对路径不存在报 `mcp_command_missing`。注意桌面启动的宿主看到的 PATH 可能比终端短。
- 修复办法：把 CLI 放到宿主可见的 PATH 上，或在 PATH 之外通过启动层重新运行 `spec-autonomous init --agent <codex|claude> --mcp`（只替换本工具写入且未被修改的条目）。

## OpenSpec / Spec Kit 要提前装吗

通常不用。启动层会在需要执行原生流程时检查 provider：

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
