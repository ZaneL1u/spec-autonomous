# 长任务可靠性修复验证

实现提交：`0c2e344`，分支 `codex/long-running-reliability`。

日期：2026-10-13。平台：macOS arm64。Node.js v26.5.0，Bun 1.4.2，Rust 1.98.1。

## 已验证

- 完整 `bun run test:all` 返回 0：TypeScript 检查、生成产物、Rust 格式和 Clippy、187 项 Rust 测试、57 项 JavaScript/打包测试、83 项端到端测试、23 项严格 OpenSpec 校验通过。
- Rust 汇总有两个 ignored 条目：真实 OpenSpec 合约随后由完整驱动显式执行并通过；另一个是仅供子进程调用的入口。
- `bun run pack:local` 返回 0，使用独立 Cargo target 和两路编译，未覆盖系统安装或二进制缓存。
- 本机包以 `--ignore-scripts` 安装到独立前缀；安装后检查版本、能力 schema、新 CLI 参数、关联仓库去重、同版本 skill 漂移，以及只读查询不创建账本，全部通过。
- 使用新 CLI 对运行中的旧 alpha.13 账本做了只读 progress 兼容检查，能同时列出主仓历史 run 与独立执行仓库当前 run；未执行迁移、resume、cleanup 或替换 MCP。

## 回归覆盖

- 活跃清理保留所有者、恢复用 accepted 工作树、未完成任务失败补丁、当前包引用、ignored 产物与持久日志。测试真实 Git 清单、协调器锁、预览哈希以及外部集成树保护。
- cleanup 的 delete_integration 从能力分发正确传递，安全范围纳入预览。
- 每个独立验收波次拥有全新目录和身份，准备钩子先于检查执行；准备失败不会运行检查或标记完成。
- 原始 stdout/stderr 与哈希、失败分类和耗时可定位；失败信息不直接复制子进程任意输出。
- 重复 argv/cwd 同波次去重；不同波次仍独立执行。原 run 验证命令修订后保留已验证任务，重新规划保留已审查检查。
- 核对并保留 dogfood-scope-and-closeout 修复，补充“后来的 submitted 不能被更早成功隐藏”和旧来源回执保护。
- 进度能看到尚无 run 的原生勾选，且不推断验收通过；独立仓库显式关联、分页与只读性得到验证。
- 全套测试发现并修复 Homebrew npm 符号链接查找、验收父目录创建、Clap 本地化后参数索引及重复参数元数据问题。

## 本机产物

候选包：`.artifacts/local/spec-autonomous-long-running-88e6ab6043cd.tgz`。

SHA-256：`88e6ab6043cdedd20d476206f1890dc901a996b754bb6497a95595890b8bcd56`。

包内部仍标为 `0.1.0-alpha.13`，这是基于该版本的本机候选，不是新的远端 Release。必须以包哈希和源码提交识别，不能将版本字符串当作已安装此修复的证明。安装路径和切换步骤见[长任务维护与安全更新](../long-running-runs.md)。原始本机验证日志在 `.artifacts/reliability-validation/`，不进入源码版本控制。

## 实际边界

没有升级、清理或改写消费项目，没有替换全局 CLI、缓存二进制或正在运行的 MCP。未发布远端安装包。没有执行 Windows/Linux 原生验证，不据 macOS 测试宣称其他平台已验收。

新包可以继续原 run；旧脚本的固定报告路径、VM/数据库准备与销毁仍需项目宿主在安全交接点显式接入。不会自动重写验收条件、重置数据库、跨波次复用 PASS，也不承诺大项目安装后立即完成。
