# 完整工作流

Spec Autonomous 既能接手一个已经写好的 change / feature，也能从一句目标开始。

## 路径一：接手现有规范

```text
已有 OpenSpec change / Spec Kit feature
  → 读取原生工件和任务
  → 讨论必要灰区
  → 生成可执行任务图
  → 分批派发工作包
  → 验证、集成、回写
  → convergence / audit
  → 归档
```

适合已经在团队中使用 SDD，只想补齐可靠执行的人。

## 路径二：从目标开始

```sh
spec-autonomous prepare \
  --goal "支持团队邀请与成员权限" \
  --id M001 \
  --json
```

此时原生规划工作仍由宿主 Agent 按 OpenSpec / Spec Kit 的官方流程完成。CLI 只准备对应工作包、保存 roadmap，并验证宿主回执。

## Milestone、Phase 和 Task

- **Milestone**：一个稳定目标的协作视图，不是产品版本号。
- **Phase**：里程碑中的交付单元，绑定一个 OpenSpec change 或 Spec Kit feature。
- **Task**：从原生任务进一步整理出的可执行工作，保留来源映射。
- **Attempt**：某个任务的一次尝试。重试会新建 attempt，不覆盖失败证据。

`milestone.toml.revision` 只是 roadmap 的乐观锁版本，不是 sprint 编号，也不是 semver。

## Smart Discuss

进入阶段规划前，系统会找出真正需要人决定的灰区，例如：

- 没有明确验收命令；
- 原生任务标记了并行候选，但项目尚未确认排序策略。

`discussion.next` 返回稳定的卡片、推荐选项、替代项、原因和影响。Auto 只能采用明确的推荐项；没有安全推荐时仍会停下来问人。

## 生成工作包

`prepare` 遇到语义工作会返回不可变请求，其中包含：

- run / phase / task / attempt 身份；
- 来源 hash 和 base commit；
- 分配好的 worktree；
- 有界上下文或完整上下文文件引用；
- 允许的写入范围与验证边界；
- 结果 JSON Schema；
- token、超时和剩余预算。

CLI 到这里就退出，不保持一条“总控模型会话”。

## 宿主执行

Skill 或 MCP 客户端让宿主创建 fresh-context Agent：

1. 用唯一会话身份 claim 工作包；
2. 在指定 worktree 中工作；
3. 按 schema 写出 `WorkerResult`；
4. 使用同一身份和 token 提交回执。

CLI 不知道也不需要知道宿主内部使用哪一个模型，只验证协议和实际结果。

## 验证与集成

回执到达后依次检查：

1. schema、ID、hash、owner 和结果大小；
2. worktree 中真实存在的 diff；
3. 是否越过声明的写入范围；
4. worker revision 上的任务验证；
5. 组合 candidate revision 上的阶段验证；
6. 验证过程有没有偷偷改源码树；
7. Git 是否仍处于预期目标 revision；
8. 原生 checkbox 是否仍能安全 CAS 回写。

全部成立后才记录 accepted revision，并准备下一批。

## 范围运行

```sh
spec-autonomous prepare --milestone M001 --from 2 --to 4 --json
spec-autonomous prepare --milestone M001 --only 3 --json
```

`from` / `to` 是包含端点的 phase 范围。范围结束是 `scope_completed`，不等于整个 milestone 已完成。

## 完成与归档

完成实现后仍可能需要 audit 或 convergence。所有要求、验证、原生回写和交付都成立，才能进入 `completed`。

归档采用“两步走”：

```sh
spec-autonomous archive --milestone M001 --json
spec-autonomous archive --milestone M001 --apply --plan-hash <hash> --json
```

第一步只是预览；第二步绑定来源、目标 HEAD 和配置执行。中间状态变化会拒绝旧预览。
