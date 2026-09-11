## 1. Research and tooling

- [x] 1.1 克隆并锁定 OpenSpec、Spec Kit、GSD Pi/Core；用 references:clone 核验 commit/remote/clean status 并提交研究报告。
- [x] 1.2 初始化 Rust/Bun/npm workspace、固定工具链与 locks；验证依赖可安装且 Rust workspace tests 通过。
- [x] 1.3 通过官方 OpenSpec CLI 初始化 spec-driven 和 Codex skills；验证两个 change 的目录及 metadata 存在。

## 2. CLI and packaging

- [x] 2.1 实现只读检测和 JSON/错误输出；验证最近根、Git 边界、混合框架、引用目录、symlink 与 partial setup 测试。
- [x] 2.2 实现 npm launcher、平台识别、参数/退出码/信号转发；验证 Node tests 通过。
- [x] 2.3 实现本地与 release 组包、CI artifact 配置；验证缺产物拒绝、精确依赖和打包内容测试。
- [x] 2.4 在本机生成并于隔离 prefix 安装真实 npm tarball；验证 --version、detect、无 lifecycle scripts 和包内无研究源码。

## 3. OpenSpec plan and acceptance

- [x] 3.1 完成自主里程碑 proposal/design/specs/tasks、README 和发行说明；运行 OpenSpec strict validation 并记录结果。
- [x] 3.2 运行 fmt/clippy、Rust/Node tests、打包安装 smoke、引用链接检查；记录本机通过范围与未验证平台。
