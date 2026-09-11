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
5. 使用已准备的 publish-npm.yml，并在真实 npm/GitHub 账号中绑定 trusted publishing / provenance（账号侧尚未配置）；先发布六个平台包并确认 registry 可取，再发布 wrapper 到 `next`，验收后提升稳定 tag。
6. 任一步失败，不发布指向缺失平台依赖的 wrapper；同版本不可变，修复用新版本。回退通过恢复 dist-tag 指向已验收版本。

发布脚本和工作流已经实现并通过离线测试；真实发布和远程 CI 仍需目标仓库/账号环境。本机打包成功不等于六平台或 registry 发布完成。

## 产品 skills 与结构化资源

wrapper 已包含 milestone/autonomous/auto/progress/resume 五个 SKILL.md。npm files 白名单与 release assembler 复制清单已同步，真实 npm tarball 测试验证入口/别名均随包交付；原生上游框架、研究 clone 和 mock fixtures 不进入产品包。

用户安装 CLI 后在仓库执行 `spec-autonomous init`，检测其现有 SDD 和宿主，绑定 /autonomous、/auto 等入口；高级配置可用 `skills install --agent <id> --scope project`。不使用 npm postinstall 猜仓库路径。安装/升级/卸载按自有 hash manifest 操作，冲突不覆盖；仅 skills 宿主显示其等价语法。skills 调用同一 Rust CLI schema，TOML 模板如随包提供也进入组包校验。

## 发布前校验与受信任发布

`node scripts/publish-release.mjs --input .artifacts/npm` 默认为离线 dry-run：检查七个真实 tarball、SHA256SUMS、版本一致、精确 optionalDependencies、无 lifecycle scripts、二进制格式/架构和 SHA512。文本平台 fixture 会被拒绝；header 检查仍不能取代真实原生运行。

只有显式 `--publish --tag next` 才接触 registry：先检查全部既存版本，只有 integrity 相同才复用；逐一发布六个平台并等待 registry 可见，重新核对六包后才发布 wrapper。任一错误阻止 wrapper，部分已发布平台版本保留并可在相同 bytes 下续接，不自动回滚已发布的不可变版本。

`publish-npm.yml` 默认 dry-run，校验输入来自同仓库、默认分支、同一 head SHA 的成功 release-artifacts run，并下载唯一对应 artifact。发布 job 使用 npm-release environment 和 OIDC/provenance；维护者仍需配置保护规则、七个包的名称所有权/Trusted Publisher，然后设置 environment variable `NPM_RELEASE_READY=true`。声明 environment 名字不等于保护规则已经存在。本地没有触发工作流或发布。

release-artifacts 的 assembler 根据实际 `GITHUB_REPOSITORY` 把同一个真实 repository.url 写入全部七包；本地也可显式传 `--repository https://github.com/<owner>/<repo>`。没有仓库来源时不虚构 URL。升级稳定 tag 或回退 tag 均由维护者基于已完成的 registry 安装 smoke 执行，尚未演练真实 tag 变更。

## alpha.2 host-driven 入口

平台分包与 launcher 机制保留。Rust CLI 现在仅提供确定性能力与 stdio MCP，不启动 agent；同一包分发五个更新后的 Skills。独立测试宿主和语义 mock 不进入 npm files 白名单。root/CLI Cargo/npm 版本统一为 0.1.0-alpha.2；运行账本 schema 为 2，公开能力 envelope 与工作回执 schema 为 1。
