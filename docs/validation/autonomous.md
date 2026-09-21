# Autonomous runtime 本地验收

日期：2026-09-11。版本：`0.1.0-alpha.1`。本轮完成本地编排实现、产品 skills、mock 仓库、测试体系及 npm 发布准备；没有执行真实 npm 发布。OpenSpec change 仍保持活动状态，剩余外部验收没有勾选或归档。

## 验证环境与结果

macOS 26.6.2 arm64；Rust 1.98.1；Git 2.50.1；Node 24.21.0 / 22.23.2；npm 11.19.0；Bun 1.4.2。OpenSpec 1.13.0；Spec Kit 1.0.7.dev0（锁定源码构建并安装原生 Codex skills）；真实 runner 为 Codex CLI 0.153.4。

| 检查 | 结果与实际边界 |
| --- | --- |
| Rust formatting / clippy | 通过，`-D warnings` |
| Rust unit / public contracts | 118 项通过；包含 parser/CAS、DAG/range、配置、Git/SQLite、100 个 fresh subprocess、取消/进程树、skills、只读 progress；独立 OpenSpec native contract 另 1 项通过 |
| Node launcher / packaging / publishing | 24 项通过；发布测试使用注入 npm，未真实 publish |
| Git/process e2e | 36 项通过；使用真实临时 Git 仓库、worktrees、SQLite、进程及 Node 断言；仅 agent 决策由 deterministic mock 替代 |
| OpenSpec strict validation | autonomous change 与两个主规范共 3 项通过 |
| Skill 格式 | autonomous / auto / milestone / progress / resume 五项 quick_validate 通过 |
| 实际 npm tarball | release binary 与五个 skills 已打包；含空格 prefix、`--ignore-scripts` 安装通过 |
| 安装后的 CLI e2e | OpenSpec 完整闭环、Spec Kit goal→roadmap→开发、only/from/to 接续三项通过 |
| 分离 wrapper/platform | 离线安装真实 darwin-arm64 平台包和 wrapper 后 version/detect 通过；其余五项仅为不可发布的组包 fixture |
| Bun 与 Node 22 | Bun 在临时目录 `add --ignore-scripts` 后 CLI 可用；Node 22 launcher/包契约与安装包 version 通过 |
| 远程 CI / npm registry | 未执行；工作流存在不等于平台或发布验收通过 |

完整测试入口为 `node scripts/test-all.mts`，等价于 `bun run test:all` 或 `npm run test:all`。记录分别位于 `.artifacts/final-checks.log`、`.artifacts/acceptance-final.log`、`.artifacts/provider-final.log`、`.artifacts/e2e-decomposition.log`、`.artifacts/e2e-provenance.log`、`.artifacts/node22-final.log`。最终完整套件记录 118 个 Rust tests、1 个显式真实 OpenSpec 契约、24 个 Node tests 和 36 个 e2e 全部通过；忽略的 subprocess 辅助入口不计入通过数。

本机曾观察到直接打开 `/Users/zaneliu/Documents` 目录阻塞：Bun 向上查找项目时卡在 `openat`；Node 仅打开该目录也会阻塞，但直接访问仓库、执行 Node 测试驱动正常。TCC preflight 是相关日志，系统根因尚未确认。没有修改系统权限，挂起探针已清理。Bun tarball 安装在独立临时目录完成。

## 真实 agent 验收

这些是实际已登录的 Codex 进程，不属于 mock e2e；不应推广为所有模型、框架版本或平台的保证。

| 场景 | Run / 结果 | 独立 session / 耗时 |
| --- | --- | --- |
| OpenSpec 从目标规划并实现 addition | `run-f37ce5d5269d4d1cb38a9dffcbf12755`，completed，accepted `8a8b7e01823c1a0349ae52a598b221def2c6af42` | 9 个 fresh sessions，562,093 ms，5 条 host 验证全部通过 |
| Spec Kit 已有原生 feature 接续 | `run-282118532cd548bd9574aacda3b694d9`，completed，accepted `7fcc675f1c89072c732eaf264d6916baa57d08b6` | 4 个 fresh sessions，184,862 ms，5 条 host 验证全部通过 |
| 产品 `$auto` 入口与阶段边界 | `run-691b0cf250284a9097e3b093d1e09e7c`，scope_completed；P002 未触碰 | 真实父 Codex 调用已安装 skill，子 worker 为 mock；不把它记作全真实模型链 |
| 产品 `$autonomous` → 原生 Spec Kit 从目标 → 实现 → audit | `run-08b1db0bf2be4aeabd0d955b51fbd481`，completed，accepted `f0b117911d1186ca1f56e55dddccf5a4d6620034` | 9 个唯一真实 worker session（8 次接受/集成、1 次审查阻塞），累计 1,650,533 ms；16 条 host 验证全部通过 |

最后一项通过正式安装的 Spec Kit integration 读取原生 specify/plan/tasks 指令，生成 `specs/addition-native` 与 research/data-model/contracts/quickstart/checklist，只有 `src/add.mjs` 进入产品实现，既有测试字节保持不变。初次最终审查要求 fresh-worker 过程证明，但 verifier 尚未得到 ledger 证据，因此正确返回 needs_input。补入通用 `execution-evidence.json` / SHA256 / 日志与 session 引用后，**恢复同一个 run**，新增一次审查通过，未重做已完成实现。此记录不是“首轮无中断通过”的声明，也没有通过删减目标获得通过。

`skill-entry` 首次在父 Codex workspace-write 模式下无法创建 Git runtime，未产生 run；随后在已授权的一次性测试仓库中使用允许 Git 写入的父测试环境通过。产品 Codex workers 仍按角色使用 read-only / workspace-write；worktree 本身不等于 OS 沙箱。

原始可追溯记录：`.artifacts/real-codex.ndjson`、`.artifacts/real-speckit.ndjson`、`.artifacts/skill-entry-events.ndjson`、`.artifacts/real-speckit-skill-events.ndjson`、`.artifacts/real-speckit-goal-resume.ndjson`、`.artifacts/real-speckit-goal-report.json`，以及各 fixture 的 Git common-dir ledger、attempt 文件与 report。日志可能含原始工具输出，保留在本机并排除在 Git/npm 外。

## 并发、恢复与上下文

- 同样六个独立 `[P]` 任务，每 worker mock 计算等待 600 ms。独立调度测试记录单 worker 13.391 秒、三个 workers 9.562 秒；实际重叠峰值分别为 1 和 3，每轮 29 条 host 验证。观察加速比约 1.40；正确性不依赖速度阈值，不是对真实模型耗时的保证。完整套件中的另一次观测有不同墙钟，均保留 dispatch timestamps。
- 验证了写集/读写冲突、未知写集串行、连续补位、依赖只读取 accepted revision，以及父任务拆分后所有子任务集成才写回一个原生 checkbox。
- 覆盖六个 debug crash windows：candidate patch、source writeback、intent finalize 之前、candidate commit 之后、Git integration advance 之后、origin advance 之后；resume 不重复接受 task。另覆盖幂等/非幂等 hook 崩溃，未知副作用保留明确恢复门。
- 验证实际进程子孙回收、暂停同步 audit、全 run 截止时间、相同失败跨 resume 的无进展停止；独立任务可继续完成。测试命令修改验收对象，即使 exit 0 也拒绝采纳。
- 40 项源任务使用五个 fresh planner batches（每批 8），全图覆盖和跨批依赖经过校验。该场景证明规划拆分，不声称 40 个真实模型任务已执行。
- 超过 16 KiB 内联预算的测试源通过不可变完整上下文文件传递，SHA256 和末尾规则保留；progress 仍显示原生全量 task 数。限制的是 packet/prompt bytes，不能据此承诺模型读取后的总 token 上限。
- `execution-evidence.json` 绑定 run、accepted commit、真实 attempts、native session header 和 verification，审查可以核实过程性需求；公开索引不包含 runner 环境值。

## 留在本机的 mock 仓库

可查看已完成的两阶段 OpenSpec mock：`.artifacts/mock-repositories/openspec-verified`，run `run-f6e21787c48845e8b5b2fd33061c59ed`，add/multiply/service 全部集成。真实 agent 仓库分别为 `real-codex`、`real-speckit`、`real-speckit-goal`；每个都保留完整 Git/worktree/runtime 供检查。

创建自己的全新 mock（目标目录须不存在）：

```sh
node scripts/create-mock-repo.mts --framework openspec --output .artifacts/my-mock --fail-once add
node packages/cli/bin/spec-autonomous.mjs run --path .artifacts/my-mock --milestone M001 --json
node packages/cli/bin/spec-autonomous.mjs progress --path .artifacts/my-mock --all-worktrees
```

测试结构和可注入边界见 [testing.md](../testing.md) 与 [architecture.md](../architecture.md)。

## OpenSpec 完成边界

本地 milestones、两种 native bridge、隔离并发、CAS、验证/repair/recovery、TOML/JSON/progress、skills 与 npm 组包已经实现。保留以下验收项未完成：其他平台的原生进程回收/安装/最低 OS 测试（3.7 / 5.2）、npm 名称所有权（5.1）、全平台真实 release tarballs 下载后安装（5.3）、账号侧 trusted publisher/保护环境与真实发布/回退（5.4）、包含这些外部条件的整个 M4 验收及归档（5.5）。

发布准备已实现：14 项注入测试覆盖平台先发布、registry integrity/visibility barrier、部分失败禁止 wrapper、同 bytes 复用。`publish-npm.yml` 默认 dry-run；本地没有配置或触发真实远程发布。profile 的 push/publish/archive 默认并限定为 false；需要这些动作时使用独立原生/发行流程，不把不可用能力伪装成已执行。

## 最终本机安装产物

- Tarball：`.artifacts/local/spec-autonomous-0.1.0-alpha.1.tgz`，2,680,202 bytes，SHA256 `a52ba85ade19e2f6ae5339abd70b74f6edae08bfe00980c75106ea8b493fcdd9`。
- darwin-arm64 binary：5,568,976 bytes，SHA256 `0575234db36fef3be48444c26b16a0ff03df5162bcf902c5f88cf05b5ce18687`。
- 最新空 prefix 安装位于 `.artifacts/final npm install`，`--ignore-scripts` 下 version/doctor 通过。Mach-O arm64 与本机 dynamic linkage 已检查；没有由此推断最低 OS 或其他架构可用。
