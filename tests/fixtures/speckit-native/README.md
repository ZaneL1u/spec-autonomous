# Native Spec Kit test inputs

Unmodified command templates from github/spec-kit commit
`c173bf19a6654e3b05386ec3599349a55282b897` (the research lock's 1.0.7.dev0 snapshot).
The accompanying MIT license is retained. These are test inputs for the local mock
repository, not implementation instructions for this repository and not runtime
fallback templates. They are excluded from the npm package.

Vendoring this small fixture set keeps unit/e2e runs independent of `.references/`
and network access. Runtime planning uses the target user's own installed native
skills/templates instead.
