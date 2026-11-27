# CI and merge gates

[简体中文](../zh-CN/architecture/MERGE_GATES.md) · [Documentation index](../INDEX.md)

PR 002 implements the checks described by the [Git and PR standard](../standards/GIT_AND_PR.md#section-13). Plans follow the approved order in the [roadmap](../roadmap/PLAN.md). Independent branches can be prepared in parallel. The owner reviews and merges each PR after the required evidence and protections are in place.

## Checks

| Check | Source and scope |
|---|---|
| `Workspace checks` | The `WinStrata CI` workflow fetches GitHub's exact test merge commit for PRs. The Linux runner uses the project toolchain pins and runs whitespace and tracked-artifact checks, policy regression tests, and the same `cargo dev init`, `build`, and `verify` commands used locally. It verifies both lockfiles remain unchanged. The job has no GitHub API permissions and receives no project secrets. |
| `winstrata/merge-policy` | The policy workflow fetches protected `main` and verifies the current test merge commit and both parents, and posts the policy status on the PR head alongside the GitHub Actions check. It verifies that the head includes the current main commit. It reads repo and PR metadata and writes statuses. Candidate code has no role in that workflow. It re-evaluates affected PRs on PR, owner comment and blocking issue events and refreshes them when `main` advances. |

To run the same workspace checks locally, use the [approved toolchain](DEVELOPMENT_TOOLCHAIN.md) and run `bash .github/scripts/check-workspace.sh` from a Linux Git checkout on the candidate commit with its first parent available.

## Merge decisions

Use `pr/NNN-description` for the branch and `PR NNN: Title` for the title. The two plan IDs must match. The gate reads the ordered roadmap from `main` and checks GitHub for merged predecessors and duplicate plan IDs. Drafts are blocked.

Apply the `merge-blocked` label to a PR for a confirmed defect in that PR. An open issue with the same label blocks all pending merges, including defects found after a merge. After fixing and retesting, the owner can close the issue or remove the label and wait for the updated status. Review conversations also remain subject to the native ruleset.

Changes to workflows, the shared developer check, the roadmap, or governance files need explicit owner confirmation for the new commit. The owner records it in a PR comment as `Approve gate policy at <full head SHA>` after reviewing the change. A new commit requires new confirmation. This check also examines renamed files.

## Activating the required checks

1. The owner verifies a successful `Workspace checks` run on PR 002 and adds it to the existing `Protect main` ruleset. Set the expected source to GitHub Actions when the selector is available. Keep the native PR, review-conversation, deletion and force-push rules and the empty bypass list.
2. After PR 002 merges, the policy workflow is available from `main`. The first subsequent candidate must show both the real `winstrata/merge-policy` status and the GitHub Actions check on its current head. The workspace job still runs on the test merge commit. Keep that candidate open while the owner adds the status as a second required check, selects GitHub Actions as its source when available, and enables the option to require branches to be up to date.
3. Read the effective rules and confirm both checks match before merging that candidate. If a new commit, blocker, label or comment has just changed, wait for policy re-evaluation. Event processing is asynchronous. GitHub repository writers and settings admins remain trusted, and the owner still inspects actual scope and checks.

GitHub first recognizes a required check after it has run. During this rollout, the owner applies any pending gate manually. The absence of hosted execution or of a rule in the effective branch settings stays recorded as pending.
