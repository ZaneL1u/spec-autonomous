# 初始化验收

交付：alpha.8。实际平台：macOS arm64。交互使用 Inquirer 8.7.2；Node 支持范围与依赖一致为 ^22.13.0 或 >=23.5.0，Bun 1.4.2。

- 单元测试覆盖缺失值选择、显式参数、检测复用、默认值、歧义、CI/结构化模式及取消传播。
- Rust 预检验证无写入、声明目录冲突和原有 Skills/MCP 所有权保护。
- CLI E2E 从空目录生成 Git、原生框架、config、Skills、声明目录；重复运行逐字保留用户配置。
- Python 标准库 PTY 驱动真实 Inquirer 选择器：方向键选择 Spec Kit / Claude / MCP，检查生成结果；Ctrl-C 与 Ctrl-D 取消均退出 130，目录保持为空。
- 真实 OpenSpec 1.13.0 / Codex 和 Spec Kit 1.0.6 / Claude 初始化成功；使用已有经版本探测的隔离工具缓存，原生初始化与后续重复运行均非 mock。下载与缺失运行时安装由既有 provider bootstrap 单元及 E2E 覆盖。

复现：`node --test packages/cli/test/init-options.test.mts tests/e2e/init.test.mts`；真实工具验收使用 `node scripts/test-init-real.mts --run`。日志与真实测试仓库路径记录于本机 `.artifacts/interactive-init-*`。

完整仓库门禁通过：137 项 Rust 测试（含真实 OpenSpec 合约）、56 项 JS/发布测试、80 项 E2E；strict specs 9 项。安装包使用 Node 22.23.2 与 Bun 1.4.2 分别完成真实 OpenSpec/Spec Kit 参数式初始化，并各通过 7 项安装后初始化 E2E，包含真实终端选择和 Ctrl-C/Ctrl-D 取消。

发布产物 SHA256：

```text
41674bae7d155f326b33fcc0bbd45cf09ea35ffc18a009d069edd175fddc2cfa  spec-autonomous-0.1.0-alpha.8.tgz
944a50f91639b67c827a8238149c03260ace4703eefa97c3f48614de425aad02  spec-autonomous-darwin-arm64
```

Windows/Linux 实机初始化未在本轮验收；实际终端和原生工具证据限 macOS arm64。

私有 GitHub Release `v0.1.0-alpha.8` 已发布，实现提交 `de00e05efc87f18dcc2a403fc6d5d1143d3d7854`。使用独立 npm prefix/cache 和空二进制 cache，执行 Git SSH 标签安装；首次运行下载并校验远端二进制。Git 安装后的 Node 22.23.2 和 Bun 1.4.2 分别通过真实两种框架初始化，以及 7 项初始化 E2E（含 PTY 选择与取消）。
