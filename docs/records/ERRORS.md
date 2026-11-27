# Global error register

[简体中文](../zh-CN/records/ERRORS.md) · [Open items](OPEN_ITEMS.md) · [Progress](STATUS.md) · [Index](../INDEX.md)

This is the global error register required by [AG-25](../standards/DOCUMENTATION.md#ag-25) and [AG-63](../../AGENTS.md#ag-63). [STATUS.md](STATUS.md) links here and is not a substitute. Module-owned errors are authoritative in the module file. Repository and CI errors below are authoritative here. Resolved errors stay in this history. Unknowns are written as unknown. This plan does not change product code.

ADR-001 still uses STATUS.md as the engineering record for progress and decisions. That decision does not replace this register.

## Index

| ID | State | Authority |
|---|---|---|
| ERR-000 | Resolved | This file |
| ERR-001 | Resolved | This file |
| ERR-002 | Resolved | [Toolchain](modules/toolchain.md#err-002) |
| ERR-003 | Resolved | [Kernel](modules/kernel.md#err-003) |
| ERR-004 | Resolved | This file |
| ERR-005 | Resolved | [Core](modules/core.md#err-005) |
| ERR-006 | Resolved | This file |
| ERR-007 | Resolved | [Kernel](modules/kernel.md#err-007) |
| ERR-008 | Resolved | [Kernel](modules/kernel.md#err-008) |
| ERR-009 | Resolved | [Core](modules/core.md#err-009) |
| ERR-010 | Resolved | [Core](modules/core.md#err-010) |
| ERR-011 | Resolved | [Core](modules/core.md#err-011) |
| ERR-012 | Resolved | [Core](modules/core.md#err-012) |
| ERR-013 | Resolved | [Core](modules/core.md#err-013). Same defect as ERR-005. |
| ERR-014 | Resolved | [Core](modules/core.md#err-014) |
| ERR-015 | Resolved | [Protocols](modules/protocols.md#err-015) |
| ERR-016 | Resolved | [Core](modules/core.md#err-016) |
| ERR-017 | Resolved | [Services](modules/services.md#err-017) |
| ERR-018 | Resolved | [Core](modules/core.md#err-018) |

`plugins/strata-script` has no open error. Its register is [StrataScript](modules/strata-script.md). The Windows AppContainer gap is [OI-040](OPEN_ITEMS.md#oi-040), not an error.

ERR-005 is the Windows process-exit failure on main `a2471d5fc13f0a307ee39d2629c454aea4537809`. ERR-013 names the same defect. Plan 052 is the designated fix and is merged as `004974215e239f96160b47d3eef40a84c316e802`. Windows workspace checks on that commit passed. Plan 051 does not implement the fix.

<a id="err-000"></a>

## ERR-000

- Error ID: ERR-000
- Discovery time: 2026-11-27. A clock time was not recorded.
- Related PR: repository setup, before plan 000 merged
- Environment: GitHub HTTPS push of a development branch
- Reproduction method: push the development branch over HTTPS
- Evidence: GitHub returned HTTP 403. No personal-account installation covered the repository. After the owner configured it, one development-branch dry-run succeeded.
- Root cause: the integration had no installation covering the repository
- Impact: development-branch push was denied
- Fix plan: owner configuration. Done.
- Retest result: the dry-run push succeeded
- Current state: resolved

<a id="err-001"></a>

## ERR-001

- Error ID: ERR-001
- Discovery time: 2026-11-27. A clock time was not recorded.
- Related PR: repository protection, before the ruleset was active
- Environment: GitHub ruleset API
- Reproduction method: create the protection ruleset through the integration
- Evidence: GitHub returned HTTP 403. The owner saved [Protect main](https://github.com/zemeng5208/winstrata/rules/28217627). Reading the branch, rulesets, and effective rules confirmed enforcement. The integration still lacks the settings permission.
- Root cause: the integration was not allowed to create the ruleset
- Impact: automated ruleset creation failed. Protection was then saved by the owner.
- Fix plan: owner saved the ruleset. Done. The missing settings permission remains a recorded limit of the integration, not an open product defect.
- Retest result: effective rules showed the ruleset active
- Current state: resolved

<a id="err-004"></a>

## ERR-004

- Error ID: ERR-004
- Discovery time: 2026-11-27, during plan 003 merge
- Related PR: GitHub PR #4, plan 003
- Environment: GitHub Actions merge policy and branch protection on main
- Reproduction method: merge plan 003 while only Workspace checks was required and the strict latest-base rule was off
- Evidence: [PR 003 policy failure](https://github.com/zemeng5208/winstrata/actions/runs/45964752411). [PR 003 main CI](https://github.com/zemeng5208/winstrata/actions/runs/45965182726). Plan 004 head `1788410` passed both checks. [PR 004 policy success](https://github.com/zemeng5208/winstrata/actions/runs/45968732577). [Effective rules](https://api.github.com/repos/zemeng5208/winstrata/rules/branches/main).
- Root cause: the required checks and the latest-base rule were not yet the rules that later blocked a failed policy status
- Impact: plan 003 merged with a failed policy status
- Fix plan: the owner required `Workspace checks`, `winstrata/merge-policy`, and an up-to-date branch, with GitHub Actions as the source and no bypass actors. Done before plan 004 merged.
- Retest result: plan 004 passed both required checks
- Current state: resolved

<a id="err-006"></a>

## ERR-006

- Error ID: ERR-006
- Discovery time: 2026-11-27, on plan 052 head `d180ae58afdfff1ce826875abb577792159e98a7`
- Related PR: GitHub PR #17, plan 052, before merge
- Environment: GitHub Actions merge-policy workflow evaluating that head against main `a2471d5fc13f0a307ee39d2629c454aea4537809`
- Reproduction method: the workflow check “Evaluate merge policy from main” posted the commit status `winstrata/merge-policy`
- Evidence: the published status failed with “PR title and branch must identify a plan ID in the roadmap”. `mergeable_state` was blocked. The script checked out main, whose roadmap had no 052, so `order.indexOf("052")` was negative. The branch and title were already `pr/052-process-exit-sample` and `PR 052: ...`. Linux job 140620679852 succeeded at 2026-11-27T16:21:28Z, Windows job 140620680173 succeeded at 2026-11-27T16:22:25Z, and Workspace job 140621425280 succeeded at 2026-11-27T16:22:30Z. [Workspace run](https://github.com/zemeng5208/winstrata/actions/runs/46032831553) · [Policy run](https://github.com/zemeng5208/winstrata/actions/runs/46032831547). The plan 008 pipe failure is [ERR-017](modules/services.md#err-017).
- Root cause: the policy script read the roadmap from main, which did not yet contain plan 052
- Impact: that head was not a review head. Linux, Windows, and Workspace checks had succeeded.
- Fix plan: when a pull request changes `docs/roadmap/PLAN.md`, read that file from the verified test merge and still reject a dropped or reordered existing ID. Implemented in `a6a22c200eca6d67f5cffff6f370d0537567850e` and merged as `004974215e239f96160b47d3eef40a84c316e802`.
- Retest result: the check “Evaluate merge policy from main” on that merge succeeded. [Policy run](https://github.com/zemeng5208/winstrata/actions/runs/46036724065/job/140633881742). The commit-status API for the merge SHA returned no contexts when this record was written.
- Current state: resolved
