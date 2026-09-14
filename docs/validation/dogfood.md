# Dogfood playground

`playground/` contains a small OpenSpec todo-list change. `node scripts/dogfood.mjs` copies it to a temporary Git project, runs `init --provider openspec --agent codex --mcp --non-interactive`, commits the generated baseline, configures the checked-in OpenSpec CLI, then drives the independent mock host through roadmap/native planning, plan tasks, work packets, receipts, verified integration, all-worktree progress and cleanup preview. The source playground is never modified.

The report is written to `.artifacts/dogfood-report.json` and the temporary project is removed. The fixture uses a pre-existing test contract so readiness can distinguish planned implementation from a missing verification target. It does not claim model quality or performance; it validates the host protocol and user-visible lifecycle.

本轮实测报告：dogfood 输出 status=completed、OpenSpec 初始化成功、progress 观察到 9 个 worktree、cleanup preview 生成 plan_hash；源 playground 未被修改。完整 alpha.10 门禁的 Rust/JS/E2E 日志记录于 `.artifacts/dogfood-alpha10-test-all.log`，E2E 83 项通过，strict specs 在补齐本变更 delta 后全部通过。

alpha.10 macOS arm64 资产 SHA256：

```text
64ef113246b086f75d8625e16857e0af0eb82b2588603ea599f51f93f5bd5d63  spec-autonomous-0.1.0-alpha.10.tgz
51e36c3cfcee0e332dd95b09e5cac8f2dd0cd8bfe2a05e8d359378d459bf0501  spec-autonomous-darwin-arm64
```

alpha.11 dogfood additionally renders a discussion card before cleanup, applies its recommended option by source CAS, and confirms one persisted click decision.
