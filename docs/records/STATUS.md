# Progress and engineering records

[简体中文](../zh-CN/records/STATUS.md) · [Index](../INDEX.md)

<a id="baseline"></a>

## Repository baseline

On 2026-11-27, the owner created the public repository [zemeng5208/winstrata](https://github.com/zemeng5208/winstrata). Git and the GitHub API verified default branch `main`, commit `fafe461abadfdee477ffef14f8bae549bddd5872`, and the initial README and Apache-2.0 license.

The owner saved [Protect main](https://github.com/zemeng5208/winstrata/rules/28217627). The GitHub branch and effective rules endpoints confirmed protection is active, PRs and resolved review conversations are required, and force pushes and branch deletion are blocked. There is no required status check at the documentation-only baseline. The owner performs review and merge in the single-maintainer phase. PR 002 will validate and add the relevant CI and ordering checks.

## Progress

| Work | State | Evidence and next action |
|---|---|---|
| PR 000 | Accepted | Owner merged [GitHub PR #1](https://github.com/zemeng5208/winstrata/pull/1) as `c3e8abf07895edbdd1f84792dbab5c48b00c25e8`. Verified main has the reviewed tree and active PR protection. |
| PR 001 | Accepted | Owner merged [GitHub PR #2](https://github.com/zemeng5208/winstrata/pull/2) as `e0a3bbf2ec8c6b3f322d3f0379e47618fd129c3c`. The merged tree matched the reviewed head. Linux doctor, build and verify passed on that tree. |
| PR 002 | Awaiting owner review | [CI and merge gates](../architecture/MERGE_GATES.md). Read the PR check results and activate the required checks in the documented order. |
| PR 003–050 | Planned | Gates, feasibility, implementation and acceptance follow the approved roadmap and prerequisites. |
| Core and PL-01 through PL-25 | Planned | Runtime and metrics remain unverified. Maintain their evidence in requirements and module records as work lands. |

## PR 000 verification

The Linux development environment rendered all 45 Markdown files using Pandoc GFM. The local reference check resolved 1,184 relative links and anchors. It matched 90 atomic requirement IDs and mappings, 25 plugin IDs and names, 61 agent rule IDs, 51 plan IDs and their direct prerequisites, and 16 scenario IDs across languages. Git checked the staged diff for whitespace and scope. Windows builds, driver loading, GUI runs and performance are tracked in the roadmap.

## PR 001 verification

On Linux x86_64, Rust and Cargo 1.100.0 with GCC 14.2, Node.js 24.22.3, and pnpm 11.25.0 ran `cargo dev doctor`, `cargo dev init`, `cargo dev build`, and `cargo dev verify` successfully. Cargo built the actual developer executable. The test harness reported zero unit tests in this phase. The executable and package-manager workflows were exercised directly.

With Node.js or pnpm removed separately from the command search path, the preflight exited with status 1 and identified the missing tool. In a clean source copy moved to a directory with a space in its name, the commands ran from the nested Rust crate and used the moved Cargo workspace, target directory, and pnpm store. The external host toolchain was reused. Windows host execution remains unverified. Git ignore checks passed for the generated Rust target, pnpm install directory, and workspace-local cache.

## PR 002 verification

On Linux x86_64, the CI bootstrap verified the official Node archive checksum, installed the pinned pnpm, and resolved the approved Rust toolchain. Six Node tests verified plan order, defect blockers, owner confirmation bound to the exact head, and verified test merge parents. The workspace script ran the policy tests and the approved developer commands. The existing Protect main rules are active. Hosted status results and required-rule activation are recorded through the PR and the effective rules.

## Decisions and open assumptions

| ID | Status | Evidence and follow-up |
|---|---|---|
| ADR-000 | Owner confirmed | Publish the project as `zemeng5208/winstrata` under Apache-2.0. The initial GitHub commit contains the license. Preserve it. Add a NOTICE only if later material adds an applicable notice obligation. |
| ADR-001 | Accepted in PR 000 | Adopt linked bilingual documentation under `docs/` and `docs/zh-CN/`, stable plan and rule IDs, a single engineering record, and the proposed component directory layout. |
| ADR-002 | Owner confirmed | Pin Rust and Cargo 1.100.0, Node.js 24.22.3 LTS, and pnpm 11.25.0. Use Cargo and pnpm root workspaces and keep this phase free of external Rust or JavaScript packages. |
| AS-001 | Unverified | Verify native Linux Windows-driver build, links and signing in PR 003. Also verify real Windows load. |
| AS-002 | Unverified | Verify runtime isolation, IPC, native collection and frontend compatibility in PR 004 and PR 015. |
| AS-003 | Pending decision | Confirm accounting and cleanup semantics, untagged metadata retention, WebView2 themes, Windows support, and performance methods in the [open decision list](../roadmap/PLAN.md#section-19). |

## Errors and handover

| ID | State | Discovery, impact and retest |
|---|---|---|
| ERR-000 | Resolved | On 2026-11-27 GitHub HTTPS denied development-branch push with HTTP 403. There was no personal-account installation covering the repository. The owner configured it and a single development-branch dry-run succeeded. |
| ERR-001 | Resolved by owner configuration | GitHub returned HTTP 403 for ruleset creation by the integration. The owner saved the protection ruleset. Retrieving the branch, rulesets and effective branch rules confirmed active enforcement. The integration still lacks the settings permission. |
| ERR-002 | Resolved in the Linux workspace | PR 001 preflight found no Rust or Cargo on PATH. The approved official toolchain was installed into the project’s ignored local tooling directory after verifying the Rustup installer checksum. Both version checks and the actual build passed. |

Continue on `pr/002-merge-gates` from the verified PR 001 merge. The owner reviews and merges PR 002. Check the effective rules and activate the trusted policy status on the first eligible subsequent candidate before it merges. Use the [standard record fields](../standards/DOCUMENTATION.md#section-12).
