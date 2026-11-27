# GUI 来源登记

[English](../../records/GUI_SOURCES.md) · [AG-64](../AGENTS.md#ag-64) · [前端规范](../architecture/FRONTEND.md#section-9) · [索引](../INDEX.md)

本登记由 [AG-64](../AGENTS.md#ag-64) 要求，适用于官方 GUI 和官方插件。[STATUS.md](STATUS.md) 不能代替本登记。一行记录表示该产品文件与这一行处于同一提交。第三方 GUI 插件可以使用自己的设计体系，不为它们补写“来自四个指定来源”的行。它们的安全、兼容性和性能证据没有实现计划编号。

## 必要字段

| 字段 | 含义 |
|---|---|
| ID | 稳定的组件或布局编号 |
| 产品路径 | 本提交中的路径。文件不在本提交时留空 |
| 上游仓库 | 所有者和仓库名 |
| 版本 | 上游版本和完整 git 修订 |
| 源码路径 | 该上游修订内的路径 |
| 许可证 | 适用于被复用文件的许可证，包括限制其范围的声明 |
| 复用方式 | 产品文件与上游文件的关系 |
| 修改内容 | 改了什么。只有字节一致时才写 `none` |
| 引入计划 | 加入该产品文件的计划 |
| 状态 | `in-product`、`gap` 或 `not-in-this-commit` |

看过上游视觉风格后再重写组件，不能记成复用行。缺少上游组件时写成缺口行，附上候选和证据；在所有者批准自行实现或其他来源之前，它一直是缺口。

## 本提交的产品行

计划 016 加入这些文件。基线是 main `58b7d4fb13f5084222bb985c5c4a81a8b82692de`，即计划 015、GitHub PR #22 的合并。`packages/ui/upstream.json` 是树内的同一份登记。下表 SHA-256 是产品文件。修改内容为 `none` 的行，该摘要也是上游 blob。四个有适配的文件在 `upstream.json` 里另记上游摘要。

Glasswork 的 `NOTICE`（`824058026d7338161460df1fda278d593e7a1682d9601c3780167c7fd6144a29`）写明仓库代码采用 MIT，macOS 壁纸媒体不在该许可内。本提交只入库 `lens.js` 和 `glass.css`，不入库那些媒体。Fluent UI 和 Astryx 不是产品依赖。

`packages/ui/dist` 是这些来源的构建产物。`dist/glass.css` 是 Glasswork 文件的逐字节副本。`dist/layout.css` 是入库的 `index.css` 与 `theme.css` 的 Tailwind 构建结果。`dist/shell.js` 是 esbuild 打包结果。`dist/stamp.json` 记录 vendor 与组合源码的 SHA-256。这些产物不是单独的上游组件。

已关闭的 PR #21 中的 `packages/ui/static/shell.js` 和 `packages/ui/static/themes.css` 不在本提交中。它们不是官方表面。

| ID | 产品路径 | 上游仓库 | 版本 | 源码路径 | 许可证 | 复用方式 | 修改内容 | 引入计划 | 状态 | SHA-256 |
|---|---|---|---|---|---|---|---|---|---|---|
| amicro-LICENSE | `packages/ui/vendor/amicro/LICENSE` | Subhan-code/Amicro--Micro-transitions- | 1.0.1, `fa5f8ffbe39537a8db191806066b8773c47c82e9` | `LICENSE` | MIT | 许可证文本，逐字节 | none | 016 | in-product | `4b2e0abfc3fdc8722545c3772ee7c6fd0bcdbe8af76112af50066689fca4a9c2` |
| amicro-fade-in-tsx | `packages/ui/vendor/amicro/fade-in.tsx` | Subhan-code/Amicro--Micro-transitions- | 1.0.1, `fa5f8ffbe39537a8db191806066b8773c47c82e9` | `registry/ui/entrance/fade-in.tsx` | MIT | 编入官方界面 | none | 016 | in-product | `742d0118dfb2237da82e62fc8740f1144e270bcb921ed561ec896f97b4d89612` |
| glasswork-LICENSE | `packages/ui/vendor/glasswork/LICENSE` | kubai087/glasswork | 1.1.0, `4f0b6744b898b3a9b98fbd913ab24fdb83c53162` | `LICENSE` | MIT | 许可证文本，逐字节 | none | 016 | in-product | `b38c1defe9e46be8baa663da135c74b3edb0d2ae8c930e9b41d9405f6013e512` |
| glasswork-NOTICE | `packages/ui/vendor/glasswork/NOTICE` | kubai087/glasswork | 1.1.0, `4f0b6744b898b3a9b98fbd913ab24fdb83c53162` | `NOTICE` | MIT | 许可证文本，逐字节 | none | 016 | in-product | `824058026d7338161460df1fda278d593e7a1682d9601c3780167c7fd6144a29` |
| glasswork-glass-css | `packages/ui/vendor/glasswork/glass.css` | kubai087/glasswork | 1.1.0, `4f0b6744b898b3a9b98fbd913ab24fdb83c53162` | `glass.css` | MIT | 编入官方界面 | none | 016 | in-product | `d13548bf70660fbb25c5361d7dc60c1fc47fc0803e4b4937d062753b664d2290` |
| glasswork-lens-js | `packages/ui/vendor/glasswork/lens.js` | kubai087/glasswork | 1.1.0, `4f0b6744b898b3a9b98fbd913ab24fdb83c53162` | `lens.js` | MIT | 编入官方界面 | none | 016 | in-product | `7dc1c4b5c37fc3aec7ee4903f643f1be774d680dcae79a95e38461cb00bb9c31` |
| shadcn-admin-LICENSE | `packages/ui/vendor/shadcn-admin/LICENSE` | satnaing/shadcn-admin | 2.2.1, `a156d9b90519348c4d71f0f0e6fcea179c49a43f` | `LICENSE` | MIT | 许可证文本，逐字节 | none | 016 | in-product | `d28c723c33a18dfed6af67b1a0ac368724f79bcb80750d3e080dbcf8dedb9e4b` |
| shadcn-admin-src-components-layout-app-sidebar-tsx | `packages/ui/vendor/shadcn-admin/src/components/layout/app-sidebar.tsx` | satnaing/shadcn-admin | 2.2.1, `a156d9b90519348c4d71f0f0e6fcea179c49a43f` | `src/components/layout/app-sidebar.tsx` | MIT | 随布局源码入库，官方界面不挂载 | none | 016 | in-product | `4683366bd5448d62945121dbe35e29cafb42dd854e2a4c4d85063b74b9e69b5f` |
| shadcn-admin-src-components-layout-app-title-tsx | `packages/ui/vendor/shadcn-admin/src/components/layout/app-title.tsx` | satnaing/shadcn-admin | 2.2.1, `a156d9b90519348c4d71f0f0e6fcea179c49a43f` | `src/components/layout/app-title.tsx` | MIT | 编入官方界面 | 接受 title 和 subtitle。默认值仍是上游字符串 Shadcn-Admin 和 Vite + ShadcnUI。 | 016 | in-product | `eede5bd4166090f05265338cbaf5b12cfb4a14d5506e3a5ef0f381c6ac5542a3` |
| shadcn-admin-src-components-layout-authenticated-layout-tsx | `packages/ui/vendor/shadcn-admin/src/components/layout/authenticated-layout.tsx` | satnaing/shadcn-admin | 2.2.1, `a156d9b90519348c4d71f0f0e6fcea179c49a43f` | `src/components/layout/authenticated-layout.tsx` | MIT | 编入官方界面 | 要求传入侧栏节点，跳过链接文案可选。渲染传入的侧栏，不再渲染 AppSidebar。SidebarInset 的 class 未改。 | 016 | in-product | `cb2b9038f55f063a66778098c602bf4ab5901eeae6d579755bab9f0e3ea735e1` |
| shadcn-admin-src-components-layout-data-sidebar-data-ts | `packages/ui/vendor/shadcn-admin/src/components/layout/data/sidebar-data.ts` | satnaing/shadcn-admin | 2.2.1, `a156d9b90519348c4d71f0f0e6fcea179c49a43f` | `src/components/layout/data/sidebar-data.ts` | MIT | 随布局源码入库，官方界面不挂载 | none | 016 | in-product | `5b1fcb85008f6c3ddffe42ec7e08181514a492063bd33a8977ce223c664c5770` |
| shadcn-admin-src-components-layout-header-tsx | `packages/ui/vendor/shadcn-admin/src/components/layout/header.tsx` | satnaing/shadcn-admin | 2.2.1, `a156d9b90519348c4d71f0f0e6fcea179c49a43f` | `src/components/layout/header.tsx` | MIT | 编入官方界面 | none | 016 | in-product | `f19060d14bdcb6efa98f1eee05452ecdc22d39a8a358af0d6e319e7ec9563777` |
| shadcn-admin-src-components-layout-main-tsx | `packages/ui/vendor/shadcn-admin/src/components/layout/main.tsx` | satnaing/shadcn-admin | 2.2.1, `a156d9b90519348c4d71f0f0e6fcea179c49a43f` | `src/components/layout/main.tsx` | MIT | 编入官方界面 | none | 016 | in-product | `e200dbc0e1b8cf383fdf6b7f7588ac1c9419de65f2658692e7941324bd09c572` |
| shadcn-admin-src-components-layout-nav-group-tsx | `packages/ui/vendor/shadcn-admin/src/components/layout/nav-group.tsx` | satnaing/shadcn-admin | 2.2.1, `a156d9b90519348c4d71f0f0e6fcea179c49a43f` | `src/components/layout/nav-group.tsx` | MIT | 编入官方界面 | none | 016 | in-product | `dd90536e487751b28fd8c850a0728a67784d5185c589b62fca580c992b1d2ed6` |
| shadcn-admin-src-components-layout-nav-user-tsx | `packages/ui/vendor/shadcn-admin/src/components/layout/nav-user.tsx` | satnaing/shadcn-admin | 2.2.1, `a156d9b90519348c4d71f0f0e6fcea179c49a43f` | `src/components/layout/nav-user.tsx` | MIT | 随布局源码入库，官方界面不挂载 | none | 016 | in-product | `739f42c8f0b2afde6a0f202f751399dea4b0a72c36113488bffb707f09207b34` |
| shadcn-admin-src-components-layout-sidebar-sponsors-tsx | `packages/ui/vendor/shadcn-admin/src/components/layout/sidebar-sponsors.tsx` | satnaing/shadcn-admin | 2.2.1, `a156d9b90519348c4d71f0f0e6fcea179c49a43f` | `src/components/layout/sidebar-sponsors.tsx` | MIT | 随布局源码入库，官方界面不挂载 | none | 016 | in-product | `aef77970d2a1e92b397477d74be45f7a4112448cdc3b762155be3c20c76d9d2a` |
| shadcn-admin-src-components-layout-team-switcher-tsx | `packages/ui/vendor/shadcn-admin/src/components/layout/team-switcher.tsx` | satnaing/shadcn-admin | 2.2.1, `a156d9b90519348c4d71f0f0e6fcea179c49a43f` | `src/components/layout/team-switcher.tsx` | MIT | 随布局源码入库，官方界面不挂载 | none | 016 | in-product | `5ea0594bf435f2e8cde44994e68e1c753ed12a9c864bc5db4292ce0d42749299` |
| shadcn-admin-src-components-layout-top-nav-tsx | `packages/ui/vendor/shadcn-admin/src/components/layout/top-nav.tsx` | satnaing/shadcn-admin | 2.2.1, `a156d9b90519348c4d71f0f0e6fcea179c49a43f` | `src/components/layout/top-nav.tsx` | MIT | 随布局源码入库，官方界面不挂载 | none | 016 | in-product | `9064632a68016ead787798e90eb7fc4b3cd2fc32274e22df943f92b55c722065` |
| shadcn-admin-src-components-layout-types-ts | `packages/ui/vendor/shadcn-admin/src/components/layout/types.ts` | satnaing/shadcn-admin | 2.2.1, `a156d9b90519348c4d71f0f0e6fcea179c49a43f` | `src/components/layout/types.ts` | MIT | 编入官方界面 | none | 016 | in-product | `07cbb707e5e950c2751d638c3e1fc6ed85df5306ba90789fe81d571533f681c8` |
| shadcn-admin-src-components-skip-to-main-tsx | `packages/ui/vendor/shadcn-admin/src/components/skip-to-main.tsx` | satnaing/shadcn-admin | 2.2.1, `a156d9b90519348c4d71f0f0e6fcea179c49a43f` | `src/components/skip-to-main.tsx` | MIT | 编入官方界面 | 接受 label。默认值仍是 Skip to Main。 | 016 | in-product | `a273f90f72dbadd9e771718c0e70fd4914c868dfaf95544a7320e25cfdbeb7f4` |
| shadcn-admin-src-components-ui-badge-tsx | `packages/ui/vendor/shadcn-admin/src/components/ui/badge.tsx` | satnaing/shadcn-admin | 2.2.1, `a156d9b90519348c4d71f0f0e6fcea179c49a43f` | `src/components/ui/badge.tsx` | MIT | 编入官方界面 | none | 016 | in-product | `a642d7a932b91ec610f09078452d34c379900845066955311506b6d503ea11e6` |
| shadcn-admin-src-components-ui-button-tsx | `packages/ui/vendor/shadcn-admin/src/components/ui/button.tsx` | satnaing/shadcn-admin | 2.2.1, `a156d9b90519348c4d71f0f0e6fcea179c49a43f` | `src/components/ui/button.tsx` | MIT | 编入官方界面 | none | 016 | in-product | `97ed8ef5651a6e264ded042b024b5be39673ba2ff222017fbcf845cf3ed37226` |
| shadcn-admin-src-components-ui-collapsible-tsx | `packages/ui/vendor/shadcn-admin/src/components/ui/collapsible.tsx` | satnaing/shadcn-admin | 2.2.1, `a156d9b90519348c4d71f0f0e6fcea179c49a43f` | `src/components/ui/collapsible.tsx` | MIT | 编入官方界面 | none | 016 | in-product | `9fd25f52e0f6fd1bbfff0f1f3853fecd6a1c7ee1a9118a73f2e59a2ffa411e0a` |
| shadcn-admin-src-components-ui-dropdown-menu-tsx | `packages/ui/vendor/shadcn-admin/src/components/ui/dropdown-menu.tsx` | satnaing/shadcn-admin | 2.2.1, `a156d9b90519348c4d71f0f0e6fcea179c49a43f` | `src/components/ui/dropdown-menu.tsx` | MIT | 编入官方界面 | none | 016 | in-product | `41ec3204cbc0917e3cf082a80a862b5ae1ea9e92c77dbbaaaa3906f28f74110b` |
| shadcn-admin-src-components-ui-input-tsx | `packages/ui/vendor/shadcn-admin/src/components/ui/input.tsx` | satnaing/shadcn-admin | 2.2.1, `a156d9b90519348c4d71f0f0e6fcea179c49a43f` | `src/components/ui/input.tsx` | MIT | 编入官方界面 | none | 016 | in-product | `4f7d443722b4aeacb1179b863b98f773f7950514ceb1d897b66c967521f08072` |
| shadcn-admin-src-components-ui-separator-tsx | `packages/ui/vendor/shadcn-admin/src/components/ui/separator.tsx` | satnaing/shadcn-admin | 2.2.1, `a156d9b90519348c4d71f0f0e6fcea179c49a43f` | `src/components/ui/separator.tsx` | MIT | 编入官方界面 | none | 016 | in-product | `aa5cc28cdebe9264228c622e48363b9e8a2684436e47c255bb1a0a905610aaef` |
| shadcn-admin-src-components-ui-sheet-tsx | `packages/ui/vendor/shadcn-admin/src/components/ui/sheet.tsx` | satnaing/shadcn-admin | 2.2.1, `a156d9b90519348c4d71f0f0e6fcea179c49a43f` | `src/components/ui/sheet.tsx` | MIT | 编入官方界面 | none | 016 | in-product | `2423f0e67f594198df542acabd32547896925e35786c6bb085f8a4eeed4273c4` |
| shadcn-admin-src-components-ui-sidebar-tsx | `packages/ui/vendor/shadcn-admin/src/components/ui/sidebar.tsx` | satnaing/shadcn-admin | 2.2.1, `a156d9b90519348c4d71f0f0e6fcea179c49a43f` | `src/components/ui/sidebar.tsx` | MIT | 编入官方界面 | none | 016 | in-product | `40f021f27b3043b2af4c247b99f665b323edd142a4317a7b84ac4e3017070e9c` |
| shadcn-admin-src-components-ui-skeleton-tsx | `packages/ui/vendor/shadcn-admin/src/components/ui/skeleton.tsx` | satnaing/shadcn-admin | 2.2.1, `a156d9b90519348c4d71f0f0e6fcea179c49a43f` | `src/components/ui/skeleton.tsx` | MIT | 编入官方界面 | none | 016 | in-product | `7617060c9ea8a25dafadf224a8567c91daebd4a43149d0e72143bba807fab457` |
| shadcn-admin-src-components-ui-tooltip-tsx | `packages/ui/vendor/shadcn-admin/src/components/ui/tooltip.tsx` | satnaing/shadcn-admin | 2.2.1, `a156d9b90519348c4d71f0f0e6fcea179c49a43f` | `src/components/ui/tooltip.tsx` | MIT | 编入官方界面 | none | 016 | in-product | `adb364c0798a914d6a37f2dc0f22a75b0537be568149ee9b90794b2352c03315` |
| shadcn-admin-src-context-layout-provider-tsx | `packages/ui/vendor/shadcn-admin/src/context/layout-provider.tsx` | satnaing/shadcn-admin | 2.2.1, `a156d9b90519348c4d71f0f0e6fcea179c49a43f` | `src/context/layout-provider.tsx` | MIT | 编入官方界面 | none | 016 | in-product | `11fe4cabe4684e42d0a99e7b2277ec6fc01919c16036c2e7a6985841c318f671` |
| shadcn-admin-src-context-search-provider-tsx | `packages/ui/vendor/shadcn-admin/src/context/search-provider.tsx` | satnaing/shadcn-admin | 2.2.1, `a156d9b90519348c4d71f0f0e6fcea179c49a43f` | `src/context/search-provider.tsx` | MIT | 编入官方界面 | 去掉 CommandMenu 的导入和元素。搜索上下文和 Ctrl/Cmd-K 监听保留。演示命令菜单不挂载。 | 016 | in-product | `266cedbfea4f5080d5a97099841ba834aa7363664bf900c040a74c390f218d31` |
| shadcn-admin-src-hooks-use-mobile-tsx | `packages/ui/vendor/shadcn-admin/src/hooks/use-mobile.tsx` | satnaing/shadcn-admin | 2.2.1, `a156d9b90519348c4d71f0f0e6fcea179c49a43f` | `src/hooks/use-mobile.tsx` | MIT | 编入官方界面 | none | 016 | in-product | `cef2003db553ef127ba219e8f4303a733cc76b85f83f17b1622131c0ecfd3b3c` |
| shadcn-admin-src-lib-cookies-ts | `packages/ui/vendor/shadcn-admin/src/lib/cookies.ts` | satnaing/shadcn-admin | 2.2.1, `a156d9b90519348c4d71f0f0e6fcea179c49a43f` | `src/lib/cookies.ts` | MIT | 编入官方界面 | none | 016 | in-product | `18c757bec8c35d721c4c619934a3703c5c327c2d05a88e3ab3ee6699ed69ae40` |
| shadcn-admin-src-lib-utils-ts | `packages/ui/vendor/shadcn-admin/src/lib/utils.ts` | satnaing/shadcn-admin | 2.2.1, `a156d9b90519348c4d71f0f0e6fcea179c49a43f` | `src/lib/utils.ts` | MIT | 编入官方界面 | none | 016 | in-product | `cc244af40baa5fe88abff3f80b424a7aa654b3137a7db612a8fe94861b61e04e` |
| shadcn-admin-src-styles-index-css | `packages/ui/vendor/shadcn-admin/src/styles/index.css` | satnaing/shadcn-admin | 2.2.1, `a156d9b90519348c4d71f0f0e6fcea179c49a43f` | `src/styles/index.css` | MIT | 编入官方界面 | none | 016 | in-product | `ee0477a25d5836191f9efcdd7260aa2b75e358865f4fecdeb59ac4e3bbefce46` |
| shadcn-admin-src-styles-theme-css | `packages/ui/vendor/shadcn-admin/src/styles/theme.css` | satnaing/shadcn-admin | 2.2.1, `a156d9b90519348c4d71f0f0e6fcea179c49a43f` | `src/styles/theme.css` | MIT | 编入官方界面 | none | 016 | in-product | `faee559c9a21b2084bf7bae193e70f0ab18b18a393a06fec1408669652c266a1` |

## 组合

这些文件把官方界面接到宿主状态。它们不是上游组件，也不是一套新的视觉体系。

| ID | 产品路径 | 上游仓库 | 版本 | 源码路径 | 许可证 | 复用方式 | 修改内容 | 引入计划 | 状态 |
|---|---|---|---|---|---|---|---|---|---|
| winstrata-copy | `packages/ui/compose/copy.ts` | 无 | 无 | 无 | Apache-2.0 | WinStrata 组合 | 英文和 zh-CN 文案。未设置语言时使用英文。 | 016 | in-product |
| winstrata-entry | `packages/ui/compose/entry.tsx` | 无 | 无 | 无 | Apache-2.0 | WinStrata 组合 | 官方页面的内存路由。 | 016 | in-product |
| winstrata-shell | `packages/ui/compose/shell.tsx` | 无 | 无 | 无 | Apache-2.0 | WinStrata 组合 | 挂载 shadcn 外壳、Glasswork 材质和 Amicro 淡入。绑定真实宿主状态。 | 016 | in-product |

三套主题共用这个外壳。白色圆角亚克力使用 shadcn 浅色令牌和 Glasswork `data-kind=light`。黑色圆角亚克力使用 shadcn `html.dark` 令牌和 Glasswork `data-kind=dark`。液态玻璃使用浅色玻璃面板，并只在带 `data-lens` 的按钮上调用 `liquidGlass()`。用户选择之前不保存主题。[OI-023](OPEN_ITEMS.md#oi-023) 仍开放。

## 缺口

| ID | 产品路径 | 上游仓库 | 版本 | 源码路径 | 许可证 | 复用方式 | 修改内容 | 引入计划 | 状态 |
|---|---|---|---|---|---|---|---|---|---|
| ui-moments | 无 | guanmo-ai/ui-moments | 修订 `c9eece0fe425ee7dfe97d162d39d1665c3bf9368` | 无 | 未审查 | 未纳入 | none | 016 | not-in-this-commit |

UI Moments 仍是 [OI-009](OPEN_ITEMS.md#oi-009)。本计划不自造组件填补这个缺口，也不另加来源。
