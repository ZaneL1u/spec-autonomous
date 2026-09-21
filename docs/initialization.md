# 初始化项目

从 alpha.8 开始，npm 安装的 CLI 支持交互式和参数式初始化。

## 新项目交互初始化

```sh
mkdir my-project
cd my-project
spec-autonomous init
```

按方向键选择 OpenSpec / Spec Kit、Codex / Claude Code，以及是否添加 MCP 配置，按 Enter 确认。所有选项收集完后，CLI 自动补齐缺失工具，调用所选框架的原生初始化，再生成项目结构。中英文跟随 CLI 界面语言。Ctrl-C 或 Ctrl-D 取消时返回 130，不安装框架、不修改项目。

已有框架或只有一个宿主目录时直接复用检测结果；同时存在多个框架时交互选择。传入的参数优先，向导只补问缺失项。完整参数不会再提问。`--interactive` 可强制进入向导流程，例如为已识别的项目补选 MCP。

## 参数与默认值

```sh
spec-autonomous init --provider openspec --agent codex --mcp --non-interactive
spec-autonomous init --provider speckit --agent claude --mcp --non-interactive

# 新项目采用 OpenSpec、Codex 和 MCP 默认配置
spec-autonomous init --yes
# 默认配置但只安装 Skills
spec-autonomous init --yes --no-mcp

# 脚本或 CI：结构化结果
spec-autonomous init --provider openspec --agent codex --non-interactive --json
```

`--non-interactive`、CI 环境及 JSON/TOML 输出不会自动提问；缺少框架或宿主选择时返回明确错误。`--interactive` 需要真实终端，不能与结构化输出、`--yes` 或 `--non-interactive` 同用。`--yes` 不会覆盖既有或不完整的框架，也不会替用户解决框架歧义。

## 生成结构

以 OpenSpec + Codex 为例：

```text
.git/                                  # 不在 Git 中时创建；不自动提交
openspec/                              # 原生 OpenSpec 初始化文件
.agents/skills/                        # 原生框架 Skills 与 Spec Autonomous Skills
.spec-autonomous/
  config.toml                          # 执行、宿主与验证配置
  skills-installed.toml                # 已安装 Skills 所有权
  milestones/                          # 里程碑声明
  plans/                               # 编排计划
  archives/                            # 归档记录
  mcp-installed.toml                   # 选择 MCP 时创建
.codex/config.toml                     # 选择 Codex MCP 时添加受管理条目
.gitignore                             # 规范声明纳入版本管理
```

Spec Kit 使用其原生 `.specify/` 结构；Claude Code 的命令写入 `.claude/commands/`，MCP 写入 `.mcp.json`。运行账本在首次需要时建立，roadmap 在规划里程碑时生成。

初始化可以重复运行。已有规范、用户配置和其他 MCP 服务器保持原样；自有 Skills 根据所有权清单更新。初始化前先预检配置和文件冲突，冲突时保留原文件；可使用 `--prefix sa` 避免 Skill 名冲突。

成功提示会显示实际路径和入口。检查 `config.toml` 的验证命令、提交初始化文件后，在 Codex 使用 `$autonomous` / `$auto`，或在 Claude Code 使用 `/autonomous` / `/auto` 开始规划里程碑。

## 实现与验证

交互采用 [Inquirer 的公开 prompt 接口](https://github.com/SBoudrias/Inquirer.js/tree/main/packages/prompts)，选择器、取消处理和输入输出与业务初始化分离。Clap 维护原生命令定义，Commander 消费元数据；终端提示走 stderr，JSON 保持独立。

```sh
node --test packages/cli/test/init-options.test.mts tests/e2e/init.test.mts
# 显式运行真实原生工具；可能下载工具
node scripts/test-init-real.mts --run
```

POSIX 键盘 E2E 使用 Python 标准库 PTY 测试夹具；测试夹具不随 npm 包分发。实际平台验收见 [初始化验收](validation/interactive-init.md)。
