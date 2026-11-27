# WinStrata Agent Instructions

[简体中文](docs/zh-CN/AGENTS.md) · [Documentation index](docs/INDEX.md)

These rules take effect when the owner approves and merges PR 000. Before work, read all applicable standards and the governing product, architecture, interface, and acceptance documents.

- Follow the owner-approved architecture, scope, dependencies, and merge ordering.
- Use development branches and pull requests. Direct pushes to main, force pushes, protection bypass, and agent merges are prohibited.
- Verify real environments and full workflows on the actual merge candidate. Fix confirmed defects and rerun checks. Record incomplete verification.
- Preserve least privilege and local data boundaries. Follow the [specified component sources, three themes, and layout rules](docs/architecture/FRONTEND.md) under [AG-30](docs/standards/ARCHITECTURE.md#ag-30), and the replaceable-GUI rule [AG-64](#ag-64).
- Follow the [preflight, documentation, PR, self-review, and handover checklist](docs/standards/AGENT_WORKFLOW.md#execution-checklist).

<a id="ag-24"></a>

## AG-24 Governance precedence and integrity

Project rules rank in order from P0, the owner's latest explicit approved decision, through P1, this file, P2, project architecture and technical standards, P3, module instructions and designs, P4, approved PR scope and acceptance, and P5, existing code. Report conflicts and apply the higher-level rule. Changing a rule, test, or acceptance condition to pass faulty code is prohibited. Record the basis, impact, and approval for changes under AG-60.

<a id="ag-58"></a>

## AG-58 Layered standards

Keep project red lines and the index at the root. Maintain detailed rules for development, architecture, Git and PRs, testing, security, documentation, dependencies, and agent workflow. Module instructions add specific requirements. Read every applicable layer. Preserve all 64 rules, each with one authoritative definition and links from related documents. Audit direct and indirect references when changing a rule.

<a id="ag-59"></a>

## AG-59 PR 000

After repository initialization, submit the standards, architecture, complete roadmap and ordering, directory and dependency plans, configuration, documentation and error governance, requirement tracking, quality and acceptance plans, and license for owner review and merge. Keep the PR limited to the project foundation. Formal feature development starts only after this PR merges.

<a id="ag-60"></a>

## AG-60 Approval for standards changes

Request substantive mandatory-rule changes in their own PR. Include the current rule, proposed wording, reason, supporting evidence, necessity, and impact. The change takes effect after owner approval and merge. Keep rule changes out of ordinary feature PRs and preserve prior decisions. Recheck linked documents, automated gates, and open PRs after merge.

<a id="ag-62"></a>

## AG-62 Unaccepted-item tracking

Every unimplemented, unaccepted, partially accepted, untested, technical-assumption, known-limitation, and deferred item has a unique ID and a persistent record in the [open-item register](docs/records/OPEN_ITEMS.md). Each record includes the related requirement, source PR, current state, reason it is incomplete, responsible PR, handoff owner, verification environment, acceptance criteria, and close conditions. Recording only “awaiting verification” without a clear follow-up plan is prohibited. At the start of development, at PR submission, after merge, on context compression, and at agent handoff, check every still-open item so none is dropped. A confirmed defect must not be disguised as a technical limitation or an unaccepted item. It is fixed and retested, and it blocks the next in-order PR merge. Items that cannot be verified because of an environment limit follow the existing environment rules, stay recorded honestly, and still have a clear later acceptance plan.

<a id="ag-63"></a>

## AG-63 Error management and closure

Enforce [AG-25](docs/standards/DOCUMENTATION.md#ag-25). Keep the independent [global error register](docs/records/ERRORS.md) and the module error registers under [docs/records/modules/](docs/records/modules/core.md). Errors must not live only in STATUS.md, a PR body, or a chat session. Every error has a unique Error ID, discovery time, related PR, environment, reproduction method, evidence, root cause, impact, fix plan, retest result, and current state. Mark unknowns explicitly. Never invent facts. Register CI failures, regressions, security issues, data-integrity risks, and confirmed functional defects promptly. Unresolved errors must not be deleted, hidden, downgraded, or closed without authorization. After every PR merge, recheck the actual main branch. On a regression, update the error record, the requirements matrix, and acceptance status immediately, and block the next in-order PR merge. Global and module error records cross-link, stay bilingual, and keep resolved errors permanently in history.

<a id="ag-64"></a>

## AG-64 Replaceable GUI and official source tracking

WinStrata must allow official and third-party plugins to extend, modify, and fully replace the GUI. Registering a standalone page or a dashboard widget is not the limit. A plugin may change the global theme, colors, materials, fonts, motion, and interaction; change the global layout, navigation, sidebar, header, and dashboard; replace shared UI components and the presentation of another plugin; and provide a complete third-party GUI that replaces the application interface. Frontend plugins are installed, enabled, disabled, updated, and uninstalled, and the default interface can be restored, through the existing plugin market in the [extensions standard](docs/architecture/EXTENSIONS.md#section-5). Do not create a second market. Do not run more than one complete GUI and rendering environment at the same time.

The official default GUI is built in, cannot be uninstalled, and is both the default interface and the recovery interface. The official GUI and official plugins follow the four approved sources, the three shared themes, and the layout bounds in the [frontend standard](docs/architecture/FRONTEND.md#section-9) under [AG-30](docs/standards/ARCHITECTURE.md#ag-30). They reuse upstream source and existing implementations. Rewriting a component from its visual style, inventing a parallel component, or inventing a new visual system is prohibited. Combining existing components, binding real business data, adapting behavior, and configuring themes is allowed. Small layout, spacing, and content adjustments of the shadcn-admin shell are allowed. A missing component is submitted with the gap, the candidates, and the evidence, and needs owner approval before a new implementation or another source. Third-party GUI plugins may use their own design system. They still meet the security, compatibility, and performance contracts.

The GUI is decoupled from Core, business plugins, storage, collection services, and kernel drivers. Replacing the GUI must not stop background monitoring, restart unrelated services, or lose data. A third-party GUI must not bypass permissions, security isolation, rescue authorization, or deletion confirmation. The trusted host performs those interactions. A third-party GUI must not forge authorization. Authorized business capabilities, data, and events are obtained through one frontend SDK and API.

This rule does not select a technical stack. A later plan researches, verifies, and proposes the approach for owner approval before implementation. Every component and layout reused by the official GUI or an official plugin is entered in the [GUI source register](docs/records/GUI_SOURCES.md) with the upstream repository, version, source path, license, reuse method, and modifications. The register describes files present in that same commit. Schedule pressure is not a reason to drop a requirement. An empty interface, a fake plugin, a fake theme switch, or a static demonstration is not completion. New capabilities stay in the requirements matrix and in real acceptance scenarios.

## Complete rule index

| ID | Authoritative rule |
|---|---|
| AG-01 | [standards/DEVELOPMENT.md](docs/standards/DEVELOPMENT.md#ag-01) |
| AG-02 | [standards/DEVELOPMENT.md](docs/standards/DEVELOPMENT.md#ag-02) |
| AG-03 | [standards/DEVELOPMENT.md](docs/standards/DEVELOPMENT.md#ag-03) |
| AG-04 | [standards/DEVELOPMENT.md](docs/standards/DEVELOPMENT.md#ag-04) |
| AG-05 | [standards/DOCUMENTATION.md](docs/standards/DOCUMENTATION.md#ag-05) |
| AG-06 | [standards/DOCUMENTATION.md](docs/standards/DOCUMENTATION.md#ag-06) |
| AG-07 | [standards/AGENT_WORKFLOW.md](docs/standards/AGENT_WORKFLOW.md#ag-07) |
| AG-08 | [standards/DEPENDENCIES.md](docs/standards/DEPENDENCIES.md#ag-08) |
| AG-09 | [standards/DEVELOPMENT.md](docs/standards/DEVELOPMENT.md#ag-09) |
| AG-10 | [standards/DEVELOPMENT.md](docs/standards/DEVELOPMENT.md#ag-10) |
| AG-11 | [standards/AGENT_WORKFLOW.md](docs/standards/AGENT_WORKFLOW.md#ag-11) |
| AG-12 | [standards/GIT_AND_PR.md](docs/standards/GIT_AND_PR.md#ag-12) |
| AG-13 | [standards/GIT_AND_PR.md](docs/standards/GIT_AND_PR.md#ag-13) |
| AG-14 | [standards/GIT_AND_PR.md](docs/standards/GIT_AND_PR.md#ag-14) |
| AG-15 | [standards/TESTING.md](docs/standards/TESTING.md#ag-15) |
| AG-16 | [standards/TESTING.md](docs/standards/TESTING.md#ag-16) |
| AG-17 | [standards/TESTING.md](docs/standards/TESTING.md#ag-17) |
| AG-18 | [standards/TESTING.md](docs/standards/TESTING.md#ag-18) |
| AG-19 | [standards/DEPENDENCIES.md](docs/standards/DEPENDENCIES.md#ag-19) |
| AG-20 | [standards/AGENT_WORKFLOW.md](docs/standards/AGENT_WORKFLOW.md#ag-20) |
| AG-21 | [standards/DEVELOPMENT.md](docs/standards/DEVELOPMENT.md#ag-21) |
| AG-22 | [standards/DEVELOPMENT.md](docs/standards/DEVELOPMENT.md#ag-22) |
| AG-23 | [standards/SECURITY.md](docs/standards/SECURITY.md#ag-23) |
| AG-24 | [AGENTS.md](#ag-24) |
| AG-25 | [standards/DOCUMENTATION.md](docs/standards/DOCUMENTATION.md#ag-25) |
| AG-26 | [standards/DEVELOPMENT.md](docs/standards/DEVELOPMENT.md#ag-26) |
| AG-27 | [standards/GIT_AND_PR.md](docs/standards/GIT_AND_PR.md#ag-27) |
| AG-28 | [standards/DOCUMENTATION.md](docs/standards/DOCUMENTATION.md#ag-28) |
| AG-29 | [standards/DOCUMENTATION.md](docs/standards/DOCUMENTATION.md#ag-29) |
| AG-30 | [standards/ARCHITECTURE.md](docs/standards/ARCHITECTURE.md#ag-30) |
| AG-31 | [standards/DEVELOPMENT.md](docs/standards/DEVELOPMENT.md#ag-31) |
| AG-32 | [standards/ARCHITECTURE.md](docs/standards/ARCHITECTURE.md#ag-32) |
| AG-33 | [standards/DEVELOPMENT.md](docs/standards/DEVELOPMENT.md#ag-33) |
| AG-34 | [standards/DEVELOPMENT.md](docs/standards/DEVELOPMENT.md#ag-34) |
| AG-35 | [standards/DEPENDENCIES.md](docs/standards/DEPENDENCIES.md#ag-35) |
| AG-36 | [standards/DOCUMENTATION.md](docs/standards/DOCUMENTATION.md#ag-36) |
| AG-37 | [standards/TESTING.md](docs/standards/TESTING.md#ag-37) |
| AG-38 | [standards/DEVELOPMENT.md](docs/standards/DEVELOPMENT.md#ag-38) |
| AG-39 | [standards/DEPENDENCIES.md](docs/standards/DEPENDENCIES.md#ag-39) |
| AG-40 | [standards/DEPENDENCIES.md](docs/standards/DEPENDENCIES.md#ag-40) |
| AG-41 | [standards/DEVELOPMENT.md](docs/standards/DEVELOPMENT.md#ag-41) |
| AG-42 | [standards/ARCHITECTURE.md](docs/standards/ARCHITECTURE.md#ag-42) |
| AG-43 | [standards/ARCHITECTURE.md](docs/standards/ARCHITECTURE.md#ag-43) |
| AG-44 | [standards/ARCHITECTURE.md](docs/standards/ARCHITECTURE.md#ag-44) |
| AG-45 | [standards/ARCHITECTURE.md](docs/standards/ARCHITECTURE.md#ag-45) |
| AG-46 | [standards/ARCHITECTURE.md](docs/standards/ARCHITECTURE.md#ag-46) |
| AG-47 | [standards/SECURITY.md](docs/standards/SECURITY.md#ag-47) |
| AG-48 | [standards/GIT_AND_PR.md](docs/standards/GIT_AND_PR.md#ag-48) |
| AG-49 | [standards/GIT_AND_PR.md](docs/standards/GIT_AND_PR.md#ag-49) |
| AG-50 | [standards/AGENT_WORKFLOW.md](docs/standards/AGENT_WORKFLOW.md#ag-50) |
| AG-51 | [standards/AGENT_WORKFLOW.md](docs/standards/AGENT_WORKFLOW.md#ag-51) |
| AG-52 | [standards/TESTING.md](docs/standards/TESTING.md#ag-52) |
| AG-53 | [standards/TESTING.md](docs/standards/TESTING.md#ag-53) |
| AG-54 | [standards/DEPENDENCIES.md](docs/standards/DEPENDENCIES.md#ag-54) |
| AG-55 | [standards/DEPENDENCIES.md](docs/standards/DEPENDENCIES.md#ag-55) |
| AG-56 | [standards/DEPENDENCIES.md](docs/standards/DEPENDENCIES.md#ag-56) |
| AG-57 | [standards/AGENT_WORKFLOW.md](docs/standards/AGENT_WORKFLOW.md#ag-57) |
| AG-58 | [AGENTS.md](#ag-58) |
| AG-59 | [AGENTS.md](#ag-59) |
| AG-60 | [AGENTS.md](#ag-60) |
| AG-61 | [standards/DOCUMENTATION.md](docs/standards/DOCUMENTATION.md#ag-61) |
| AG-62 | [AGENTS.md](#ag-62) |
| AG-63 | [AGENTS.md](#ag-63) |
| AG-64 | [AGENTS.md](#ag-64) |
