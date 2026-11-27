# Architecture standard

[简体中文](../zh-CN/standards/ARCHITECTURE.md) · [Index](../INDEX.md)

<a id="ag-30"></a>

## AG-30 Frontend design

Use [the frontend standard](../architecture/FRONTEND.md#section-9). Reuse Amicro, Glasswork, shadcn-admin, and UI Moments, and ship White Acrylic, Black Acrylic, and Liquid Glass with Glasswork buttons and frosted panels. Reuse upstream primitives and preserve the design system. UI Moments supplies existing interactive components and motion; its component examples need license and version review, and Fluent UI and Astryx are not approved product dependencies. Business adaptations and minor shadcn-admin layout changes are allowed. Document gaps and get approval before using alternatives. Verify plugin UI, resizing, real data and errors, theme switching, persistence, interaction, and performance. Root AGENTS links this rule and the frontend standard.

<a id="ag-32"></a>

## AG-32 Compatibility and migration

Version public interfaces, events, protocols, databases, and configuration. Check direct and indirect consumers, official plugins, SDK, and MCP. Get breaking changes approved and preserve user settings and data. Verify upgrades, rollback, and migration failures. Remove approved obsolete code, tests, dependencies, and docs together and run overall regression. Avoid duplicate legacy implementations without need.

<a id="ag-42"></a>

## AG-42 Core and plugin responsibilities

Core owns common infrastructure and plugins own business functionality. Use standardized contracts and on-demand shared collection. Avoid per-plugin logic in Core and duplicate collection paths. Verify stable Core behavior with all business plugins disabled. Propose the necessity, generality, and impact of new core capabilities for approval and audit.

<a id="ag-43"></a>

## AG-43 Independent and replaceable plugins

Support independent loading, start, pause, stop, update, and recovery. Disable releases dedicated resources and subscriptions. Persist state across restart and preserve history. Use capabilities, services, and events without hardcoded identities, paths, or access to private implementation. Built-ins can be disabled or replaced through compatible contracts without editing Core or unrelated plugins, while remaining installed. Verify hot replacements and keep special rescue authorizations isolated.

<a id="ag-44"></a>

## AG-44 Unified storage and shared data

Core manages storage, querying, indexing, encryption, retention, and specialized formats. Share object identities, persist source records once where practicable, and centralize permissions for plugin structures. Cache derivable data only for a demonstrated performance need. Verify migration and backup consistency and measure storage efficiency and schema change impact.

<a id="ag-45"></a>

## AG-45 Concurrency and async

Centralize scheduling and runtime and grant resources to plugins. Support priority, cancellation, timeouts, limits, and safe release on disable. Prevent thread leaks, deadlocks, unbounded pools and queues, and unlimited waits. Keep the GUI responsive. Verify specialized high-throughput mechanisms with real concurrency, stress, and longevity tests.

<a id="ag-46"></a>

## AG-46 Event transport

Unify event format, type, time, source, and transport. Support subscriptions, filters, batches, priorities, bounded backpressure, low copying and serialization costs, and traceable loss. Unsubscribe on disable and enforce permissions on every channel. Check all consumers and producers of changed protocols. Verify throughput, bursts, and integrity with real data.
