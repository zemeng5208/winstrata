# Frontend design standard

[简体中文](../zh-CN/architecture/FRONTEND.md) · [Index](../INDEX.md)

<a id="section-9"></a>

## Required upstream sources

Reuse the owner-specified upstream projects. Do not invent or rewrite existing UI components, and do not introduce an unapproved design system. The table states approved uses. It does not record completed integration. The built-in themes remain the three listed below.

| Project | Approved use |
|---|---|
| [Amicro — Micro transitions](https://github.com/Subhan-code/Amicro--Micro-transitions-) | Reuse existing components and micro-interactions. |
| [Glasswork](https://github.com/kubai087/glasswork) | Reuse its Liquid Glass-style buttons, glass materials, and frosted effects. |
| [shadcn-admin](https://github.com/satnaing/shadcn-admin) | Use its app shell and layout conventions for navigation, header, content, dashboard, tables, and settings. Minor layout adaptations for WinStrata are permitted. |
| [UI Moments](https://github.com/guanmo-ai/ui-moments) | Additional source for existing interactive components and motion: original controls and loaders, and applicable component examples only after license and version review. Site: [guanmo-ai.github.io/ui-moments](https://guanmo-ai.github.io/ui-moments/). Designation does not approve Fluent UI or Astryx as product dependencies. |

Amicro, Glasswork, and shadcn-admin publish MIT license metadata. UI Moments is a hands-on gallery of interactive UI and motion maintained by 观默 (Guanmo). Its project code is MIT. Some component examples are built on third-party libraries, so the gallery is not MIT-only. Before inclusion, verify the selected revision's license, third-party materials, and transitive licenses, and preserve copyright and notices.

The categories, counts, and credits below come from [README.md](https://github.com/guanmo-ai/ui-moments/blob/c9eece0fe425ee7dfe97d162d39d1665c3bf9368/README.md), [README.zh-CN.md](https://github.com/guanmo-ai/ui-moments/blob/c9eece0fe425ee7dfe97d162d39d1665c3bf9368/README.zh-CN.md), and [CREDITS.md](https://github.com/guanmo-ai/ui-moments/blob/c9eece0fe425ee7dfe97d162d39d1665c3bf9368/CREDITS.md) on default-branch commit `c9eece0fe425ee7dfe97d162d39d1665c3bf9368`, read on 2026-11-27. The first release has 884 live examples. Names in this document are the English README's names. The Simplified Chinese document uses the Chinese README's names where those labels differ, including English “Code Shake” and Chinese “验证码输入”.

| Category | Examples named upstream |
|---|---|
| Core controls | Ink Switch, Detent Dial, Hold to Confirm, Odometer Stepper, Leaf Tabs, Tide Progress |
| Visual interactions | Magnet Grid, Jelly Cube, Merge Drops, Ripple Pad, Bloom Toggle |
| Gestures and physics | Swipe to Confirm, Pull to Refresh, Elastic String, Domino Row, Rocker Switch, Scrub Wave, Snap Grid, Radial Menu, Step Track, Breath Orb |
| Action feedback | Copy Button, Like Burst, Paper Plane Send, Download Ring, Code Shake, Bell Toggle |
| Form inputs | Stroke Checklist, Pebble Rating, Notch Slider, Strength Meter, Chip Input, Plan Picker |
| Color controls | Hue Orbit, Pull Cord Lamp, Flip Coin |
| Original loaders | Paper Fold, Signal Return, and 16 other loaders. Parameters can be adjusted. Export HTML or JSX. |
| Component examples | 830 live examples covering drawers, forms, menus, data grids, charts, schedules, code editors, and more, built on third-party libraries |

| Work | Attribution and version | License |
|---|---|---|
| Original controls, loaders, site, and integration examples | 观默 (Guanmo) | MIT |
| Fluent UI React | Microsoft, pinned to `@fluentui/react-components@9.75.2`. The examples use system fonts. | Package and upstream licenses under `gallery/fluent-ui/vendor/` |
| Meta Astryx | Meta Platforms, Inc. Core, the Neutral theme, and the CLI are pinned to 0.7.17. Charts, Lab, and Vega retain 0.7.17-canary.g72f2d7c. | MIT |
| Geist / Geist Mono | Vercel, bundled Latin variable fonts | OFL 1.1 |

The Fluent UI and Astryx names are attribution only. UI Moments is unaffiliated with Microsoft and Meta. Inspect the real source, applicable components, reuse method, WebView2 compatibility, performance, and dependencies, then submit specific versions and an integration plan. Do not treat upstream demo pages or unverified APIs as ready-to-use components. Concrete versions, extra dependencies, and out-of-scope changes still require approval under [AG-08](../standards/DEPENDENCIES.md#ag-08) and [AG-39](../standards/DEPENDENCIES.md#ag-39). The plan 004 trials cover Amicro, Glasswork, and shadcn-admin only. UI Moments, Fluent UI, and Astryx have not received that verification. See the [platform report](PLATFORM_FEASIBILITY.md).

## Built-in themes

| Theme | Requirement |
|---|---|
| White Acrylic | Light backgrounds, rounded acrylic surfaces, and consistent controls. |
| Black Acrylic | Dark backgrounds and rounded acrylic surfaces with readable text, boundaries, and states. |
| Liquid Glass | Glasswork-style buttons with frosted panels. |

Apply all themes to the main window, dashboard, official plugin pages, settings, market, onboarding, and dialogs. Share components and theme resources, persist the selection, and switch at runtime. Approve initial theme and material parameter choices separately. Store one home layout independently of theme.

## Implementation and acceptance

Reuse existing upstream primitives for buttons, forms, dialogs, navigation, transitions, and other baseline components. Combine components, bind backend state, add business adapters and necessary theme parameters, and adapt shadcn-admin spacing, density, or layout within the source design. If a required component is missing or fails functionality, security, compatibility or performance, show evidence and seek approval for an alternative. Host official and third-party pages through shared themes and registration. Centralize sources and theme resources. Avoid duplicate component libraries across plugins.

UI Moments adds a source for existing interactive components and motion. Original controls and loaders still need revision, license, WebView2, performance, and dependency review. Component examples also need license and version review before use. This designation does not approve Fluent UI or Astryx as product dependencies.

Validate glass blur, refraction and transitions on real Windows WebView2. Bound CPU, GPU, memory and I/O and suspend offscreen animation. Verify theme switching, persistence and restore, behavior across themes, live plugin pages, resizing, readable translations, and performance. Record upstream sources and changes. Approval is required before dropping a theme capability.

## User flows

Separate backend behavior and state from presentation. Use actual backends and honest status states. Provide complete Simplified Chinese and English, English fallback, plugin language resources, and runtime switching.

First launch chooses language, valid location, enabled plugins, collection scope, retention (indefinite default), start at boot (off), optional encryption, MCP, and the initial baseline. Allow skipping nonessential steps and resuming. Exclude rescue from batch enablement. Keep preset scripts and automatic snapshots off. Baseline only the selected scope and start only selected sources.

The single customizable dashboard supports adding, removing, dragging, resizing, arranging, settings and refresh frequency for dynamic plugin widgets. Auto-save and reset its layout. Keep background collection independent of offscreen rendering. The same market supplies plugins and scripts.

Search software, paths, registry, processes, PIDs, IP/domain/port, and combine regex, time and type filters. Show shared object profiles, relationships, timeline, source navigation and confidence. Use dedicated plugin-management views.

On window close, offer tray continuation or full exit with flushing and safe resource release. Store an optional preference and allow changing it. Monitor only while authorized and active. Support runtime switching to historical analysis, stopping capture and safely unloading unnecessary drivers, flushing pending data and recording gaps. Carry authorized historical data, snapshots, captures, app profiles and queries including MCP. Provide global and per-plugin pause and resume and live configuration of disk, path, file type, process, software, registry and interface scope. Distinguish pause, disable, failure, and loss. Report loading, empty, permission, unsupported, offline, and storage states accurately.
