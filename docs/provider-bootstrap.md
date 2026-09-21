# 原生 SDD 工具自动安装

从 alpha.3 开始，npm 包的 Bun / Node 兼容 JS 层负责安装 OpenSpec、Spec Kit 和缺失的运行前置依赖。Rust 只接收原生命令 argv 并维护工作流；安装由 JS 层完成。

## 使用

```sh
# 仓库已有 openspec/ 或 .specify/：自动补齐缺失 CLI
spec-autonomous init --agent codex --mcp

# 尚无 SDD：明确选择后，先生成原生 scaffold，再绑定我们的 Skills
spec-autonomous init --provider openspec --agent codex --mcp
spec-autonomous init --provider speckit --agent claude --mcp

# 只读安装状态 / 显式安装
spec-autonomous providers status --json
spec-autonomous providers ensure openspec --json
spec-autonomous providers ensure speckit --json

# 原生 CLI 不在 PATH 也能执行；参数按独立 argv 传递
spec-autonomous providers exec openspec -- --version
spec-autonomous providers exec speckit -- version
```

从 alpha.8 起可直接 `spec-autonomous init` 交互选择；详见 [初始化项目](initialization.md)。两种框架共存时必须明确选择。native init 在隔离临时目录中运行，随后预检目标文件：不同内容或符号链接冲突会报错，保留用户文件。现有框架只补齐 CLI，不重复生成或替换原生规范。

普通 CLI 的原生操作会先准备依赖。npm MCP transport 公开 `sa_providers`：

```json
{"operation":"ensure","provider":"speckit"}
```

省略 operation 时为 status；可选 `managed: true`。MCP 的 prepare、inspect、apply-result、archive、原生细粒度操作先准备依赖，之后交给 Rust。progress、doctor、next、detect、工具发现和 provider status 均不会安装或下载。安装日志进入 stderr，stdout 保持 JSON / MCP 协议。直接运行 Rust 二进制不包含 JS 安装服务；使用 npm 安装出的 CLI 获得完整入口。

## 安装位置与版本

| 项目 | 默认行为 |
| --- | --- |
| OpenSpec | `@fission-ai/openspec@1.13.0`，在隔离目录通过 npm 安装；脚本由 Bun 运行时使用 Bun add |
| Spec Kit | `specify-cli==1.0.6`，官方 PyPI 正式版，通过 uv tool install |
| uv | 复用已有可用 uv；缺失时下载官方 `0.12.13` 平台 archive 并核对内置 SHA256 |
| Python | uv 选择满足 `>=3.11` 的解释器；缺失时自动下载到专用目录 |

入口包版本固定；传递依赖由上游包元数据解析。受支持的 uv 平台与当前六个平台包一致，Linux 限 glibc。安装需要 npm registry、PyPI，以及缺 uv / Python 时的 GitHub 下载访问。下载与子进程有独立超时；不使用 sudo，不写 shell profile，不修改项目 package.json / lockfile。

默认位置：macOS 为 `~/Library/Application Support/spec-autonomous/providers`，Linux 为 `$XDG_DATA_HOME/spec-autonomous/providers`（默认 `~/.local/share`），Windows 为 `%LOCALAPPDATA%/spec-autonomous/providers`。

```sh
# 可用于受控缓存、迁移测试或 CI
export SPEC_AUTONOMOUS_PROVIDER_HOME=/absolute/path/to/provider-cache
# 离线时复用已有安装；缺失依赖会返回 provider_offline
export SPEC_AUTONOMOUS_OFFLINE=1
```

选择优先级：显式 OpenSpec argv 配置、项目本地工具、PATH、已验证 managed 工具。已有命令运行失败时报告 `provider_unusable`，不静默覆盖。可另装隔离版本并明确执行：

```sh
spec-autonomous providers ensure openspec --managed --json
spec-autonomous providers exec openspec --managed -- --version
```

`--managed` 不改写项目已有的工具选择。自动安装工具也不把命令加入全局 PATH；原生人工流程可使用 providers exec。

## 并发、失败与验证

各 worktree 共享用户安装目录及安装锁。每次安装使用独立 generation 路径，成功执行版本探测后才原子写 `current.json`；Python venv 不在安装后重命名。失败 generation 不会被当作可用安装，下次 ensure 重新安装。安装仍在运行时返回或等待锁；安装者退出后的锁可回收。用户已有工具不被删除或升级。

离线 mock 检查包含安装失败、并发、版本探测、checksum、原生文件冲突、CLI 参数边界、MCP 首次准备及只读查询。真实网络验收单独运行，不混入常规测试：

```sh
node --test packages/cli/test/providers.test.mts
node --test tests/e2e/provider-bootstrap.test.mts
node scripts/test-providers-real.mts --run
# 同一 JS 脚本也可由 Bun 执行
bun scripts/test-providers-real.mts --run
```

真实测试强制不复用 PATH 上的工具，使用独立 provider home，并要求受管理的 Python，覆盖没有 uv / Python 的首次安装链路。实际平台证据见 [验收记录](validation/provider-bootstrap.md)。

上游依据：[OpenSpec 安装说明](https://github.com/Fission-AI/OpenSpec/blob/main/docs/installation.md)、[Spec Kit PyPI 安装](https://github.github.com/spec-kit/install/pypi.html)、[uv CLI 与工具目录](https://docs.astral.sh/uv/reference/cli/)、[uv 0.12.13 发行](https://github.com/astral-sh/uv/releases/tag/0.12.13)。
