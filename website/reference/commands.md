# 命令参考

运行 `spec-autonomous --help` 查看当前安装版本的完整参数。下面按使用目的解释主要命令。

## 全局参数

```sh
spec-autonomous \
  --path <repository> \
  --framework auto|openspec|speckit \
  --lang zh-CN|en-US \
  --view agent|full \
  --json \
  <command>
```

- `--path`：目标仓库，可指向仓库子目录。
- `--framework`：多框架共存时明确选择。
- `--json` / `--format`：机器可读输出。
- `--view agent|full`：默认给 Agent 有界摘要，需要诊断时看完整结果。
- `--fields`：只取指定字段。
- `--limit` / `--offset`：分页，聚合总数不会因分页丢失。
- `--lang`：覆盖自动检测的界面语言。

## 检测与读取

### `inspect`

查看 provider、原生工件、来源和项目状态，不写文件：

```sh
spec-autonomous inspect --json
spec-autonomous inspect --change add-auth --json
spec-autonomous inspect --feature specs/001-auth --json
```

### `progress`

聚合整个 Git 仓库的进度，包括 linked、external 和 unknown worktree：

```sh
spec-autonomous progress --all-worktrees --json
```

### `next`

只读预览某个 run 的下一动作、就绪工作和阻塞：

```sh
spec-autonomous next --run-id <run-id> --json
```

## 准备与推进

### `prepare`

新建或继续 run。它准备工作包，但不启动 Agent：

```sh
spec-autonomous prepare --change add-auth --json
spec-autonomous prepare --feature specs/001-auth --json
spec-autonomous prepare --milestone M001 --from 2 --to 4 --json
spec-autonomous prepare --goal "团队邀请 MVP" --id M001 --json
spec-autonomous prepare --run-id <run-id> --json
```

常用模式：

- `autonomous`：通过宿主工作包持续推进；
- `native`：完成 roadmap 后交回原生流程；
- `plan`：只形成计划，不执行实现。

### `claim`

宿主领取工作包，注册唯一 fresh session：

```sh
spec-autonomous claim <run-id> <request-id> \
  --token <token> \
  --host-id <host> \
  --session-id <unique-session> \
  --fresh-context \
  --json
```

### `apply-result`

提交宿主按结果 schema 生成的回执：

```sh
spec-autonomous apply-result \
  --result <result.json> \
  --token <token> \
  --host-id <host> \
  --session-id <unique-session> \
  --fresh-context \
  --json
```

重复提交相同回执是幂等的；相同 token 的不同内容会被拒绝。

## 初始化与 provider

```sh
spec-autonomous init
spec-autonomous init --provider openspec --agent codex --mcp
spec-autonomous providers status --json
spec-autonomous providers ensure speckit --json
spec-autonomous providers exec openspec -- --version
```

`providers exec` 保持 argv 边界，不把用户参数拼成 shell 字符串。

## 暂停、取消与诊断

```sh
spec-autonomous pause <run-id>
spec-autonomous cancel <run-id>
spec-autonomous doctor --json
```

暂停或取消可以停止 CLI 自己启动的验证进程。外部 Agent 必须由宿主停止，再通过 `work.revoke` 确认。

## 归档

```sh
# 预览
spec-autonomous archive --change add-auth --json

# 应用同一预览
spec-autonomous archive \
  --change add-auth \
  --apply \
  --plan-hash <hash> \
  --json
```

里程碑归档使用 `--milestone M001`，按 phase 依赖顺序处理。

## 高级工具

```sh
spec-autonomous tools list --all --json --limit 200
spec-autonomous tools call state.get \
  --input '{"run_id":"<run-id>"}' \
  --json
spec-autonomous tools call document.inspect \
  --input @arguments.json \
  --json
```

每个条目都带 input schema、读写属性和说明。优先查询目录，不要猜参数。
