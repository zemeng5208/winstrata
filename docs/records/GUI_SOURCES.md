# GUI source register

[简体中文](../zh-CN/records/GUI_SOURCES.md) · [AG-64](../../AGENTS.md#ag-64) · [Frontend standard](../architecture/FRONTEND.md#section-9) · [Index](../INDEX.md)

This register is required by [AG-64](../../AGENTS.md#ag-64) for the official GUI and for official plugins. [STATUS.md](STATUS.md) is not a substitute. A row is a claim that the named product file is in the same commit as the row. Third-party GUI plugins may use their own design system; they do not receive a row that pretends they came from the four approved sources. Their security, compatibility, and performance evidence has no implementation plan ID.

## Required fields

| Field | Meaning |
|---|---|
| ID | Stable component or layout ID |
| Product path | Path in this commit. Empty when the file is not in this commit |
| Upstream repository | Owner and repository name |
| Version | Upstream version and the full git revision |
| Source path | Path inside that upstream revision |
| License | The license that applies to the reused file, including notices that limit it |
| Reuse method | How the product file relates to the upstream file |
| Modifications | What changed. `none` only when the bytes match |
| Introducing plan | The plan that adds the product file |
| State | `in-product`, `gap`, or `not-in-this-commit` |

Rewriting a component after looking at an upstream visual style is not a reuse row. A missing upstream component is a gap row with the candidates and the evidence, and it stays a gap until the owner approves an implementation or another source.

## Product rows in this commit

Plan 016 adds these files. The base is main `58b7d4fb13f5084222bb985c5c4a81a8b82692de`, the merge of plan 015 and GitHub PR #22. `packages/ui/upstream.json` is the same register in the tree. SHA-256 below is the product file. For a row whose modifications are `none`, that digest is also the upstream blob. The four adapted files record a separate upstream digest in `upstream.json`.

Glasswork `NOTICE` (`824058026d7338161460df1fda278d593e7a1682d9601c3780167c7fd6144a29`) says the repository code is MIT and that the macOS wallpaper media is not. This commit vendors `lens.js` and `glass.css` only. It does not vendor that media. Fluent UI and Astryx are not product dependencies.

`packages/ui/dist` is the build output of these sources. `dist/glass.css` is a byte copy of the Glasswork file. `dist/layout.css` is the Tailwind build of the vendored `index.css` and `theme.css`. `dist/shell.js` is the esbuild bundle. `dist/stamp.json` records the SHA-256 of the vendor and composition sources. Those outputs are not separate upstream components.

Closed PR #21's `packages/ui/static/shell.js` and `packages/ui/static/themes.css` are not in this commit. They are not official surfaces.

| ID | Product path | Upstream repository | Version | Source path | License | Reuse method | Modifications | Introducing plan | State | SHA-256 |
|---|---|---|---|---|---|---|---|---|---|---|
| amicro-LICENSE | `packages/ui/vendor/amicro/LICENSE` | Subhan-code/Amicro--Micro-transitions- | 1.0.1, `fa5f8ffbe39537a8db191806066b8773c47c82e9` | `LICENSE` | MIT | license text, verbatim | none | 016 | in-product | `4b2e0abfc3fdc8722545c3772ee7c6fd0bcdbe8af76112af50066689fca4a9c2` |
| amicro-fade-in-tsx | `packages/ui/vendor/amicro/fade-in.tsx` | Subhan-code/Amicro--Micro-transitions- | 1.0.1, `fa5f8ffbe39537a8db191806066b8773c47c82e9` | `registry/ui/entrance/fade-in.tsx` | MIT | compiled into the official shell | none | 016 | in-product | `742d0118dfb2237da82e62fc8740f1144e270bcb921ed561ec896f97b4d89612` |
| glasswork-LICENSE | `packages/ui/vendor/glasswork/LICENSE` | kubai087/glasswork | 1.1.0, `4f0b6744b898b3a9b98fbd913ab24fdb83c53162` | `LICENSE` | MIT | license text, verbatim | none | 016 | in-product | `b38c1defe9e46be8baa663da135c74b3edb0d2ae8c930e9b41d9405f6013e512` |
| glasswork-NOTICE | `packages/ui/vendor/glasswork/NOTICE` | kubai087/glasswork | 1.1.0, `4f0b6744b898b3a9b98fbd913ab24fdb83c53162` | `NOTICE` | MIT | license text, verbatim | none | 016 | in-product | `824058026d7338161460df1fda278d593e7a1682d9601c3780167c7fd6144a29` |
| glasswork-glass-css | `packages/ui/vendor/glasswork/glass.css` | kubai087/glasswork | 1.1.0, `4f0b6744b898b3a9b98fbd913ab24fdb83c53162` | `glass.css` | MIT | compiled into the official shell | none | 016 | in-product | `d13548bf70660fbb25c5361d7dc60c1fc47fc0803e4b4937d062753b664d2290` |
| glasswork-lens-js | `packages/ui/vendor/glasswork/lens.js` | kubai087/glasswork | 1.1.0, `4f0b6744b898b3a9b98fbd913ab24fdb83c53162` | `lens.js` | MIT | compiled into the official shell | none | 016 | in-product | `7dc1c4b5c37fc3aec7ee4903f643f1be774d680dcae79a95e38461cb00bb9c31` |
| shadcn-admin-LICENSE | `packages/ui/vendor/shadcn-admin/LICENSE` | satnaing/shadcn-admin | 2.2.1, `a156d9b90519348c4d71f0f0e6fcea179c49a43f` | `LICENSE` | MIT | license text, verbatim | none | 016 | in-product | `d28c723c33a18dfed6af67b1a0ac368724f79bcb80750d3e080dbcf8dedb9e4b` |
| shadcn-admin-src-components-layout-app-sidebar-tsx | `packages/ui/vendor/shadcn-admin/src/components/layout/app-sidebar.tsx` | satnaing/shadcn-admin | 2.2.1, `a156d9b90519348c4d71f0f0e6fcea179c49a43f` | `src/components/layout/app-sidebar.tsx` | MIT | vendored with the layout source, not mounted | none | 016 | in-product | `4683366bd5448d62945121dbe35e29cafb42dd854e2a4c4d85063b74b9e69b5f` |
| shadcn-admin-src-components-layout-app-title-tsx | `packages/ui/vendor/shadcn-admin/src/components/layout/app-title.tsx` | satnaing/shadcn-admin | 2.2.1, `a156d9b90519348c4d71f0f0e6fcea179c49a43f` | `src/components/layout/app-title.tsx` | MIT | compiled into the official shell | Accepts title and subtitle. Defaults remain the upstream strings Shadcn-Admin and Vite + ShadcnUI. | 016 | in-product | `eede5bd4166090f05265338cbaf5b12cfb4a14d5506e3a5ef0f381c6ac5542a3` |
| shadcn-admin-src-components-layout-authenticated-layout-tsx | `packages/ui/vendor/shadcn-admin/src/components/layout/authenticated-layout.tsx` | satnaing/shadcn-admin | 2.2.1, `a156d9b90519348c4d71f0f0e6fcea179c49a43f` | `src/components/layout/authenticated-layout.tsx` | MIT | compiled into the official shell | Requires a sidebar node and an optional skip label. Renders that sidebar instead of AppSidebar. SidebarInset class names are unchanged. | 016 | in-product | `cb2b9038f55f063a66778098c602bf4ab5901eeae6d579755bab9f0e3ea735e1` |
| shadcn-admin-src-components-layout-data-sidebar-data-ts | `packages/ui/vendor/shadcn-admin/src/components/layout/data/sidebar-data.ts` | satnaing/shadcn-admin | 2.2.1, `a156d9b90519348c4d71f0f0e6fcea179c49a43f` | `src/components/layout/data/sidebar-data.ts` | MIT | vendored with the layout source, not mounted | none | 016 | in-product | `5b1fcb85008f6c3ddffe42ec7e08181514a492063bd33a8977ce223c664c5770` |
| shadcn-admin-src-components-layout-header-tsx | `packages/ui/vendor/shadcn-admin/src/components/layout/header.tsx` | satnaing/shadcn-admin | 2.2.1, `a156d9b90519348c4d71f0f0e6fcea179c49a43f` | `src/components/layout/header.tsx` | MIT | compiled into the official shell | none | 016 | in-product | `f19060d14bdcb6efa98f1eee05452ecdc22d39a8a358af0d6e319e7ec9563777` |
| shadcn-admin-src-components-layout-main-tsx | `packages/ui/vendor/shadcn-admin/src/components/layout/main.tsx` | satnaing/shadcn-admin | 2.2.1, `a156d9b90519348c4d71f0f0e6fcea179c49a43f` | `src/components/layout/main.tsx` | MIT | compiled into the official shell | none | 016 | in-product | `e200dbc0e1b8cf383fdf6b7f7588ac1c9419de65f2658692e7941324bd09c572` |
| shadcn-admin-src-components-layout-nav-group-tsx | `packages/ui/vendor/shadcn-admin/src/components/layout/nav-group.tsx` | satnaing/shadcn-admin | 2.2.1, `a156d9b90519348c4d71f0f0e6fcea179c49a43f` | `src/components/layout/nav-group.tsx` | MIT | compiled into the official shell | none | 016 | in-product | `dd90536e487751b28fd8c850a0728a67784d5185c589b62fca580c992b1d2ed6` |
| shadcn-admin-src-components-layout-nav-user-tsx | `packages/ui/vendor/shadcn-admin/src/components/layout/nav-user.tsx` | satnaing/shadcn-admin | 2.2.1, `a156d9b90519348c4d71f0f0e6fcea179c49a43f` | `src/components/layout/nav-user.tsx` | MIT | vendored with the layout source, not mounted | none | 016 | in-product | `739f42c8f0b2afde6a0f202f751399dea4b0a72c36113488bffb707f09207b34` |
| shadcn-admin-src-components-layout-sidebar-sponsors-tsx | `packages/ui/vendor/shadcn-admin/src/components/layout/sidebar-sponsors.tsx` | satnaing/shadcn-admin | 2.2.1, `a156d9b90519348c4d71f0f0e6fcea179c49a43f` | `src/components/layout/sidebar-sponsors.tsx` | MIT | vendored with the layout source, not mounted | none | 016 | in-product | `aef77970d2a1e92b397477d74be45f7a4112448cdc3b762155be3c20c76d9d2a` |
| shadcn-admin-src-components-layout-team-switcher-tsx | `packages/ui/vendor/shadcn-admin/src/components/layout/team-switcher.tsx` | satnaing/shadcn-admin | 2.2.1, `a156d9b90519348c4d71f0f0e6fcea179c49a43f` | `src/components/layout/team-switcher.tsx` | MIT | vendored with the layout source, not mounted | none | 016 | in-product | `5ea0594bf435f2e8cde44994e68e1c753ed12a9c864bc5db4292ce0d42749299` |
| shadcn-admin-src-components-layout-top-nav-tsx | `packages/ui/vendor/shadcn-admin/src/components/layout/top-nav.tsx` | satnaing/shadcn-admin | 2.2.1, `a156d9b90519348c4d71f0f0e6fcea179c49a43f` | `src/components/layout/top-nav.tsx` | MIT | vendored with the layout source, not mounted | none | 016 | in-product | `9064632a68016ead787798e90eb7fc4b3cd2fc32274e22df943f92b55c722065` |
| shadcn-admin-src-components-layout-types-ts | `packages/ui/vendor/shadcn-admin/src/components/layout/types.ts` | satnaing/shadcn-admin | 2.2.1, `a156d9b90519348c4d71f0f0e6fcea179c49a43f` | `src/components/layout/types.ts` | MIT | compiled into the official shell | none | 016 | in-product | `07cbb707e5e950c2751d638c3e1fc6ed85df5306ba90789fe81d571533f681c8` |
| shadcn-admin-src-components-skip-to-main-tsx | `packages/ui/vendor/shadcn-admin/src/components/skip-to-main.tsx` | satnaing/shadcn-admin | 2.2.1, `a156d9b90519348c4d71f0f0e6fcea179c49a43f` | `src/components/skip-to-main.tsx` | MIT | compiled into the official shell | Accepts label. The default remains Skip to Main. | 016 | in-product | `a273f90f72dbadd9e771718c0e70fd4914c868dfaf95544a7320e25cfdbeb7f4` |
| shadcn-admin-src-components-ui-badge-tsx | `packages/ui/vendor/shadcn-admin/src/components/ui/badge.tsx` | satnaing/shadcn-admin | 2.2.1, `a156d9b90519348c4d71f0f0e6fcea179c49a43f` | `src/components/ui/badge.tsx` | MIT | compiled into the official shell | none | 016 | in-product | `a642d7a932b91ec610f09078452d34c379900845066955311506b6d503ea11e6` |
| shadcn-admin-src-components-ui-button-tsx | `packages/ui/vendor/shadcn-admin/src/components/ui/button.tsx` | satnaing/shadcn-admin | 2.2.1, `a156d9b90519348c4d71f0f0e6fcea179c49a43f` | `src/components/ui/button.tsx` | MIT | compiled into the official shell | none | 016 | in-product | `97ed8ef5651a6e264ded042b024b5be39673ba2ff222017fbcf845cf3ed37226` |
| shadcn-admin-src-components-ui-collapsible-tsx | `packages/ui/vendor/shadcn-admin/src/components/ui/collapsible.tsx` | satnaing/shadcn-admin | 2.2.1, `a156d9b90519348c4d71f0f0e6fcea179c49a43f` | `src/components/ui/collapsible.tsx` | MIT | compiled into the official shell | none | 016 | in-product | `9fd25f52e0f6fd1bbfff0f1f3853fecd6a1c7ee1a9118a73f2e59a2ffa411e0a` |
| shadcn-admin-src-components-ui-dropdown-menu-tsx | `packages/ui/vendor/shadcn-admin/src/components/ui/dropdown-menu.tsx` | satnaing/shadcn-admin | 2.2.1, `a156d9b90519348c4d71f0f0e6fcea179c49a43f` | `src/components/ui/dropdown-menu.tsx` | MIT | compiled into the official shell | none | 016 | in-product | `41ec3204cbc0917e3cf082a80a862b5ae1ea9e92c77dbbaaaa3906f28f74110b` |
| shadcn-admin-src-components-ui-input-tsx | `packages/ui/vendor/shadcn-admin/src/components/ui/input.tsx` | satnaing/shadcn-admin | 2.2.1, `a156d9b90519348c4d71f0f0e6fcea179c49a43f` | `src/components/ui/input.tsx` | MIT | compiled into the official shell | none | 016 | in-product | `4f7d443722b4aeacb1179b863b98f773f7950514ceb1d897b66c967521f08072` |
| shadcn-admin-src-components-ui-separator-tsx | `packages/ui/vendor/shadcn-admin/src/components/ui/separator.tsx` | satnaing/shadcn-admin | 2.2.1, `a156d9b90519348c4d71f0f0e6fcea179c49a43f` | `src/components/ui/separator.tsx` | MIT | compiled into the official shell | none | 016 | in-product | `aa5cc28cdebe9264228c622e48363b9e8a2684436e47c255bb1a0a905610aaef` |
| shadcn-admin-src-components-ui-sheet-tsx | `packages/ui/vendor/shadcn-admin/src/components/ui/sheet.tsx` | satnaing/shadcn-admin | 2.2.1, `a156d9b90519348c4d71f0f0e6fcea179c49a43f` | `src/components/ui/sheet.tsx` | MIT | compiled into the official shell | none | 016 | in-product | `2423f0e67f594198df542acabd32547896925e35786c6bb085f8a4eeed4273c4` |
| shadcn-admin-src-components-ui-sidebar-tsx | `packages/ui/vendor/shadcn-admin/src/components/ui/sidebar.tsx` | satnaing/shadcn-admin | 2.2.1, `a156d9b90519348c4d71f0f0e6fcea179c49a43f` | `src/components/ui/sidebar.tsx` | MIT | compiled into the official shell | none | 016 | in-product | `40f021f27b3043b2af4c247b99f665b323edd142a4317a7b84ac4e3017070e9c` |
| shadcn-admin-src-components-ui-skeleton-tsx | `packages/ui/vendor/shadcn-admin/src/components/ui/skeleton.tsx` | satnaing/shadcn-admin | 2.2.1, `a156d9b90519348c4d71f0f0e6fcea179c49a43f` | `src/components/ui/skeleton.tsx` | MIT | compiled into the official shell | none | 016 | in-product | `7617060c9ea8a25dafadf224a8567c91daebd4a43149d0e72143bba807fab457` |
| shadcn-admin-src-components-ui-tooltip-tsx | `packages/ui/vendor/shadcn-admin/src/components/ui/tooltip.tsx` | satnaing/shadcn-admin | 2.2.1, `a156d9b90519348c4d71f0f0e6fcea179c49a43f` | `src/components/ui/tooltip.tsx` | MIT | compiled into the official shell | none | 016 | in-product | `adb364c0798a914d6a37f2dc0f22a75b0537be568149ee9b90794b2352c03315` |
| shadcn-admin-src-context-layout-provider-tsx | `packages/ui/vendor/shadcn-admin/src/context/layout-provider.tsx` | satnaing/shadcn-admin | 2.2.1, `a156d9b90519348c4d71f0f0e6fcea179c49a43f` | `src/context/layout-provider.tsx` | MIT | compiled into the official shell | none | 016 | in-product | `11fe4cabe4684e42d0a99e7b2277ec6fc01919c16036c2e7a6985841c318f671` |
| shadcn-admin-src-context-search-provider-tsx | `packages/ui/vendor/shadcn-admin/src/context/search-provider.tsx` | satnaing/shadcn-admin | 2.2.1, `a156d9b90519348c4d71f0f0e6fcea179c49a43f` | `src/context/search-provider.tsx` | MIT | compiled into the official shell | Removes the CommandMenu import and element. The search context and Ctrl/Cmd-K listener remain. The demo command menu is not mounted. | 016 | in-product | `266cedbfea4f5080d5a97099841ba834aa7363664bf900c040a74c390f218d31` |
| shadcn-admin-src-hooks-use-mobile-tsx | `packages/ui/vendor/shadcn-admin/src/hooks/use-mobile.tsx` | satnaing/shadcn-admin | 2.2.1, `a156d9b90519348c4d71f0f0e6fcea179c49a43f` | `src/hooks/use-mobile.tsx` | MIT | compiled into the official shell | none | 016 | in-product | `cef2003db553ef127ba219e8f4303a733cc76b85f83f17b1622131c0ecfd3b3c` |
| shadcn-admin-src-lib-cookies-ts | `packages/ui/vendor/shadcn-admin/src/lib/cookies.ts` | satnaing/shadcn-admin | 2.2.1, `a156d9b90519348c4d71f0f0e6fcea179c49a43f` | `src/lib/cookies.ts` | MIT | compiled into the official shell | none | 016 | in-product | `18c757bec8c35d721c4c619934a3703c5c327c2d05a88e3ab3ee6699ed69ae40` |
| shadcn-admin-src-lib-utils-ts | `packages/ui/vendor/shadcn-admin/src/lib/utils.ts` | satnaing/shadcn-admin | 2.2.1, `a156d9b90519348c4d71f0f0e6fcea179c49a43f` | `src/lib/utils.ts` | MIT | compiled into the official shell | none | 016 | in-product | `cc244af40baa5fe88abff3f80b424a7aa654b3137a7db612a8fe94861b61e04e` |
| shadcn-admin-src-styles-index-css | `packages/ui/vendor/shadcn-admin/src/styles/index.css` | satnaing/shadcn-admin | 2.2.1, `a156d9b90519348c4d71f0f0e6fcea179c49a43f` | `src/styles/index.css` | MIT | compiled into the official shell | none | 016 | in-product | `ee0477a25d5836191f9efcdd7260aa2b75e358865f4fecdeb59ac4e3bbefce46` |
| shadcn-admin-src-styles-theme-css | `packages/ui/vendor/shadcn-admin/src/styles/theme.css` | satnaing/shadcn-admin | 2.2.1, `a156d9b90519348c4d71f0f0e6fcea179c49a43f` | `src/styles/theme.css` | MIT | compiled into the official shell | none | 016 | in-product | `faee559c9a21b2084bf7bae193e70f0ab18b18a393a06fec1408669652c266a1` |

## Composition

These files bind the official shell to host state. They are not upstream components and they are not a new visual system.

| ID | Product path | Upstream repository | Version | Source path | License | Reuse method | Modifications | Introducing plan | State |
|---|---|---|---|---|---|---|---|---|---|
| winstrata-copy | `packages/ui/compose/copy.ts` | none | none | none | Apache-2.0 | WinStrata composition | English and zh-CN strings. English is the fallback. | 016 | in-product |
| winstrata-entry | `packages/ui/compose/entry.tsx` | none | none | none | Apache-2.0 | WinStrata composition | Memory-router routes for the official pages. | 016 | in-product |
| winstrata-shell | `packages/ui/compose/shell.tsx` | none | none | none | Apache-2.0 | WinStrata composition | Mounts the shadcn shell, Glasswork material, and Amicro fade. Binds real host state. | 016 | in-product |

The three themes share this shell. White rounded acrylic uses the shadcn light tokens and Glasswork `data-kind=light`. Black rounded acrylic uses the shadcn `html.dark` tokens and Glasswork `data-kind=dark`. Liquid Glass uses the light glass panels and `liquidGlass()` on buttons that carry `data-lens`. No theme is stored until the user chooses one. [OI-023](OPEN_ITEMS.md#oi-023) stays open.

## Gap

| ID | Product path | Upstream repository | Version | Source path | License | Reuse method | Modifications | Introducing plan | State |
|---|---|---|---|---|---|---|---|---|---|
| ui-moments | none | guanmo-ai/ui-moments | revision `c9eece0fe425ee7dfe97d162d39d1665c3bf9368` | none | not reviewed | not included | none | 016 | not-in-this-commit |

UI Moments stays [OI-009](OPEN_ITEMS.md#oi-009). This plan does not invent a component to fill that gap and does not add another source.
