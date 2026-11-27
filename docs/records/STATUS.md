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
| PR 002 | Accepted | Both GitHub Actions checks are required on an up-to-date branch. Effective rules and PR 004 results verified. See ERR-004. |
| PR 003 | Merged with open acceptance items | The owner merged [GitHub PR #4](https://github.com/zemeng5208/winstrata/pull/4) as `f8809cc68c97ea841ca02ebe5e8c7a17a6287994`. Its tree matched the submitted tree. Linux workspace and both x64 link probes passed on that baseline. Packaging, signing and actual driver loading remain open. |
| PR 004 | Accepted for platform feasibility | The owner merged [GitHub PR #5](https://github.com/zemeng5208/winstrata/pull/5) as `36c12ddf45b6f2eb3b0f3ec401e0942f3fce14ee`. The file tree matched the reviewed head. Postmerge Linux, Windows and policy checks passed. |
| PR 005 | Accepted | The owner merged [protocols PR #6](https://github.com/zemeng5208/winstrata/pull/6). Main commit `14a8ec6` has the same tree as reviewed head `e446453`. |
| PR 006 | In preparation | The core configuration and per-user storage branch starts at the verified PR 005 merge. |
| PR 007–050 | Planned | Follow the approved roadmap and verify prerequisites. |
| Core and PL-01 through PL-25 | Planned | Runtime and metrics remain unverified. Maintain their evidence in requirements and module records as work lands. |

## PR 000 verification

The Linux development environment rendered all 45 Markdown files using Pandoc GFM. The local reference check resolved 1,184 relative links and anchors. It matched 90 atomic requirement IDs and mappings, 25 plugin IDs and names, 61 agent rule IDs, 51 plan IDs and their direct prerequisites, and 16 scenario IDs across languages. Git checked the staged diff for whitespace and scope. Windows builds, driver loading, GUI runs and performance are tracked in the roadmap.

## PR 001 verification

On Linux x86_64, Rust and Cargo 1.100.0 with GCC 14.2, Node.js 24.22.3, and pnpm 11.25.0 ran `cargo dev doctor`, `cargo dev init`, `cargo dev build`, and `cargo dev verify` successfully. Cargo built the actual developer executable. The test harness reported zero unit tests in this phase. The executable and package-manager workflows were exercised directly.

With Node.js or pnpm removed separately from the command search path, the preflight exited with status 1 and identified the missing tool. In a clean source copy moved to a directory with a space in its name, the commands ran from the nested Rust crate and used the moved Cargo workspace, target directory, and pnpm store. The external host toolchain was reused. Windows host execution remains unverified. Git ignore checks passed for the generated Rust target, pnpm install directory, and workspace-local cache.

## PR 002 verification

On Linux x86_64, the CI bootstrap verified the official Node archive checksum, installed the pinned pnpm, and resolved the approved Rust toolchain. Seven Node tests cover plan order, defect blockers, owner confirmation bound to the exact head, and current test merge and base verification. The local status adapter test checks that the result is published to the same head commit as the GitHub Actions check. The workspace script ran the policy tests and the approved developer commands. The existing Protect main rules are active. Hosted status results and required-rule activation are recorded through the PR and the effective rules.

## PR 003 verification

On Linux x86_64 with Rust 1.100.0, bundled LLD 23.1.5 and Binutils 2.44, `cargo dev cross-probe` compiled COFF and linked the console and kernel PE32+ images twice. The binaries were byte-identical. Verified the expected imports, two subsystems and nonzero entry points, and inspected the driver checksum and empty signature directory. Checked both original package SHA-512 values and the extracted library digests. In a relocated checkout with a space in its path, verified missing-library and changed-checksum failures, restored the approved inputs, and successfully built from the nested tool directory. Images and Microsoft inputs stayed in ignored directories. No Windows program or driver was executed.

## PR 004 verification

Windows Server 2022 x64 exercised CIM and Event Log, Job Objects, an AppContainer file grant and revocation, local MCP and the three upstream components offline in WebView2. Linux regressions covered the workspace, both link probes, local transport, dependency resolution and upstream components. The selected UI dependencies were authorized for disposable trials. The [platform report](../architecture/PLATFORM_FEASIBILITY.md) records the measurements, controls, original runs and limits.

Phase 0 acceptance covers the tested toolchain, enforced merge gates and feasibility probes. The three workflows passed on the actual PR 004 merge commit. AS-001 and AS-002 track the production and client checks carried into later phases. [Linux CI](https://github.com/zemeng5208/winstrata/actions/runs/45969082288) · [Windows feasibility](https://github.com/zemeng5208/winstrata/actions/runs/45969082284) · [Merge policy](https://github.com/zemeng5208/winstrata/actions/runs/45969082364)

## PR 005 verification

On the candidate for `995d148`, Linux passed 11 library tests and two integration tests. Windows Server 2022 x64 also passed a fourteenth test reading a real System event and querying it again by record ID to compare its provider and native timestamp. A live Node child verified its PID. Rust and Node exchanged events and errors with precision above the JavaScript safe-integer range. Review found that the default decoder accepted positional arrays or numeric enum tags. The named-object and string-tag checks rejected the original cases on both systems. All six JSON blocks across the two document languages passed typed decode, encode and decode again. Linux also type-checked the tests for the Windows MSVC x64 target. [Linux workspace](https://github.com/zemeng5208/winstrata/actions/runs/45971619572/job/140413162125) · [Windows protocols](https://github.com/zemeng5208/winstrata/actions/runs/45971619572/job/140413162795)

After merge, the workspace, Windows protocol and platform checks passed again. Both required status contexts and the strict latest-base rule remain active. [Main CI](https://github.com/zemeng5208/winstrata/actions/runs/45972348949) · [Windows platform](https://github.com/zemeng5208/winstrata/actions/runs/45972349084) · [Merge policy](https://github.com/zemeng5208/winstrata/actions/runs/45972348858)

## Decisions and open assumptions

| ID | Status | Evidence and follow-up |
|---|---|---|
| ADR-000 | Owner confirmed | Publish the project as `zemeng5208/winstrata` under Apache-2.0. The initial GitHub commit contains the license. Preserve it. Add a NOTICE only if later material adds an applicable notice obligation. |
| ADR-001 | Accepted in PR 000 | Adopt linked bilingual documentation under `docs/` and `docs/zh-CN/`, stable plan and rule IDs, a single engineering record, and the proposed component directory layout. |
| ADR-002 | Owner confirmed | Pin Rust and Cargo 1.100.0, Node.js 24.22.3 LTS, and pnpm 11.25.0. Use Cargo and pnpm root workspaces and keep this phase free of external Rust or JavaScript packages. |
| ADR-003 | Owner confirmed | Use the official Microsoft WDK and Windows SDK x64 10.0.26100.6584 import libraries locally under their respective tool licenses for the Linux probe. |
| ADR-004 | Owner confirmed | Begin Phase 1 with the initial event, object, error and version contracts. Use `serde` 1.0.229 with `derive`, `serde_json` 1.0.154, and their locked transitive dependencies for JSON encoding and decoding. [Protocol contracts](../architecture/PROTOCOLS.md) |
| AS-001 | Partially verified | Linux linked both x64 probes. PR 008 needs a validated INF/CAT package, Microsoft signing and representative load/unload on Windows with Secure Boot and HVCI enabled. PR 050 must verify the full Linux cross-build. |
| AS-002 | Partially verified | The [platform report](../architecture/PLATFORM_FEASIBILITY.md) records Server 2022 probes. Verify product authorization in PR 008, mixed-runtime containment in PR 015, ordinary-user GUI in PR 016, sustained performance in PR 048 and Windows 10/11 clients in PR 049. |
| AS-003 | Pending decision | Confirm accounting and cleanup semantics, untagged metadata retention, WebView2 themes, Windows support, and performance methods in the [open decision list](../roadmap/PLAN.md#section-19). |

## Errors and handover

| ID | State | Discovery, impact and retest |
|---|---|---|
| ERR-000 | Resolved | On 2026-11-27 GitHub HTTPS denied development-branch push with HTTP 403. There was no personal-account installation covering the repository. The owner configured it and a single development-branch dry-run succeeded. |
| ERR-001 | Resolved by owner configuration | GitHub returned HTTP 403 for ruleset creation by the integration. The owner saved the protection ruleset. Retrieving the branch, rulesets and effective branch rules confirmed active enforcement. The integration still lacks the settings permission. |
| ERR-002 | Resolved in the Linux workspace | PR 001 preflight found no Rust or Cargo on PATH. The approved official toolchain was installed into the project’s ignored local tooling directory after verifying the Rustup installer checksum. Both version checks and the actual build passed. |
| ERR-003 | Resolved | The first Linux driver link failed because the selected x64 WDK import archive did not export `KeQueryInterruptTime`. Inspected the archive, switched to the documented export `KeQueryTimeIncrement`, and reran both probes and PE inspection. |
| ERR-004 | Resolved on PR 004 | PR 003 merged with a failed policy status while only Workspace checks was required and strict latest-base was off. PR 004 passed both checks on head `1788410`. The effective rules require `Workspace checks`, `winstrata/merge-policy` and an up-to-date branch, with GitHub Actions as the source for both checks and no bypass actors. [PR 003 policy failure](https://github.com/zemeng5208/winstrata/actions/runs/45964752411) · [PR 003 main CI](https://github.com/zemeng5208/winstrata/actions/runs/45965182726) · [PR 004 policy success](https://github.com/zemeng5208/winstrata/actions/runs/45968732577) · [Effective rules](https://api.github.com/repos/zemeng5208/winstrata/rules/branches/main) |

Continue on `pr/006-core-user-storage` from `14a8ec6`. PR 006 covers current-user identity, runtime configuration, directory selection, migration preflight and real permissions.
