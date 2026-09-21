# Provider bootstrap 验收

日期：2026-09-11。平台：macOS arm64。交付版本：0.1.0-alpha.3。

## 最终结果

| 检查 | 结果 |
| --- | --- |
| cargo fmt / clippy -D warnings | 通过 |
| Rust 单元及合约 | 129 项通过；另有 1 项真实 OpenSpec 合约通过 |
| JS / npm 组包及发布合约（Node 22.23.2） | 最终 38 项全部通过 |
| 完整 CLI / Git / SQLite / MCP e2e | 61 项全部通过 |
| 已安装 alpha.3（禁 lifecycle scripts，Node 22） | 5 项 provider e2e 全部通过 |
| Node 与 Bun 真实依赖安装 / 两种原生 init | 通过；uv 与 Python 均实际下载验证 |
| 已安装 CLI 的离线原生 init / MCP 绑定 | OpenSpec、Spec Kit 均通过 |
| OpenSpec strict validation | 5 项通过 |
| npm tarball / 当前源码 / 安装后文件 | 18 个文件逐字节一致 |

最终包：`.artifacts/local/spec-autonomous-0.1.0-alpha.3.tgz`，约 3.1 MB，仅 macOS arm64。
SHA256：`7d3b517c2b1d9b1c6cc1633bfa0db2c4b3ec3bf55bb975006f9ad92d1699f891`。
安装包复验证据为 `.artifacts/provider-bootstrap-installed-final.log` 与 `.artifacts/provider-bootstrap-package-final.json`；最新 JS 合约为 `.artifacts/provider-bootstrap-node22-package-final.log`。

## 真实上游安装

`node scripts/test-providers-real.mts --run` 强制禁用已有 PATH 工具的复用，在独立临时 provider home 安装：

- OpenSpec 1.13.0：npm 安装、版本探测、原生 init、框架检测和 Skills 绑定通过。
- uv 0.12.13：官方 macOS arm64 archive 下载、内置 SHA256 校验、解包与版本探测通过。
- Python 3.14.7：`UV_PYTHON_PREFERENCE=only-managed` 强制覆盖没有合适 Python 的路径，实际下载到专用目录。
- Spec Kit 1.0.6：官方 PyPI 安装、版本探测、原生 init、框架检测和 Skills 绑定通过。
- 两种框架的重复 init 复用已验证安装，没有改写原有规范。

Node 实测记录为 `.artifacts/provider-bootstrap-real.json` / `.log`。同一脚本和 JS 模块复制到临时运行目录，用 Bun 1.4.2 实际运行通过；OpenSpec 确实走 `bun add`，记录为 `.artifacts/provider-bootstrap-bun-real.json` / `.log`。临时目录运行避免此前 Documents 路径下 Bun 的挂起问题；没有修改系统隐私权限。

## 合约与回归范围

- JS 合约：缺失依赖、复用、显式配置、损坏命令、并发安装、失败后重试、离线、不匹配 checksum、安装锁与恢复锁、超时与取消、scaffold 冲突、原生 argv 和 run 的框架继承。
- CLI/MCP e2e：缺失 OpenSpec 时自动安装、保持 Markdown、providers exec 的参数及 exit 7、只读无下载、歧义与显式选择、MCP 首次 prepare 自动安装、失败不发布 ready receipt。
- Node 22 的 npm 包安装使用 `--ignore-scripts`，执行实际打包的 launcher 和 native 二进制。
- 已验证缓存的离线验收：安装后的 Node 22 CLI 在两个空 Git 仓库分别完成 OpenSpec / Spec Kit 原生初始化、MCP 绑定、status 和 native version。
- Bun frozen lockfile 和五个 Skills 的 quick_validate 通过。

全仓回归首次暴露既有的暂停 mailbox 竞态：协调锁占用时 pause 未及时写入 run 状态，apply-result 保存 submitted 回执后仍返回 awaiting_host。新增确定性 Rust contract 在修复前失败、修复后通过；修复仅确认暂停状态并保留已提交回执，不丢结果或重新派发工作。

运行命令和日志：

```sh
node scripts/test-all.mts
# .artifacts/provider-bootstrap-full-final.log
node --test packages/cli/test/*.test.mts scripts/test/*.test.mts
# Node 22: .artifacts/provider-bootstrap-node22-package-final.log
node scripts/pack-local.mts
# .artifacts/provider-bootstrap-pack-final.log
```

## 平台与发布边界

只实测 macOS arm64。Windows / Linux / macOS Intel 的下载映射和 SHA256 已配置，实际运行需对应机器或 CI 验证。没有调用真实模型，没有 CLI agent launcher，没有向 npm registry 或任何远程仓库发布。首次安装需要网络；离线且依赖缺失时返回明确错误。
