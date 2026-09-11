# Rust + Bun/npm 发行设计

## 当前交付

开发依赖由 Bun workspace 管理，Rust workspace 编译独立 `spec-autonomous` 二进制。
Node.js launcher 负责平台选择、argv/stdio/退出码与 SIGINT/SIGTERM 转发，不承载调度逻辑。
发布包要求 Node.js 22+；Rust/Bun 只在源码开发或发布构建时需要。

`bun run build` 在本机生成 `packages/cli/native/<platform>/<binary>`。
`bun run pack:local` 将该二进制与 launcher 打成 `.artifacts/local/*.tgz`，用于本机 npm 安装验收。
**本地 tarball 不可作为通用跨平台发布包。**

## 正式包结构

| npm 包 | Rust target | runner |
| --- | --- | --- |
| spec-autonomous | 纯 Node launcher，精确版本 optionalDependencies | 组包 job |
| spec-autonomous-darwin-arm64 | aarch64-apple-darwin | macos-14 |
| spec-autonomous-darwin-x64 | x86_64-apple-darwin | macos-15-intel |
| spec-autonomous-linux-arm64 | aarch64-unknown-linux-gnu | ubuntu-22.04-arm |
| spec-autonomous-linux-x64 | x86_64-unknown-linux-gnu | ubuntu-22.04 |
| spec-autonomous-win32-arm64 | aarch64-pc-windows-msvc | windows-11-arm |
| spec-autonomous-win32-x64 | x86_64-pc-windows-msvc | windows-2022 |

包名是暂定命名；本次未建立 npm 账号/组织、未预留这些名字、未发布。
runner 标签依据 [GitHub 官方清单](https://docs.github.com/en/actions/reference/runners/github-hosted-runners)；实际额度和可用性需在目标远程仓库验证。
Linux 先支持 glibc，目标构建基线 Ubuntu 22.04；musl/Alpine 返回明确错误。最终最低 glibc/macOS/Windows 版本以对应平台运行结果和 binary linkage 检查确定，不用一次编译成功推断。

`scripts/package-release.mjs` 要求 `.artifacts/binaries/<target>/<binary>` 六项全部存在且非空，再生成 `.artifacts/release/`：平台包带 `os`、`cpu`（Linux 带 `libc`）约束，wrapper 指定所有 optionalDependencies 的**完全相同精确版本**，并附原生文件 SHA256SUMS。
输出目录必须不存在，避免把旧版本二进制混入新包。脚本不会执行 publish；它校验结构和缺失文件，不能证明输入二进制的架构、来源或可运行性，这些由构建/安装 CI 验证。

不使用安装脚本临时从 GitHub 下载二进制：npm 自带包完整性校验和平台依赖选择，禁用 lifecycle scripts 的安装也应工作。参考 [npm package.json 的 bin、os、cpu、libc 与 optionalDependencies](https://docs.npmjs.com/cli/v11/configuring-npm/package-json/)。Bun workspace 的选择依据 [Bun 官方文档](https://bun.sh/docs/pm/workspaces)。

## 发布步骤（待远程发布时执行）

1. 对齐根 package.json、packages/cli/package.json、Cargo workspace 版本及 locks；检查 npm 名称/组织所有权。
2. 全部测试、strict spec validation 通过；执行 `release-artifacts.yml` 六平台原生构建与 CLI smoke。当前 workflow 为手动触发，组包后先生成 tgz 再上传，以保留 Unix binary 权限，尚不自动 publish。
3. 组包后逐包 `npm pack --dry-run` 检查内容，生成真实 tarball；排除源码 clone、runtime state、私钥、测试 fixture。保存 tarball integrity 和对应 Git commit。
4. 在各原生 runner 的空目录安装匹配平台包和 wrapper tarball，`--ignore-scripts` 模式下运行 version/detect；另验证 Bun 全局安装、缺失 optional dependency 的错误、Node 22。
5. 使用 npm trusted publishing / provenance 绑定受信任发布 workflow（尚未配置）；先发布六个平台包并确认 registry 可取，再发布 wrapper 到 `next`，验收后提升稳定 tag。
6. 任一步失败，不发布指向缺失平台依赖的 wrapper；同版本不可变，修复用新版本。回退通过恢复 dist-tag 指向已验收版本。

发布和远程 CI 属于后续工作，本次不会把本机打包成功描述成六平台和 registry 发布完成。
