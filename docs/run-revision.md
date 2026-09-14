# 验证命令修订与续跑

alpha.9 增加 `run.revise`。当 run 的任务、阶段或里程碑验证使用了错误 argv/cwd，可以在保留 run ID、已接受提交、已验证任务和历史证据的情况下修正待完成检查。

## 预览与应用

先用 `state.get` 的完整视图获取 `plans` 和 `milestone` 中的现行检查，并核对原生需求。将修订写入一个 JSON 文件，例如 `revision.json`：

```json
{
  "run_id": "run-example",
  "reason": "本机 Node 将 test/ 作为模块入口；原生要求仍是统计结果及其单测，改用明确测试文件。",
  "task_checks": [
    {
      "phase_id": "P1",
      "task_id": "t12",
      "checks": [{ "argv": ["node", "--test", "test/wordcount.test.mjs"], "cwd": "." }]
    }
  ]
}
```

```sh
spec-autonomous tools call run.revise --input @revision.json --json
```

审阅返回的 `diff`、`active_requests` 和 `retained_verified_tasks`。如果仍有任务在执行，先让宿主停止并用 `work.revoke` 确认撤销；已提交的回执应先恢复处理。随后重新预览，在相同 JSON 中加入 `"apply": true` 和当前 `"plan_hash": "..."`，再次调用同一命令。

```sh
spec-autonomous tools call run.revise --input @revision.json --json
spec-autonomous prepare --run-id run-example --json
```

MCP 使用 `sa_tools`：`operation: call`、`capability: run.revise`、`arguments` 为相同 JSON。阶段检查使用 `phase_checks: [{phase_id,checks}]`；完整里程碑的检查使用 `milestone_checks: [{argv,cwd}]`。修订后的检查仍由协调器执行，结果通过门禁后才完成。

## 保留与约束

- 当前运行的有效验证保存在账本；原有 TOML 是基线。`state.get` 的完整视图展示有效 plan 和 `verification_revisions`，语义审查的证据文件也包含修订原因和前后差异。
- 不改变已完成任务、DAG、source_ids、原生 Markdown、阶段范围或 Git HEAD；不能用空检查数组取消验证。检查是否仍满足需求需要宿主根据原生规范审阅，框架不把新命令的成功等同于需求正确。
- 修订仅重置受影响待完成任务的重试计数起点；旧失败、日志、提交和运行耗时保留。需要扩展时间时显式使用已有预算选项。
- 只清除被此次修订替代的 `host-verification` 修复。原生需求/语义修复保持独立；验收标准本身有误时应通过原生工作流修订。
- 同一 hash 与内容重放幂等；状态、来源或 Git 变化后旧预览被拒绝。已取消/完成的 run 保留终态。
- 后续新 run 使用项目基线；若希望复用修正，运行结束后通过原生流程更新规划文件。`prepare(run_id,plan)` 现在明确返回 `revision_required`，不会静默忽略 plan。

## 验证预检与任务粒度

`verification_readiness` 区分 ready、deferred 与配置诊断，`executed: false` 明确说明没有执行未来测试。计划中尚未生成的文件可以 deferred；缺失工具、无效 cwd 会给出诊断。Node `--test test/` 是版本相关的兼容风险，优先选择具体文件、受支持的模式或自动发现；不能只因一个 Node 版本失败就宣称所有版本都不支持目录。

规划器被要求将相关实现与测试合为完整交付单元，以多个 `source_ids` 覆盖原生任务，保留真实依赖和阶段约束。没有修改且写入范围未声明的失败验证会停止自动重试，供宿主判断命令或范围问题。`writes: []` 仍表示未知范围并保守串行，不表示只读。

多工作包可使用 `work.claim-batch` 一次认领，参数为 `run_id` 与 `requests: [{request_id,token,host}]`；每个 host 必须包含独立 `host_id/session_id/fresh_context`。批内任一项无效不会部分认领。终态资源清理见 [运行清理](run-cleanup.md)。

首次应用验证修订时，账本在同一事务中升级到内部 schema 3，以阻止旧版 CLI 写回时丢弃修订历史。公开 JSON/MCP 字段和工作包 schema 保持兼容。升级 npm 后请重新加载项目 MCP 服务，再运行 init 更新项目 Skills；不要让旧版服务继续处理修订后的账本。

原生 roadmap 的无关内容更新会保留有效验证覆盖；如果原生流程明确修改了对应检查，旧覆盖标记为 superseded_by_native 并进入审查历史，采用新的原生检查。

命令选择参考 [Node 24 测试运行器文档](https://nodejs.org/download/release/v24.13.1/docs/api/test.html#running-tests-from-the-command-line)；本轮目录参数行为另以实际 Node 24.21.0 复现记录为准。

## Smart discuss

Before planning a phase, use `discussion.next` to render cards. Put the recommended option first and show alternatives; clicking an option calls `discussion.apply` with the source hash. `auto:true` applies only recommendations and leaves unresolved cards explicit. Decisions are written to a derived phase CONTEXT/DISCUSSION-LOG and the run ledger; native specs remain authoritative. This follows GSD discuss-phase's gray-area selection and context artifact pattern, adapted to deterministic host buttons.

Smart discuss follows GSD's phase gray-area pattern: cards are deterministic, the recommended choice is first, alternatives remain visible, and host clicks are persisted as derived context decisions. It does not require conversational user answers.

`auto` performs this check by default. An empty `cards` array means no gray area requires the user; the host shows the current decision summary and continues. A non-empty `requires_user` card list pauses the phase before native planning.

Roadmap `revision` is an internal CAS epoch, not a product version. In a continuously evolving project, add or split work items/phases and let the system advance the epoch automatically; teams coordinate through stable IDs, dependencies, ownership, snapshots and leases.
