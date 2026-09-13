# CLI 国际化

CLI 支持 English 和简体中文 `zh-CN`。命令名、选项名、状态值、JSON 字段、错误 code、MCP tool 名和 Markdown/TOML 数据格式保持英文稳定；帮助描述、human 状态标签和产品错误 message 随语言切换。Provider 原生 stdout/stderr 原样透传。

## 语言选择

语言优先级为：显式 `--lang`、`SPEC_AUTONOMOUS_LANG`、`LC_ALL`、`LC_MESSAGES`、`LANGUAGE`、操作系统首选界面语言、`LANG`，最后是 Node `Intl` locale。`zh`、`zh-CN`、`zh_CN.UTF-8`、`cmn-Hans` 都选择简体中文；未知语言、`C` 和 `POSIX` 回退 English。`LC_ALL=C` 按显式覆盖处理，会强制英文。macOS 读取 `AppleLanguages`，Windows 读取 `Get-UICulture`，超时或不可用时继续回退；系统界面为中文、终端格式为 `LANG=en_US.UTF-8` 或 `LANG=C.UTF-8` 时仍显示中文。Linux 使用上述环境变量。

```sh
# 跟随系统语言
spec-autonomous --help

# 一次性覆盖系统语言
spec-autonomous --lang zh-CN --help
spec-autonomous --lang en-US providers ensure --help

# 项目或 CI 固定语言
SPEC_AUTONOMOUS_LANG=zh-CN spec-autonomous progress
```

`--lang` 必须放在 npm CLI 的参数区；`providers exec ... --` 之后的 `--lang` 属于原生 Provider，不改变 Spec Autonomous 的界面语言。

## 双运行时实现

`packages/cli/locales/en.json` 与 `zh-CN.json` 是唯一的审查资源，使用稳定 key。Rust Clap 在 `cli-metadata describe` 和直接 native `--help` 前嵌入并应用 catalog；Node Commander 消费已经本地化的 Clap schema，并从相同 catalog 补充 provider 文案。缺失 key 只回退 English，不联网。

这是成熟 gettext / Fluent 工作流的核心原则：稳定 message id、locale 协商、回退和资源与代码分离。当前选择 JSON 是为了让 Rust binary 和 Bun/Node npm 包共享同一资源，不引入额外运行时依赖；key 设计可在未来迁移到 Fluent `.ftl`。

## 机器接口

```json
{
  "schema_version": 1,
  "error": {
    "code": "invalid_path",
    "message": "仓库路径无效: ..."
  }
}
```

同一个错误在 English 与中文下的 `code`、exit code、schema 和字段完全一致，只有 `message` 改变。MCP 的 JSON-RPC framing、方法、tool schema 和资源 URI 不翻译，确保 LLM host 不依赖自然语言解析。

## 测试

```sh
cargo test -p spec-autonomous-cli --locked
node --test packages/cli/test/locale.test.mjs tests/e2e/cli-framework.test.mjs
LC_ALL=zh_CN.UTF-8 target/debug/spec-autonomous --help
```

CI 检查两份 catalog 的 key 集合相同；Node、Bun、native 和安装后的 npm 包覆盖环境检测、显式覆盖、fallback、help、human output、JSON error 与无副作用行为。

## alpha.7 修正

alpha.6 的自动检测只覆盖终端 locale，且遗漏 Commander 标题、参数错误和初始化错误。alpha.7 增加系统界面语言检测，补齐帮助标题、默认值、可选值、原生错误目录、安装及下载提示。通过 Commander 的公开 `configureHelp` 接口和 Clap 命令定义渲染，不对最终输出做全局字符串替换。

进度仅翻译已知状态字段；路径、任务名称、用户文本和结构化数据保持原值。上游 OpenSpec / Spec Kit、npm、gh 及操作系统自身的输出保留原文，以免丢失诊断细节。

语言选择参考 [Apple 首选界面语言](https://developer.apple.com/documentation/foundation/locale/preferredlanguages) 与 [GNU gettext 环境变量](https://www.gnu.org/software/gettext/manual/html_node/The-LANGUAGE-variable.html)；本项目明确采用上述跨平台应用优先级。帮助扩展遵循 [Commander 公开帮助接口](https://github.com/tj/commander.js/blob/master/docs/help-in-depth.md)。
