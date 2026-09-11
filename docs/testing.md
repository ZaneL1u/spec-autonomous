# 测试与验证

本项目的测试要证明一次启动能够推进完整里程碑或选定阶段范围，并在失败、暂停、源码漂移和进程中断后保持完成证据可信。测试分开验证生产 Rust 控制流、真实原生规范接口、npm 包装，以及使用确定性 agent 的完整执行闭环。

本文记录当前测试结构和复现方法，不把 CI 配置、模拟 worker 或测试文件存在视为验收通过。当前测试数量以命令输出为准；OpenSpec 实施清单仍按每项行为和证据逐条判断，不能据此声称全部任务完成。[bootstrap 记录](validation/bootstrap.md) 是早期初始化快照，不能用其中的历史数量描述当前 runtime 验证。

## 环境与完整命令

需要 Git、仓库固定的 Rust toolchain、Node.js 22+、Bun 1.4.2。运行命令前位于仓库根目录。Node e2e 使用 debug CLI；`bun run build` 生成的是发行二进制，不能替代下面的 debug build。

常规入口为 `bun run test:all`，依次运行格式/lint、Rust、显式上游契约、Node 包装、mock e2e 和规范验证。`bun run test` 只覆盖 Rust 与 Node 包装，不包含显式 OpenSpec 契约或全流程 e2e。需要分层排错时使用以下完整命令；`test:unit`、`test:provider`、`test:package`、`test:e2e` 是 [package.json](../package.json) 中对应的脚本入口。

```sh
bun install --frozen-lockfile
bun run check
cargo test --workspace --locked -- --nocapture

# 这个上游集成契约默认标为 ignored，必须明确运行
cargo test -p spec-autonomous-core --locked --test provider_contracts real_openspec_plans_then_accepts_skipped_specs_and_custom_tracking_artifact -- --ignored --exact --nocapture

node --test --test-reporter=tap packages/cli/test/*.test.mjs scripts/test/*.test.mjs
cargo build -p spec-autonomous-cli --locked
node --test --test-reporter=tap --test-concurrency=2 tests/e2e/*.test.mjs

bun run spec:validate
bun run build
node packages/cli/bin/spec-autonomous.mjs detect --json
bun run pack:local
```

Unix 新终端若找不到 Cargo，可先加载 `~/.cargo/env`；Windows 使用已安装的 Rust 工具链。上面的 glob 命令可在 bash/Git Bash 执行，CI 三个平台统一使用 bash。OpenSpec 遥测可以通过环境变量 `OPENSPEC_TELEMETRY=0`、`DO_NOT_TRACK=1` 关闭，CI 已设置。

Spec Kit e2e 使用已提交的 [speckit-native fixture](../tests/fixtures/speckit-native/README.md)：五份未修改的官方 command template，锁定 `github/spec-kit` commit `c173bf19a6654e3b05386ec3599349a55282b897`（研究快照 `1.0.7.dev0`），同时保留 MIT LICENSE。它们只作为测试输入，不是 runtime 的替代模板，不随 npm 包发布；实际用户仓库仍使用用户已安装的原生 skills/templates。

安装锁定的 npm 依赖后，单测和 mock e2e 不需要 `.references/` clone 或模型网络调用。若需重新研究完整上游源码，可另运行 `bun run references:clone`，按 [upstreams.lock.json](research/upstreams.lock.json) 恢复来源、HEAD 和干净状态；这不是 CI 测试前置步骤。

e2e 默认读取 `target/debug/spec-autonomous`（Windows 为 `.exe`）。使用自定义 Cargo target directory 时，显式设置 `SPEC_AUTONOMOUS_TEST_BINARY` 为对应 debug 二进制的绝对路径。崩溃注入依赖 debug build，不要将 release binary 代入这些场景。

## 测试层次与证据边界

| 层次 | 位置与执行方式 | 验证的行为 |
| --- | --- | --- |
| Rust 单元与公共契约 | `cargo test --workspace --locked` | 模型、路径、调度、进程、Git、SQLite 与公开 API 的可观察行为 |
| 配置契约 | [config_contracts.rs](../crates/core/tests/config_contracts.rs) | 用户/项目/default 优先级、数组覆盖、未知预算字段拒绝、错误脱敏及不支持的 lifecycle/溢出期限拒绝 |
| Provider 契约 | [provider_contracts.rs](../crates/core/tests/provider_contracts.rs) | MD provenance、frontmatter、框架 parser 差异、CRLF/Unicode/CAS、原生阶段、路径与协议边界 |
| 执行计划契约 | [plan_contracts.rs](../crates/core/tests/plan_contracts.rs) | 来源覆盖、父子拆分、DAG、阶段约束、文件冲突、稳定 phase 标签、范围与 roadmap 编辑保护 |
| Runtime 契约 | [runtime_contracts.rs](../crates/core/tests/runtime_contracts.rs) | 真实临时 Git/SQLite、事务回滚、仓库锁、进程身份、后代进程清理、取消/超时、结果协议与上下文限制 |
| Progress 契约 | [progress_contracts.rs](../crates/core/tests/progress_contracts.rs) | 任一 worktree 的全仓视图、外部 unknown、无账本只读、坏索引、计数去重、partial 与 accepted/candidate 区分 |
| Skill 安装契约 | [skill_contracts.rs](../crates/core/tests/skill_contracts.rs) | 宿主绑定、别名、冲突保护、更新/卸载和原生流程保留 |
| 真实 OpenSpec 集成 | provider 文件中的显式 ignored 测试 | 安装的 OpenSpec CLI、临时仓库、自定义 artifact/tracking file、`skip_specs`、原生规划和回写后刷新 |
| Node 包装与打包 | [packages/cli/test](../packages/cli/test)、[scripts/test](../scripts/test) | argv、退出码、信号、平台选择、skills 分发、tarball 内容和精确平台依赖 |
| 全流程 e2e | [tests/e2e](../tests/e2e) | debug CLI 经真实工作区、Git、账本、规范文件和验证命令完成规划、执行、修复、范围与恢复 |

Rust runtime 的 subprocess fixture 是独立测试进程；`tests/mock-agent.mjs` 是另一个实现 worker 协议的确定性程序。它们用于让同一输入稳定产生成功、失败、错误结果或挂起，不请求模型 API。测试中的子进程隔离、文件修改、Node 断言、Git 集成和账本读写仍调用生产实现。

真实 OpenSpec 契约使用 `bun install` 获得的固定 `@fission-ai/openspec`，不是伪造 CLI JSON。它证明接口适配；native-planning 内容仍由测试填入，并不证明真实模型能够完成规划。其默认 ignored 是为了显式区分需要 Node/OpenSpec 的集成契约；CI 会按准确测试名执行它，不使用 `--ignored` 扫描运行所有测试辅助入口。

Node 发行测试中的六平台二进制有部分是只验证包装结构的占位文件。它们可证明平台包隔离、版本依赖及 skills 内容，不能证明六个平台都能启动对应原生程序。

## 整里程碑 e2e

[autonomy.test.mjs](../tests/e2e/autonomy.test.mjs) 覆盖 OpenSpec 原生工件规划、fresh worker、可并行任务、失败后修复、完整交付，以及 OpenSpec 和 Spec Kit 分别从目标形成 roadmap 和原生工件。它也检查 from/to/only 范围、范围外依赖拒绝、越界改动不交付、跨 worktree 进度和暂停。

[verification.test.mjs](../tests/e2e/verification.test.mjs) 验证 phase/milestone 检查失败后进入有界补任务与修复，并保存真实失败和成功命令证据；还覆盖外部 plan 带非法依赖时在创建 worktree 前拒绝，以及人工修改原生规范后暂停、提交编辑并恢复。完成断言同时检查最终代码、Git 状态、任务回写和证据，不能仅看 worker 自报 candidate 或退出码为零。

[recovery.test.mjs](../tests/e2e/recovery.test.mjs) 覆盖 native handoff/resume、旧阶段证据失效、mandatory hook 预检查、Spec Kit 路径重映射、运行期限和同步 worker 取消，以及验证命令修改代码或审核报告不覆盖原生要求时拒绝交付。部分进程信号场景明确跳过 Windows；应读取报告中的 skip，不把它们算作 Windows 通过。

[resources.test.mjs](../tests/e2e/resources.test.mjs) 验证资源限制与原生产物路径：超过输入上限的原生上下文保存在带 SHA256 的完整引用文件中，`input.json` 与 `prompt.md` 保持大小限制，尾部规则仍可读取；40 项原生任务按每批最多 8 项分成 5 个 fresh planner 调用，合并后任务及 source ID 完整覆盖，并保留批次间依赖；显式 `cleanup` 只移除可清理的受管理工作区，保留 dirty worker、integration、外部 worktree、分支引用和证据。另覆盖 Spec Kit 原生 planning sidecars 的集成和 feature.json 的原始字节保留。40 项场景验证的是分批规划，不能据此声称已执行完 40 项真实模型任务；上下文文件内容完整也不独立证明模型读取了每条规则。

[hooks.test.mjs](../tests/e2e/hooks.test.mjs) 在 `after_hook` 退出 86 后分别恢复幂等和非幂等原生 hook。幂等 hook 使用稳定 `SPEC_AUTONOMOUS_HOOK_KEY` 避免重复效果；非幂等 hook 必须停在 `hook_outcome_unknown`，经 `resolve-hook --outcome completed --evidence ...` 明确协调结果后再继续。两种路径都检查计数器只有一次效果、范围完成及干净交付，不把“进程可能跑过”猜成成功。

2026-09-11 的本地独立运行日志 `.artifacts/e2e-resources.log` 记录资源测试 3 项通过，`.artifacts/e2e-hooks.log` 记录 hook 测试 2 项通过，均无失败或跳过。这些是对应文件的局部运行证据，不是当前全量套件结果。

可以只跑一个场景，参数必须放在测试文件之前：

```sh
node --test --test-reporter=tap --test-name-pattern='crash recovery' tests/e2e/autonomy.test.mjs
node --test --test-reporter=tap tests/e2e/verification.test.mjs
node --test --test-reporter=tap tests/e2e/resources.test.mjs
node --test --test-reporter=tap tests/e2e/hooks.test.mjs
```

## 真实 Codex runner 验收

以下结果从本地 NDJSON 最后一条 run 状态及各 attempt 的 Codex `thread.started` 事件核对，运行后端为 `runner.profile=codex`，不是 `tests/mock-agent.mjs`。两次运行都记录了 phase/milestone 验证证据和最终 `accepted_head`。

| 2026-09-11 本机验收 | 范围与结果 | Run 与证据 |
| --- | --- | --- |
| OpenSpec 从目标启动 | `roadmap → native-planning → plan-tasks → implement → audit`，最终 `completed`；9 attempts、9 个唯一 session，562093 ms | `run-f37ce5d5269d4d1cb38a9dffcbf12755`；`.artifacts/real-codex.ndjson`；accepted commit `8a8b7e01823c1a0349ae52a598b221def2c6af42` |
| Spec Kit 导入现成 native feature | `plan-tasks → implement → audit`，最终 `completed`；4 attempts、4 个唯一 session，184862 ms | `run-282118532cd548bd9574aacda3b694d9`；`.artifacts/real-speckit.ndjson`；accepted commit `7fcc675f1c89072c732eaf264d6916baa57d08b6` |

两次均为 macOS arm64 上的小型单阶段算术样例。Spec Kit 这次真实运行从已有规范开始，不能算作真实模型“从目标创建完整 Spec Kit 规划”的验收；多阶段并行、故障注入、范围与资源极限仍以对应 mock/契约测试为主要证据。生成仓库路径虽然位于 `.artifacts/mock-repositories/`，上表后端和 session 证据确认这两次用的是真实 Codex。

这些日志与生成仓库是 gitignored 本地产物，不随 npm 包或 Git checkout 分发。后续重跑的最终结论应绑定新的 run ID、版本、输入与代码 revision，不能把这一快照推广为其他 runner、模型、原生模板版本或全部平台的结果。CI 不自动重跑真实模型验收。

## Node IPC 环境的假绿回归

从 `node --test` 启动 CLI，再由 CLI 启动 `node --test` 验证时，外层 runner 的内部环境可能泄漏到被测进程：`NODE_TEST_CONTEXT`、`NODE_TEST_WORKER_ID`、`NODE_CHANNEL_FD`、`NODE_CHANNEL_SERIALIZATION_MODE`、`NODE_UNIQUE_ID`。这些值描述外层测试或 IPC 通道，不能成为任务验证的输入；泄漏可能产生二进制 IPC 输出或跳过预期断言。

生产进程启动层与 [e2e helper](../tests/e2e/helpers.mjs) 都清理这些内部变量。Rust 回归测试 `nested_node_test_protocol_environment_cannot_turn_failed_verification_green` 故意注入它们，再执行一个抛出 `VERIFICATION_MUST_FAIL` 的真实 Node 测试。通过条件同时包含：非零退出、正常 TAP、`not ok`、异常标记出现；只因 IPC 初始化失败而退出也不算通过。

```sh
cargo test -p spec-autonomous-core --locked --test runtime_contracts nested_node_test_protocol_environment_cannot_turn_failed_verification_green -- --exact --nocapture
```

该回归在本机缺 Node 时会打印未执行原因并返回；CI 先安装并输出 Node 版本，确保此项不会因缺依赖被省略。测试日志中的断言执行证据与退出码需要一起保留。

## 本地可保留的 mock 仓库

普通 e2e 在系统临时目录建仓，测试 teardown 会删除它们。需要逐步检查 worktrees、源文件和日志时，使用仓库生成脚本建立单独的持久 fixture；脚本拒绝覆盖已存在目录。

```sh
cargo build -p spec-autonomous-cli --locked
node scripts/create-mock-repo.mjs --framework openspec --output .artifacts/mock-repositories/openspec-review --fail-once add
./target/debug/spec-autonomous --path .artifacts/mock-repositories/openspec-review run --milestone M001 --json
./target/debug/spec-autonomous --path .artifacts/mock-repositories/openspec-review progress --all-worktrees --format toml
./target/debug/spec-autonomous --path .artifacts/mock-repositories/openspec-review status --json
```

Windows 将可执行文件改为 `target/debug/spec-autonomous.exe`。生成器也支持 `--framework speckit` 和 `--goal-only`；目标模式再通过 `milestone new '<goal>' --id MGOAL --mode autonomous` 创建路线。fixture 配置会绑定当前仓库的 mock agent 和已安装 OpenSpec 路径，因此它是本机调试材料，不是可移植用户项目模板。

从 `status --json` 取 run ID 后，可用 `report <run-id> --json` 查看事件、尝试和证据。运行账本与 worker 产物位于该测试仓库的 Git common directory 下 `spec-autonomous/`，例如普通仓库的 `.git/spec-autonomous/`；linked worktree 的 `.git` 通常是指针文件，不应据此猜账本位置。

## 故障注入与复现

| 注入点或输入 | 当前测试方式 | 必须保留的语义 |
| --- | --- | --- |
| `after_candidate_commit` | debug 环境 `SPEC_AUTONOMOUS_TEST_FAILPOINT`，退出 86 | 候选提交不能被当作 accepted；恢复不能重复应用 |
| `after_git_advance` | 同上，e2e 随后 resume 原 run | 协调 Git 副作用与账本，已集成任务不重复计数 |
| `after_origin_advance` | 同上，最终交付窗口崩溃 | 原分支已前进时恢复交付，不重复开发 |
| `after_hook` | hooks e2e 分别中断幂等/非幂等 hook，再 resume 或显式 resolve-hook | 幂等恢复不重复效果；非幂等未知结果不盲目重跑，协调后才继续 |
| 错误实现/越界写/伪造结果 | mock 的 `failOnce`、`MOCK_SCOPE_ESCAPE`、`MOCK_FORGE_ID` 等 | 验证失败或协议失败不解锁依赖，不交付越界修改 |
| 同步 worker 挂起 | mock `MOCK_HANG`、慢 worker、pause/deadline | 取消并清理后代进程，不等待完整 attempt 超时 |
| DB 写入失败 | Rust 测试的 SQLite 失败 trigger | run 更新和事件在同一事务回滚 |
| 读取 inventory 时出现新 worktree | 独立测试进程包装真实 Git，两次 list 之间创建工作区 | 返回 partial 与变化诊断，不修改调用者环境 |
| 来源漂移/过期验收 | e2e 提交原生文档、源码或 verification 修改 | 重新导入和失效证据，保留人工改动 |

故障注入变量只用于独立测试仓库。debug failpoint 的 86 是刻意模拟崩溃，恢复时需移除该变量。正式 release 编译不启用这些 `debug_assertions` 注入分支。更多负面协议及身份场景以 Rust 契约测试名为入口，不应通过关闭验证来让测试通过。

## CI 与日志

[ci.yml](../.github/workflows/ci.yml) 保留 `ubuntu-22.04`、`macos-14`、`windows-2022` 三个原生 runner，安装 Node 22 和固定 Bun/Rust，显式执行各层检查。步骤使用 bash 的失败传播和 `tee`；测试失败不会因日志管道而变成成功。

CI 的 `tests/e2e/*.test.mjs` glob 会覆盖新增的 resources 与 hooks 文件，并在它们之前构建 debug CLI；显式 OpenSpec ignored 契约仍单独执行。`test:all` 不包含 release build/launcher smoke/pack，CI 另外执行这些步骤。本地与 CI 的 e2e 文件级并发均限制为 2；复现时仍应保留具体工具链、平台与命令。

每个矩阵项上传 `ci-logs-<runner>` artifact，保留 14 天，包含工具链版本、Rust 合约、显式 OpenSpec 契约、Node TAP、e2e TAP、规范校验和组包日志。上传步骤使用 `always()`，前序失败仍保留已经产生的日志；未执行步骤没有日志不能解释为通过。CI 只上传明确日志目录，不上传用户配置、认证信息或整个临时工作区。

目前 e2e teardown 会清理临时 fixture，CI artifact 不是完整的 worktree/SQLite 备份。失败复现可使用上面的持久 mock 仓库，然后按报告路径检查具体产物。任何真实用户项目日志另行审查，不混入公开 fixture artifact。

## 尚不能据此宣称完成的验证

- 本轮本机环境是 macOS arm64。新增远程 CI 步骤是否通过须看实际 Actions run；配置文件本身不证明 Linux、Windows 或其他架构通过。
- npm 声明的六个平台是 macOS arm64/x64、Linux glibc arm64/x64、Windows arm64/x64。三 OS CI 不等于六平台发行矩阵；macOS x64、Linux arm64、Windows arm64 等必须取得对应原生构建、进程控制、安装及运行结果。
- 上述真实 Codex 样例已经完成；其他 vendor runner、真实模型的多阶段并行/范围/native handoff、宿主交互中 `/autonomous` 或 `/auto` 的完整触发流程，以及不同权限/模板/平台仍需分别验收。fixture/mock-agent 不能补足这些外部运行证据。
- registry 名称所有权、真实 `npm publish`、provenance、dist-tag、下载后安装、最低 OS/glibc 与所有平台完整运行尚需发行验收。本 CI 不调用模型、不执行发布。
- 各测试的 `#[cfg(unix)]`、`skip`、`ignored` 和外部依赖缺失说明必须进入验收记录；不能用总数或 OpenSpec 的规划完成状态替代这些边界。

## 补充入口与新增回归

`test:all` 现由 `scripts/test-all.mjs` 以明确 argv 顺序执行，同一个入口可以用 `bun run test:all`、`npm run test:all` 或 `node scripts/test-all.mjs`。测试本身不依赖 Bun 的 shell。CI 与本地 e2e 文件并发均为 2。

新增 budgets.test.mjs 验证相同失败的无进展停止跨 resume 保留，同时独立任务仍可完成；scheduling.test.mjs 用同样六个任务验证 1/3 workers 的实际重叠、峰值和最终等价产物。autonomy 的 debug failpoints 覆盖 after_candidate_patch、after_source_writeback、before_intent_finalize、after_candidate_commit、after_git_advance、after_origin_advance 六个切点，另有 after_hook 的专门恢复测试。

本机本轮观察到打开 `/Users/zaneliu/Documents` 目录阻塞，Bun 从祖先路径定位项目时受影响；Node 对该目录的直接 open 探针也阻塞，但直接访问仓库与执行 `node scripts/test-all.mjs` 正常。TCC preflight 日志仅是相关现象，未确认系统根因。未修改系统权限，已清理挂起探针。Bun 在临时目录对实际 tarball 的 `add --ignore-scripts` 与 launcher smoke 已通过。

最终新增 `provenance.rs` 的 audit 证据索引，以及 resources 中绑定 run/head、artifact SHA256、host verification、环境值不泄漏的 e2e；真实 Spec Kit 目标曾因缺该过程证据而停止，修复后同 run 恢复通过。发布准备新增 14 项注入 npm 的契约测试，覆盖实际 npm tar 格式和发布顺序；测试默认没有 registry 写入。最终数量及本机产物 hash 见 [验收记录](validation/autonomous.md)。
