# Git and PR standard

[简体中文](../zh-CN/standards/GIT_AND_PR.md) · [Index](../INDEX.md)

<a id="ag-12"></a>

## AG-12 Git and PR authority

Agents may create branches and worktrees, commit, push development branches, open and update PRs, and inspect CI. Direct pushes to main, force pushes, rewriting published history, bypassing protection, and agent merges are prohibited. Obtain approval for deleting branches, tags, or releases, closing other PRs, and changing order. Only the owner merges. Reverify later PRs against merged prerequisites.

<a id="ag-13"></a>

## AG-13 PR planning and merge order

Approve a detailed roadmap before feature development. Specify plan IDs, prerequisites, scope and paths, goals, contracts, exclusions, measurable tests, documentation, security, and performance. Request merge position, identify the base and prerequisites, and describe conflicts. Parallel work is allowed within the approved merge order. Additions, removals, splits, combinations, and reorderings require approval. Verify the real merge candidate on the latest target and check semantic conflicts, behavior regressions, and the return of removed features, dependencies, interfaces, or documents.

<a id="ag-14"></a>

## AG-14 Review and defect blocking

In single-maintainer work, self-review is allowed and an independent agent is optional. Audit architecture, logic, security, performance, tests, scope, obsolete behavior, and documentation. Resolve confirmed defects, rerun checks, and record evidence. A defect found after merge blocks the next merge in order. Independent development may continue.

<a id="ag-27"></a>

## AG-27 Postmerge verification

Fetch current main and inspect the actual merge, CI, integrated regressions, prior fixes, and the absence of removed behavior. Check errors, docs, PR and module status, and the next prerequisites. Block the next merge on failures. Use an approved PR for tracking updates.

<a id="ag-48"></a>

## AG-48 Automatable gates

Use approved CI and repository rules progressively for ordering, dependencies, architecture, reintroduced changes, references, hygiene, security, and quality. Reuse established tools and a shared framework. Check manual conditions in review. Verify enforcement, approve rule changes, and preserve protections.

<a id="ag-49"></a>

## AG-49 Phase acceptance and build responsibility

At phase close, verify every PR, features, core and plugins, security, performance, stability, architecture, dependencies, config, tree, docs, errors and debt, and next-phase prerequisites. Pause phase progression on acceptance failures. Developers build, test, and diagnose their available local or cloud environment. Keep both build capabilities project-wide and record unrun environments. A Linux build establishes compilation evidence, with Windows runtime verification tracked separately. Resolve confirmed cross-platform defects. Missing environment blocks only affected verification. This rule supersedes per-PR dual-build obligations.

<a id="section-13"></a>

## Repository gates and review

Use plan IDs independently of GitHub PR numbers. Request the merge position before submitting and describe scope, prerequisites, downstream PRs, shared interfaces and files, current base, risks and validation. Development may overlap within approved boundaries. Merge in approved order. The owner approves plan changes.

Protect main with an active ruleset or branch protection requiring PRs, preventing force pushes and preventing unauthorized direct updates. The owner reviews and merges. In the single-maintainer phase, GitHub need not require a separate approver. Verify the actual active rule and effective target. CI gates begin with known real checks in PR 002. Plan order, dependencies, defects, current merge candidate, removal and regression checks, architecture, hygiene, documentation, and security enforcement. Do not impose a nonexistent check or present a planned check as enabled. Where automation is incomplete, the owner blocks noncompliant merges. Merge queues and latest-base integration are implementation decisions for PR 002.

Before approval, update from actual main without rewriting published history. Verify semantic interaction with previous work and test the real merge candidate. Prior defects and removed behavior must stay resolved. Use the [PR template](../../.github/pull_request_template.md) for English submissions. Record each confirmed finding and its retest. After the owner merges, fetch and verify main, quality status, merged diff, regression results, decisions, records and next prerequisites. Block the next merge when any acceptance criterion fails.
