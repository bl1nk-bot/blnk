# blnk agentic-workflow contract

The repository's canonical context is `AGENTS.md`, `TODO.md`,
`specs/spec.md`, `docs/architecture.md`, `docs/api.md`,
`docs/blueprint.md`, `docs/implementation-status.md`, `docs/plans/**`, and
`.github/**`. Never treat those files as disposable reports.

Read `docs/agent-operations.md` before delivery work. It is the persistent
delivery lifecycle and learning ledger; update it when an operational mistake
reveals a reusable process lesson.

For headless work, prefer evidence-producing checks and a reviewable pull
request over prose status updates. Do not create a report merely because a
task could not be completed. Report an exact blocker in the task/PR instead.

Working reports are candidates for cleanup only after the linked GitHub issue
is closed, the implementation is proven reachable from `main`, and all
canonical references have been checked. A cleanup agent may delete only the
specific verified report and stale links in the same PR.

All delivery PRs target `main`. Every PR bumps the patch version exactly once,
adds the matching changelog entry and the next semantic version, and relies on the release workflow to make
the immutable tag and GitHub Release. Never retag, overwrite a release, or
merge a PR without passing the automated checks.
