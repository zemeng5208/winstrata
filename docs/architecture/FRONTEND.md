# Frontend design standard

[简体中文](../zh-CN/architecture/FRONTEND.md) · [Index](../INDEX.md)

<a id="section-9"></a>

## Required upstream sources

| Project | Approved use |
|---|---|
| [Amicro — Micro transitions](https://github.com/Subhan-code/Amicro--Micro-transitions-) | Reuse existing components and micro-interactions. |
| [Glasswork](https://github.com/kubai087/glasswork) | Reuse its Liquid Glass-style buttons, glass materials, and frosted effects. |
| [shadcn-admin](https://github.com/satnaing/shadcn-admin) | Use its app shell and layout conventions for navigation, header, content, dashboard, tables, and settings. Minor layout adaptations for WinStrata are permitted. |

The sources are approved as design inputs. Verify the selected revision's license, third-party materials and transitive licenses before inclusion. Repository metadata indicates MIT. Preserve copyright and notices. Verify actual component coverage, implementation, reuse mechanisms, WebView2 compatibility, dependencies and performance. Submit versions and integration choices for approval under [AG-08](../standards/DEPENDENCIES.md#ag-08) and [AG-39](../standards/DEPENDENCIES.md#ag-39).

## Built-in themes

| Theme | Requirement |
|---|---|
| White Acrylic | Light backgrounds, rounded acrylic surfaces, and consistent controls. |
| Black Acrylic | Dark backgrounds and rounded acrylic surfaces with readable text, boundaries, and states. |
| Liquid Glass | Glasswork-style buttons with frosted panels. |

Apply all themes to the main window, dashboard, official plugin pages, settings, market, onboarding, and dialogs. Share components and theme resources, persist the selection, and switch at runtime. Approve initial theme and material parameter choices separately. Store one home layout independently of theme.

## Implementation and acceptance

Reuse existing upstream primitives for buttons, forms, dialogs, navigation, transitions, and other baseline components. Combine components, bind backend state, add business adapters and necessary theme parameters, and adapt shadcn-admin spacing, density, or layout within the source design. If a required component is missing or fails functionality, security, compatibility or performance, show evidence and seek approval for an alternative. Host official and third-party pages through shared themes and registration. Centralize sources and theme resources. Avoid duplicate component libraries across plugins.

Validate glass blur, refraction and transitions on real Windows WebView2. Bound CPU, GPU, memory and I/O and suspend offscreen animation. Verify theme switching, persistence and restore, behavior across themes, live plugin pages, resizing, readable translations, and performance. Record upstream sources and changes. Approval is required before dropping a theme capability.

## User flows

Separate backend behavior and state from presentation. Use actual backends and honest status states. Provide complete Simplified Chinese and English, English fallback, plugin language resources, and runtime switching.

First launch chooses language, valid location, enabled plugins, collection scope, retention (indefinite default), start at boot (off), optional encryption, MCP, and the initial baseline. Allow skipping nonessential steps and resuming. Exclude rescue from batch enablement. Keep preset scripts and automatic snapshots off. Baseline only the selected scope and start only selected sources.

The single customizable dashboard supports adding, removing, dragging, resizing, arranging, settings and refresh frequency for dynamic plugin widgets. Auto-save and reset its layout. Keep background collection independent of offscreen rendering. The same market supplies plugins and scripts.

Search software, paths, registry, processes, PIDs, IP/domain/port, and combine regex, time and type filters. Show shared object profiles, relationships, timeline, source navigation and confidence. Use dedicated plugin-management views.

On window close, offer tray continuation or full exit with flushing and safe resource release. Store an optional preference and allow changing it. Monitor only while authorized and active. Support runtime switching to historical analysis, stopping capture and safely unloading unnecessary drivers, flushing pending data and recording gaps. Carry authorized historical data, snapshots, captures, app profiles and queries including MCP. Provide global and per-plugin pause and resume and live configuration of disk, path, file type, process, software, registry and interface scope. Distinguish pause, disable, failure, and loss. Report loading, empty, permission, unsupported, offline, and storage states accurately.
