# Agent workflow

[简体中文](../zh-CN/standards/AGENT_WORKFLOW.md) · [Index](../INDEX.md)

<a id="ag-07"></a>

## AG-07 Multiple agents

The lead assigns tasks, file boundaries, contracts, and acceptance. Coordinate shared files and isolate work when appropriate with branches or worktrees. Each contributor follows the applicable rules and scope. The lead reviews actual diffs, tests, and docs, integrates changes, and owns the result. Keep merge order intact. Match models to complexity.

<a id="ag-11"></a>

## AG-11 Autonomous work and approvals

Within approved scope, design, and acceptance, carry implementation, build, debugging, testing, and documentation through completion and report actual progress. Consult the owner on architecture, stack, dependencies, reused projects, scope, requirement conflicts, major blockers, high-risk or irreversible actions, and merges. Routine execution stays with the developer.

<a id="ag-20"></a>

## AG-20 Model and token cost

Use high-capability models for architecture, kernel work, complex logic, and security. Prefer lower-cost assistance for documentation, research, scans, and simple repetitive work. Load relevant context and reuse valid results. Reverify facts invalidated by changes. Minimize repeated output and save a handover early. Preserve quality, tests, security, and documentation completeness.

<a id="ag-50"></a>

## AG-50 Continuity and handover

The developer remains responsible for implementation, CI, review, conflicts, regression repair, and acceptance after submission or session changes. On handover, read standards and records, inspect current files and real tests, and continue the approved implementation through full acceptance before later work. Keep work and evidence already valid. Process defects through root-cause and approval rules.

<a id="ag-51"></a>

## AG-51 Session close

Before an extended session ends, check worktree and tree, actual builds and tests, linked docs, status, PRs, errors, temporary artifacts, blockers, and next steps. Write a resumable handover even if work is incomplete. Lower-cost assistance may organize real evidence without changing implementations, rules, or criteria. The lead verifies and completes the record if needed.

<a id="ag-57"></a>

## AG-57 Reporting

Report actual progress, findings, verification, defects, blockers, and decisions. Distinguish implemented, verified, awaiting verification, and incomplete. Keep routine reports concise and record substantive evidence in the repository. Avoid repeated requirements, empty plans, subjective percentages, inflated claims, and raw log dumps.

<a id="execution-checklist"></a>

## Execution checklist

### Before work

- Recover instructions, architecture, requirements, previous PRs and handover. Confirm plan ID and approved scope.
- Verify branch, base, worktree, concurrency, merge sequence and dependencies. Preserve existing edits.
- Check tools, permissions, environment, network and available assistance. Record constraints.
- Verify approved project dependencies, versions, lockfiles and resolved paths. Correct real gaps.
- Search existing code, interfaces, tests, reusable components and open source. Request approvals for proposed changes.
- Define execution, interfaces, security, performance, real acceptance and affected documents.

### During work

Proceed within approvals. Reproduce assumptions and interfaces. Maintain issue, evidence, progress and document links. Use proven infrastructure and concentrated configuration and dependencies. Fix verified defects, check related cases and rerun checks without fabricating behavior. Keep scope and approval decisions explicit.

### Before requesting review

Check approved scope and completeness, real build and Windows/GUI where applicable, integrated regression, performance and security. Record unrun checks. Inspect architecture, dependencies, reuse, settings, docs and tree, and remove obsolete temporary artifacts. Verify actual latest-base merge, semantic conflicts, avoidance of reintroduced behaviors, and corrected defects. Update tracking, evidence and linked documents. Request order and let the owner merge.

### After merge or session close

Fetch actual main after merge. Verify evidence and prerequisites and record blockers for the next merge. Update authoritative progress, requirements, issues and handover through the applicable PR. Record remaining work, worktree, temporary artifact cleanup and handover responsibility. Continue the same approved work through acceptance.
