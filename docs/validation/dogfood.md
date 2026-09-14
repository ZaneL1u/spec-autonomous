# Dogfood playground

`playground/` contains a small OpenSpec todo-list change. `node scripts/dogfood.mjs` copies it to a temporary Git project, runs `init --provider openspec --agent codex --mcp --non-interactive`, commits the generated baseline, configures the checked-in OpenSpec CLI, then drives the independent mock host through roadmap/native planning, plan tasks, work packets, receipts, verified integration, all-worktree progress and cleanup preview. The source playground is never modified.

The report is written to `.artifacts/dogfood-report.json` and the temporary project is removed. The fixture uses a pre-existing test contract so readiness can distinguish planned implementation from a missing verification target. It does not claim model quality or performance; it validates the host protocol and user-visible lifecycle.
