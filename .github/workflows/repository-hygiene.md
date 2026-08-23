---
# Trigger - when should this workflow run?
on:
  workflow_dispatch:

# Alternative triggers (uncomment to use):
# on:
#   issues:
#     types: [opened, reopened]
#   pull_request:
#     types: [opened, synchronize]
#   schedule: daily  # Fuzzy daily schedule (scattered execution time)
#   # schedule: weekly on monday  # Fuzzy weekly schedule

# Permissions - what can this workflow access?
# Write operations (creating issues, PRs, comments, etc.) are handled
# automatically by the safe-outputs job with its own scoped permissions.
permissions:
  contents: read
  issues: read
  pull-requests: read

# AI engine to use for this workflow
engine: codex

# Tools - GitHub API access via toolsets (context, repos, issues, pull_requests)
# tools:
#   github:
#     toolsets: [default]

# Network access
network: defaults

# Outputs - what APIs and tools can the AI use?
safe-outputs:
  create-agent-task:
    max: 1
  # actions:
  # activation-comments:
  # add-comment:
  # add-labels:
  # add-reviewer:
  # allowed-github-references:
  # assign-milestone:
  # assign-to-agent:
  # assign-to-user:
  # autofix-code-scanning-alert:
  # call-workflow:
  # close-discussion:
  # close-issue:
  # close-pull-request:
  # concurrency-group:
  # create-agent-session:
  # create-agent-task:
  # create-check-run:
  # create-code-scanning-alert:
  # create-discussion:
  # create-project:
  # create-project-status-update:
  # create-pull-request:
  # create-pull-request-review-comment:
  # dismiss-pull-request-review:
  # dismiss-review:
  # dispatch-repository:
  # dispatch-workflow:
  # dispatch_repository:
  # environment:
  # failure-issue-repo:
  # footer:
  # group-reports:
  # hide-comment:
  # id-token:
  # link-sub-issue:
  # mark-pull-request-as-ready-for-review:
  # max-bot-mentions:
  # max-patch-files:
  # mentions:
  # merge-pull-request:
  # missing-data:
  # missing-tool:
  # noop:
  # push-to-pull-request-branch:
  # remove-labels:
  # replace-label:
  # reply-to-pull-request-review-comment:
  # report-failed-jobs:
  # report-failure-as-issue:
  # report-incomplete:
  # resolve-pull-request-review-thread:
  # scripts:
  # set-issue-field:
  # set-issue-type:
  # steps:
  # submit-pull-request-review:
  # threat-detection:
  # unassign-from-user:
  # update-discussion:
  # update-issue:
  # update-project:
  # update-pull-request:
  # update-release:
  # upload-artifact:
  # upload-asset:
  # urls:

---

# repository-hygiene

## Instructions

Act as a repository steward. Work read-only until you have enough evidence to
create at most one `agent-task`; never delete files, push commits, merge PRs,
move tags, or create a release yourself.

First inspect `AGENTS.md` and `.github/aw/instructions.md`. They are binding.
Then inspect the triggering issue when present, all closed issues, merged PRs,
`main`, GitHub releases, tags, `CHANGELOG.md`, and the repository files.

Perform these deterministic audits:

1. Every merged PR's merge commit must be reachable from `main`.
2. Every merged PR must have exactly one `v0.1.N` release/tag record whose
   target is that merge commit, in merge order; the newest source version and
   changelog entry must agree.
3. A working report may be considered only when it is explicitly tied to a
   closed issue and is not a canonical context file. Canonical context files
   are never cleanup candidates.

If all audits pass, return a concise no-op result. If an audit fails, create
one agent task containing exact issue/PR/tag/file evidence and this constraint:
the task must open a reviewable PR, preserve canonical context files, make the
smallest repair, and rerun the relevant verification. For a completed working
report, the task must prove the issue is closed and its acceptance work is in
`main` before deleting only that report and stale links. Never create an agent
task merely to restate policy or write a status report.

## Notes

- Run `gh aw compile` to generate the GitHub Actions workflow
- See https://github.github.com/gh-aw/ for complete configuration options and tools documentation
