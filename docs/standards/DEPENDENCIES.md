# Dependencies and configuration

[简体中文](../zh-CN/standards/DEPENDENCIES.md) · [Index](../INDEX.md)

<a id="ag-08"></a>

## AG-08 Technical research and reuse

Verify third-party libraries and APIs against official documentation, source, and real behavior. Confirm interfaces, compatibility, and feasibility before relying on them. Research open-source reuse first. Compare coverage, gaps, licensing, maintenance, vulnerabilities, quality, performance, builds on both environments, and integration and replacement costs. Recommend direct integration, partial reuse, a fork, or implementation with evidence. Owner approval precedes introduction or copying. Preserve copyright and architecture requirements. Delegated screening requires lead verification. Record the basis for the selection.

<a id="ag-19"></a>

## AG-19 Repository hygiene

Exclude build outputs, caches, temporary files, debug logs, real telemetry and captures, databases, secrets, personal settings, unused scripts, stale or duplicate docs, unapproved binaries, and agent scratch material. Keep useful history, centralized build and temporary paths, and reproducible tools. Check ignores, secret exposure, and contamination before each commit and PR.

<a id="ag-35"></a>

## AG-35 Cross-platform build capability

Keep Linux and Windows builds with unified dependencies, parameters, paths, and outputs. Avoid hardcoded environments and unapproved Windows-only build limits. Test the available environment, record unrun environments, and validate Windows runtime separately from Linux compilation. Fix and retest confirmed cross-platform faults and block merges. Synchronize toolchain documents. AG-49 defines per-PR responsibility.

<a id="ag-39"></a>

## AG-39 Project dependencies and supply chain

Centralize versions, lockfiles, declarations, and paths for Rust, npm, Python, SDKs, WDK, tools, and open source. Members select required dependencies. Unify isolated third-party runtime dependencies. Review origin, license, vulnerabilities, and maintenance before approving additions, upgrades, or downgrades. Record version, direct purpose, and selection basis. Clean removed dependencies, build in the available environment, and run integrated regression. Minimize unnecessary versions, scattered environments and caches, and oversized dependencies.

<a id="ag-40"></a>

## AG-40 Unified configuration

Core manages registration, reads, updates, validation, and migrations. Plugins declare schemas. Centralize development, build, runtime, and user configuration. Define defaults, dynamic updates, permission isolation, and secure storage for secrets. Preserve user config across upgrades and check project-wide impact. Modules use the registry for global settings.

<a id="ag-54"></a>

## AG-54 Dependency completeness

Reuse valid environments and caches. Before development check approved dependencies, versions, and resolved paths and fill genuine gaps. Use declared tools and libraries and approved installation paths. Preserve reproducible instructions and run clean builds where needed to detect contamination.

<a id="ag-55"></a>

## AG-55 Tool capability and dependency paths

Verify tools, permissions, network, environment, and available agents before work. Report limits and actual commands, access, builds, tests, and delegation. Keep project decisions independent of incidental tool limits. Resolve dependencies through centralized project declarations, avoid personal or other-project paths and scattered environments, and fix bad resolution before reinstalling. Validate moved directories and environment changes. Configure external SDKs and WDK centrally without copying them into the repo. Record outcomes in preflight.

<a id="ag-56"></a>

## AG-56 Workspace and environment isolation

Apply the same rules in local and cloud environments and worktrees. Keep roots, build outputs, and runtime state separate and label evidence by its actual environment. Keep machine paths and settings out of the repository. Recheck paths and dependencies when moving, and resume from Git and records. The current developer owns verification without a universal requirement to use multiple environments.
