# Progress and engineering records

[简体中文](../zh-CN/records/STATUS.md) · [Index](../INDEX.md)

<a id="baseline"></a>

## Repository baseline

On 2026-11-27, the owner created the public repository [zemeng5208/winstrata](https://github.com/zemeng5208/winstrata). Git and the GitHub API verified default branch `main`, commit `fafe461abadfdee477ffef14f8bae549bddd5872`, and the initial README and Apache-2.0 license.

The owner saved [Protect main](https://github.com/zemeng5208/winstrata/rules/28217627). The GitHub branch and effective rules endpoints confirmed protection is active, PRs and resolved review conversations are required, and force pushes and branch deletion are blocked. There is no required status check at the documentation-only baseline. The owner performs review and merge in the single-maintainer phase. PR 002 will validate and add the relevant CI and ordering checks.

## Progress

| Work | State | Evidence and next action |
|---|---|---|
| PR 000 | Awaiting owner review | Bilingual foundation, [requirements](../roadmap/REQUIREMENTS.md), [roadmap](../roadmap/PLAN.md), and [acceptance](../roadmap/ACCEPTANCE.md). Review all open decisions and verify actual main after merge. |
| PR 001–050 | Planned | Gates, feasibility, implementation and acceptance follow the approved roadmap and prerequisites. |
| Core and PL-01 through PL-25 | Planned | Runtime and metrics remain unverified. Maintain their evidence in requirements and module records as work lands. |

## PR 000 verification

The Linux development environment rendered all 45 Markdown files using Pandoc GFM. The local reference check resolved 1,184 relative links and anchors. It matched 90 atomic requirement IDs and mappings, 25 plugin IDs and names, 61 agent rule IDs, 51 plan IDs and their direct prerequisites, and 16 scenario IDs across languages. Git checked the staged diff for whitespace and scope. Windows builds, driver loading, GUI runs and performance are tracked in the roadmap.

## Decisions and open assumptions

| ID | Status | Evidence and follow-up |
|---|---|---|
| ADR-000 | Owner confirmed | Publish the project as `zemeng5208/winstrata` under Apache-2.0. The initial GitHub commit contains the license. Preserve it. Add a NOTICE only if later material adds an applicable notice obligation. |
| ADR-001 | Pending owner review | Adopt linked bilingual documentation under `docs/` and `docs/zh-CN/`, stable plan and rule IDs, a single engineering record, and the proposed component directory layout. |
| AS-001 | Unverified | Verify native Linux Windows-driver build, links and signing in PR 003. Also verify real Windows load. |
| AS-002 | Unverified | Verify runtime isolation, IPC, native collection and frontend compatibility in PR 004 and PR 015. |
| AS-003 | Pending decision | Confirm accounting and cleanup semantics, untagged metadata retention, WebView2 themes, Windows support, and performance methods in the [open decision list](../roadmap/PLAN.md#section-19). |

## Errors and handover

| ID | State | Discovery, impact and retest |
|---|---|---|
| ERR-000 | Resolved | On 2026-11-27 GitHub HTTPS denied development-branch push with HTTP 403. There was no personal-account installation covering the repository. The owner configured it and a single development-branch dry-run succeeded. |
| ERR-001 | Resolved by owner configuration | GitHub returned HTTP 403 for ruleset creation by the integration. The owner saved the protection ruleset. Retrieving the branch, rulesets and effective branch rules confirmed active enforcement. The integration still lacks the settings permission. |

Continue on `pr/000-project-foundation` from the recorded baseline. Check that the remote base, actual PR and effective protections match before the owner merges. After merge, fetch main and record the verified foundation before starting PR 001. For the next record, use the [standard record fields](../standards/DOCUMENTATION.md#section-12).
