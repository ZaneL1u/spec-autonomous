# 恢复与清理

## 先看状态

```sh
spec-autonomous next --run-id <run-id> --json
spec-autonomous progress --all-worktrees --json
spec-autonomous doctor --json
```

不要看到 stale 就立刻重跑。Stale 只说明心跳过期，旧 Agent 仍可能写文件。

## 继续一个 run

```sh
spec-autonomous prepare --run-id <run-id> --json
```

恢复会先检查：

- canonical receipt 是否已写；
- intent 是否已记录；
- Git candidate / accepted head 到了哪一步；
- 原生任务是否已经回写；
- worktree 和 owner 是否仍存在；
- source hash 是否变化。

## 停止外部工作

```sh
spec-autonomous pause <run-id>
spec-autonomous cancel <run-id>
```

返回值会列出宿主应停止的 request。宿主确认 Agent 及其子进程停止后：

```sh
spec-autonomous tools call work.revoke \
  --input '{
    "run_id":"<run-id>",
    "request_id":"<request-id>",
    "token":"<token>",
    "host_stopped":true,
    "reason":"Host confirmed the session stopped"
  }' \
  --json
```

CLI 不会假装自己杀掉了宿主内部会话。

## 修订错误验证

如果代码没问题，失败来自错误的 argv / cwd / fixture，可以使用 `run.revise`。流程是：

1. 检查原生需求和失败证据；
2. 预览修订；
3. 审查 before / after；
4. 用相同 payload 和 `plan_hash` 应用；
5. 继续同一个 run；
6. 新验证仍必须真正通过。

它不会抹掉早先失败，也不会让已接受任务重做。

不要为了绿灯删除检查。长期修正还应写回原生 plan；run revision 默认只对当前 run 生效。

## 来源变化

用户修改原生 spec / tasks 或目标 HEAD 后，旧计划可能不再适用。系统会用 source hash 和 expected head 停止，而不是把旧回执套到新需求上。

先停止旧 host work，再导入新来源并重建受影响的图。已验证且不受影响的工作会保留。

## 未知 hook 结果

非幂等 hook 在“已执行但还没记录”时崩溃，不能安全重跑。使用 `hook.resolve` 明确记录：

- 已成功；
- 已失败；
- 未发生。

并附上人工或外部系统证据。

## 终态清理

```sh
# 预览
spec-autonomous tools call run.cleanup \
  --input '{"run_id":"<run-id>"}' \
  --json

# 应用
spec-autonomous tools call run.cleanup \
  --input '{
    "run_id":"<run-id>",
    "apply":true,
    "plan_hash":"<hash>"
  }' \
  --json
```

默认保留证据和分支。只有受管理、已合并、未被其他 worktree 使用且状态与预览一致的资源才会删除。

Dirty、locked、unknown、external 和共享资源会保留并报告。不要用 `git reset --hard` 或强删目录来制造“看起来很干净”的结果。
