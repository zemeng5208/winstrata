# Documentation standard

[简体中文](../zh-CN/standards/DOCUMENTATION.md) · [Index](../INDEX.md)

<a id="ag-05"></a>

## AG-05 Evidence and development records

Update module and global records after a module or submodule, PR, extended session, significant change, blocker, or verified result. Record actual commands, environment, files, results, builds, tests, performance, remaining issues, and next actions. Mark unrun items unverified in the [open-item register](../records/OPEN_ITEMS.md), with the fields [AG-62](../../AGENTS.md#ag-62) requires. Preserve evidence in the repository. A lower-cost agent may organize supplied evidence. The lead verifies it and completes the work directly when delegation is unavailable.

<a id="ag-06"></a>

## AG-06 Context recovery and document consistency

On handover, context reduction, interruption, or agent change, recover instructions, decisions, progress, preceding PRs, errors, and the current handover. Check every still-open item in the [open-item register](../records/OPEN_ITEMS.md) and every unresolved error in the [error register](../records/ERRORS.md). Report the checks. [AG-62](../../AGENTS.md#ag-62) applies. For every document change, recursively check parent and child documents, direct and indirect references, cross-module impact, obsolete examples, links, and bilingual semantics. Maintain the reference index, authoritative ownership, and the impact record for updates, unaffected documents, and pending approvals. A lower-cost review may assist. The lead verifies the result. Avoid unnecessary rewrites.

<a id="ag-25"></a>

## AG-25 Errors, degradation, and records

Use common error types, codes, and diagnostics. Keep detection, missing data, degradation, and recovery observable and traceable. Avoid silent failure, success on error, or unbounded retry. Isolate plugin failures from independent modules and sanitize diagnostics. Maintain separate global and module error records and retain resolved history under the [record schema](#section-12). The [global register](../records/ERRORS.md) and the [module registers](../records/modules/core.md) are those records. [STATUS.md](../records/STATUS.md) links to them and is not a substitute. Close only after retesting. Keep operational logs separate from engineering records and link defects to merge blocking under [AG-63](../../AGENTS.md#ag-63). A lower-cost audit may assist.

<a id="ag-28"></a>

## AG-28 Traceability and professional documentation

Link material changes to requirements, decisions, errors, and the PR. Record accurate commit and PR descriptions, ADRs, superseded decisions, and reasons, consequences, and approval for removals. Keep technical documents factual, concise, and free of private conversation, personal data, inflated claims, blame, and arguments. Convert informal input to formal requirements without changing meaning. Focus error records on facts, root cause, and resolution.

<a id="ag-29"></a>

## AG-29 Languages and comments

Use English for identifiers, paths, commits, PR titles and bodies, and protocol fields. Write concise Simplified Chinese code comments about intent, constraints, and meaningful changes. Provide complete Simplified Chinese and English formal docs and GUI text. Maintain detailed history in Git and development records. UI design documents cover behavior, layout, interaction, and relevant changes.

<a id="ag-36"></a>

## AG-36 Global progress

Maintain one authoritative view of PRs, core, all official plugins, atomic requirements, actual acceptance and evidence, errors, blockers, decisions, and next work. Derive completion from approved accepted tasks. Update regressions and align module and global status, including the [open-item register](../records/OPEN_ITEMS.md) and the [error register](../records/ERRORS.md). Lower-cost assistance may maintain records subject to lead verification.

<a id="ag-61"></a>

## AG-61 Requirements traceability

Assign every approved requirement a unique ID. Link its module, PR, acceptance scenario, and real evidence in the project-wide matrix. Preserve requirements through long sessions, context reduction, module integration, and agent transitions. Record actual implementation status, analyze every change for project-wide impact, keep the matrix in sync with global progress, and verify each requirement individually at final acceptance.

<a id="section-12"></a>

## Ownership and linked documents

The [index](../INDEX.md) points to parallel Simplified Chinese and English documents. Use stable IDs and a single authoritative source per fact. Module documents can add module-specific design, implementation, evidence, progress and errors within global rules. Keep relevant direct and indirect references, examples and translations in the same PR. Record affected documents, justified non-impact and pending approvals.

| Record | Fields |
|---|---|
| Requirement | ID, statement, authoritative source and approval, module, PR, scenario, actual evidence, status |
| ADR | ID, decision, grounds, alternatives, impact, approval, related superseded records |
| Progress | Plan ID, state, owner, prerequisites, acceptance, open issues, next action |
| Error | Error ID, discovery time, related PR, environment, reproduction method, evidence, root cause, impact, fix plan, retest result, current state, links. Mark unknown fields as unknown. |
| Open item | ID, related requirement, source PR, current state, reason it is incomplete, responsible PR, handoff owner, verification environment, acceptance criteria, close conditions |
| Assumption | ID, hypothesis, impact, method, environment, evidence, state, conclusion and approval |
| Verification | Base, environment, scenario, actual steps, expected and observed results, metrics, errors, evidence, verification state |
| Handover | Branch and commit, worktree, completed and pending work, blockers, docs and next responsibility |

Error states are open, fixing, awaiting verification, and resolved. Retain history and verify before closing. Progress states are planned, developing, awaiting verification, awaiting review, awaiting merge, and accepted. Update on regressed acceptance. Calculate completion from approved and accepted work. Use redacted evidence rather than raw private, large or sensitive material.
