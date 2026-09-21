# 运行结束后的资源清理

`run.cleanup` 为 completed、scope_completed 或 cancelled 的运行生成清理预览。预览不删除文件，不创建运行记录；只有返回的清单经 `apply` 提交后才会执行。

```sh
spec-autonomous tools call run.cleanup --input '{"run_id":"<run-id>","delete_branches":true,"view":"full"}' --json
spec-autonomous tools call run.cleanup --input '{"run_id":"<run-id>","delete_branches":true,"apply":true,"plan_hash":"<预览返回的哈希>","view":"full"}' --json
```

MCP 对应 `sa_tools` 的 `operation: "call"`、`capability: "run.cleanup"`，上述 JSON 作为 `arguments`。

清理包括已接受、集成、失败、撤销及被新尝试取代的工作树，以及已接受或放弃的集成候选工作树。运行被取消不等于正在工作的会话已经停止：有未结束所有者或进程、未完成请求、未知状态的资源仍会保留。脏工作树、锁定的工作树、分支或目录归属变化、其他运行引用的资源都不会移除。`retained_details` 为每项返回稳定的原因代码；`worktrees`、`branches` 是拟执行清单，`removed`、`removed_branches` 是实际执行结果。

默认保留全部分支。显式指定 `delete_branches: true` 时，只会删除运行记录中归属明确、已经合入该运行 accepted_head 且没有被任何工作树检出的分支。删除使用预览的提交 SHA 做比较并交换；分支移动后需要重新预览。未合入的候选提交通过分支继续保留，即使相应干净工作树已被移除。之前移除工作树后残留的安全分支，也可以通过此操作清理。

始终保留集成工作树及其分支、运行账本、验证日志与证据文件。清理不会改写已验证进度，也不会清理外部或未登记的工作树。工作树或计划在预览之后发生变化会拒绝旧哈希；执行时再次检查 Git 状态，无法安全移除的项留在结果中供下一次处理。

历史命令 `spec-autonomous cleanup <run-id>` 保持兼容：立即移除符合条件的干净工作树并保留全部分支。需要审阅清单或删除安全分支时使用 `tools call run.cleanup`。


`delete_integration:true` 是额外的显式范围：仅在 integration worktree 干净、HEAD 等于 accepted_head、不是当前 checkout、且分支可安全处理时删除。运行记录、验证证据和账本始终保留；默认值为 false。预览哈希包含两个删除选项，应用时必须保持完全相同的 scope。

Dogfood 可执行 `node scripts/dogfood.mts`，验证初始化、auto 回执、progress 和 cleanup preview；它复制 `playground/` 到临时目录并在结束后删除。
