## Context

现有 npm launcher 仅转发参数。Rust OpenSpec adapter 优先显式配置、项目本地 CLI，再查 PATH；Spec Kit 读取仓库原生模板。init 仅绑定 Skills，缺少 provider 会失败。这里增加安装层，不把网络安装和语言环境管理放入 Rust。

## Goals / Non-Goals

**Goals:** npm 安装不执行 lifecycle 下载；首次明确使用时补齐依赖。Bun 和 Node 共用可注入 I/O 的 JS 模块。提供机器可读状态、重入安装和跨 worktree 复用。

**Non-Goals:** 不启动模型，不改全局 PATH / shell 配置，不升级用户原有工具，不执行未选择框架的初始化。离线首次安装不能伪造成功。

## Decisions

1. **独立用户安装目录。** SPEC_AUTONOMOUS_PROVIDER_HOME 可覆盖默认用户数据目录。每个版本在独立 generation 路径安装，只有执行版本探测成功才原子写入 current.json。venv 路径不重命名，避免 Python shebang 损坏。目录锁记录持有者及安装进程，失败可重试；陈旧 generation 保留以便诊断，不误用未验证安装。
2. **固定上游。** OpenSpec 1.13.0 通过 npm / Bun 安装到专用 prefix，忽略 lifecycle scripts；Spec Kit 1.0.6 通过 uv tool install 安装。缺 uv 时下载固定 0.12.13 官方平台 archive，匹配内置 SHA256 后解包；uv 在专用目录自动选择或下载 Python >=3.11。固定入口包版本，传递依赖仍按上游包元数据解析。
3. **复用和选择。** 优先有效的项目本地 / PATH 命令，再使用兼容已验证 managed 安装。不自动替换存在但运行失败的用户命令。检测使用 native detect，保持 Git 边界；歧义拒绝，明确 provider 可仅安装工具。空仓库 init 要求 provider 和 host；原生 scaffold 在临时目录生成，冲突检查后合入，已有文件不同则拒绝。
4. **统一调用。** providers status/ensure/exec 在 JS launcher 拦截；需要 provider 的普通命令先 ensure。Rust 仅接受 JS 提供的 OpenSpec argv 环境桥接，保留显式配置和本地 CLI 优先级。MCP JS transport 在需要原生工具的请求前 ensure，并把安装能力作为结构化工具公开；后端 provider bridge 每次读取已验证 receipt，支持当前 MCP 会话首次安装。stdout 保持协议输出，安装日志走 stderr。
5. **保持被动能力。** progress / doctor / 工具发现不下载。CLI exec 只转发已选择 provider 的原生命令和独立 argv；宿主继续负责 agent。安装请求失败返回结构化原因，并阻止依赖它的核心操作。SPEC_AUTONOMOUS_OFFLINE=1 禁止新下载，已装工具可用。

## Risks / Trade-offs

- [网络 / registry 不可达] → 有限超时、失败不写就绪记录、重试、离线诊断。
- [外部工具存在但损坏] → 报具体命令探测失败，不静默覆盖；显式 managed 选项可选隔离版本。
- [并发和中断] → 安装锁、子进程终止、generation 隔离和就绪记录原子替换。
- [平台差异] → 官方平台映射和固定 hash；本机真实安装，Windows / Linux 分开报告未运行证据。
- [完整回归暴露的既有暂停竞态] → apply-result 持有协调锁时接到 pause 邮箱，保存回执后确认 run 为 paused；保留 submitted 回执供续跑，不反复返回 awaiting_host。使用锁竞争的确定性 contract 验证。

## Migration Plan

发布 alpha.3，原 npm 安装命令不变。原仓库无需改规范或 lockfile；已有安装先复用。原生二进制单独运行仍只提供工作流核心，完整自动安装入口为 npm JS launcher。回退 npm 包不会删除用户目录的 provider。
