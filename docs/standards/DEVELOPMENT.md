# Development standard

[简体中文](../zh-CN/standards/DEVELOPMENT.md) · [Index](../INDEX.md)

<a id="ag-01"></a>

## AG-01 Architecture decision authority

Follow approved architecture, stack, interfaces, and requirements. The owner approves replacements, removals, and changes in standards. When a defect or blocker appears, pause affected work and present evidence, impact, and alternatives for approval. Continue independent work.

<a id="ag-02"></a>

## AG-02 Root causes and validity

Investigate root causes. Preserve tests, checks, dependency and compatibility requirements, and performance and security standards. Empty implementations, hardcoded outcomes, invented data, swallowed failures, fake success, and masking patches are prohibited. Report unresolved issues with evidence and keep unverified work pending.

<a id="ag-03"></a>

## AG-03 Approved scope

Limit changes to the current approved PR. Obtain approval before extending scope, interfaces, dependencies, directories, or future work. Preserve other contributors' uncommitted changes. Review the entire Git diff before submission.

<a id="ag-04"></a>

## AG-04 Preflight and documentation

Read root and module instructions, architecture, and interfaces. Check the branch, working tree, existing code and tests, current PR position and dependencies, conflicts, tools and dependency paths, plan, acceptance, and documentation impacts. Report real checks, state, issues, and evidence. Pause work whose prerequisites fail. In the same PR, update affected documents, roadmap, and progress for changes to code, interfaces, configuration, dependencies, or behavior. Include documentation consistency in quality gates. Keep authoritative requirements intact when correcting defects.

<a id="ag-09"></a>

## AG-09 Maintainability and debt

Avoid duplicated logic, oversized mixed responsibilities, cycles, cross-layer coupling, unnecessary indirection, hardcoded settings, dead code, deprecated interfaces, and unapproved temporary implementations. Document clear interfaces, types, errors, and duties. Record debt, its cause, risk, impact, and proposed fix. Request out-of-scope fixes and plan their order. Validate and approve quantitative complexity thresholds.

<a id="ag-10"></a>

## AG-10 Evidence and design review

Raise conflict, infeasibility, risk, performance problems, and alternatives with verified evidence. Identify facts, inferences, and assumptions and compare tradeoffs. The owner decides changes. Update linked documentation and the PR plan after approval.

<a id="ag-21"></a>

## AG-21 Feature completeness

Implement the entire approved PR scope and meet each approved stage's criteria. Report blockers and keep incomplete work out of merge requests. Unapproved MVPs, deferred requirements, and presenting a partial implementation as complete are prohibited.

<a id="ag-22"></a>

## AG-22 Production and debug separation

Keep debug interfaces, development backdoors, permission bypasses, test hardcoding, personal paths, unconditional high-frequency logs, and obsolete temporary code out of production paths. Use conditional log levels and separate development tools. Preserve authentic data and performance under debugging. Development builds must meet the same security and stability standards.

<a id="ag-26"></a>

## AG-26 Simplicity

Prefer the simplest reliable maintainable solution that meets the requirements. Reuse approved infrastructure and open source. Every module, interface, abstraction, dependency, config, script, and document has a concrete purpose. Preserve requirements while simplifying.

<a id="ag-31"></a>

## AG-31 Repository-wide reuse

Search the repository for reusable components, tools, representations, collection, storage, permissions, protocols, UI, and services. Use existing implementations where applicable and check upstream and downstream effects. Keep one implementation per shared capability and avoid duplicated collection. Record duplications and request refactoring approval. Preserve separation of duties. Update affected SDK, tests, and docs and verify during preflight and review.

<a id="ag-33"></a>

## AG-33 Removals and intermediate artifacts

When removing a feature, recursively clean its implementation, entry points, components, routes, settings, permissions, translations, dedicated dependencies, tests, build paths, plugin, SDK, MCP, and documentation. Preserve shared capabilities and historical decisions. Check dangling references, orphan files, and reintroduced behavior. Before ending module, PR, or extended work, inspect the working tree and ignored artifacts. Clean your obsolete temporary patches, tests, backups, and worktrees while preserving user data, other contributors' work, and useful caches.

<a id="ag-34"></a>

## AG-34 Data authenticity

Trace presentation, metrics, and analysis to real sources and computation, range, and time. Separate measured values from inferences and confidence, missing and failed collection, and unsupported values. Preserve accuracy and coverage. Never use cached, default, invented, zero, or random values as current measurements. Use consistent semantics in GUI, exports, plugins, and MCP and verify them in integrated tests.

<a id="ag-38"></a>

## AG-38 Central development tools

Provide a unified entry point for setup, build, run, tests, checks, performance, and cleanup with consistent arguments and output across environments where practical. Reuse existing tools and remove temporary ones. Document duties and errors for approved tools and apply PR and verification rules.

<a id="ag-41"></a>

## AG-41 Architecture audit

Review responsibilities, dependency direction, duplicated services, Core and plugin couplings, interface bypasses, centralized dependency/config/tool/storage management, the tree, security, and architecture docs for every PR. Delegated scans require lead verification. Record the audit, fix violations, retest, and block later merges on confirmed issues.
