# CI and merge gates

[简体中文](../zh-CN/architecture/MERGE_GATES.md) · [Documentation index](../INDEX.md)

PR 002 implements the checks described by the [Git and PR standard](../standards/GIT_AND_PR.md#section-13). Plans follow the approved order in the [roadmap](../roadmap/PLAN.md). Independent branches can be prepared in parallel. The owner reviews and merges each PR after the required evidence and protections are in place.

## Checks

| Check | Source and scope |
|---|---|
| `Workspace checks` | The Linux and Windows jobs check out the same GitHub test merge commit for a PR or the exact commit pushed to main. Linux runs the workspace script, including artifact and whitespace checks, policy tests, the pinned developer commands and lockfile checks. Windows runs `cargo test --locked --workspace` with the project toolchain and Node.js. The required check passes only when both jobs succeed. All three jobs use empty GitHub API permissions. |
| `winstrata/merge-policy` | The policy workflow fetches protected `main` and verifies the current test merge commit and both parents, and posts the policy status on the PR head alongside the GitHub Actions check. It verifies that the head includes the current main commit. It reads repo and PR metadata and writes statuses. Candidate code has no role in that workflow. When the PR changes `docs/roadmap/PLAN.md`, the script on `main` reads that file from the verified test merge commit as data. It re-evaluates affected PRs on PR, owner comment and blocking issue events and refreshes them when `main` advances. |

To run the same workspace checks locally, use the [approved toolchain](DEVELOPMENT_TOOLCHAIN.md) and run `bash .github/scripts/check-workspace.sh` from a Linux Git checkout on the candidate commit with its first parent available. On a Windows host with the pinned Node.js and Rust toolchains, run `cargo test --locked --workspace` from the repository root.

## Merge decisions

Use `pr/NNN-description` for the branch and `PR NNN: Title` for the title. The two plan IDs must match. The gate reads the ordered roadmap from `main`. When the PR changes `docs/roadmap/PLAN.md`, it uses the order in the verified test merge commit. Every plan ID already on `main` must remain in that same relative order. IDs present only in the test merge are that PR's additions. Dropping or reordering an existing ID fails. The gate checks GitHub for merged predecessors and duplicate plan IDs. A merged plan ID cannot be reused. Drafts are blocked.

Apply the `merge-blocked` label to a PR for a confirmed defect in that PR. An open issue with the same label blocks all pending merges, including defects found after a merge. After fixing and retesting, the owner can close the issue or remove the label and wait for the updated status. Review conversations also remain subject to the native ruleset.

Changes to workflows, the shared developer check, the roadmap, or governance files need explicit owner confirmation for the new commit. The owner records it in a PR comment as `Approve gate policy at <full head SHA>` after reviewing the change. A new commit requires new confirmation. This check also examines renamed files.

## Activating the required checks

1. The owner verifies a successful `Workspace checks` run on PR 002 and adds it to the existing `Protect main` ruleset. Set the expected source to GitHub Actions when the selector is available. Keep the native PR, review-conversation, deletion and force-push rules and the empty bypass list.
2. After PR 002 merges, the policy workflow is available from `main`. The first subsequent candidate must show both the real `winstrata/merge-policy` status and the GitHub Actions check on its current head. The workspace job still runs on the test merge commit. Keep that candidate open while the owner adds the status as a second required check, selects GitHub Actions as its source when available, and enables the option to require branches to be up to date.
3. Read the effective rules and confirm both checks match before merging that candidate. If a new commit, blocker, label or comment has just changed, wait for policy re-evaluation. Event processing is asynchronous. GitHub repository writers and settings admins remain trusted, and the owner still inspects actual scope and checks.

GitHub first recognizes a required check after it has run. During this rollout, the owner applies any pending gate manually. The absence of hosted execution or of a rule in the effective branch settings stays recorded as pending.
