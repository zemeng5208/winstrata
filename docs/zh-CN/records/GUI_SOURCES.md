# GUI 来源登记

[English](../../records/GUI_SOURCES.md) · [AG-64](../AGENTS.md#ag-64) · [前端规范](../architecture/FRONTEND.md#section-9) · [索引](../INDEX.md)

本登记由 [AG-64](../AGENTS.md#ag-64) 要求，适用于官方 GUI 和官方插件。[STATUS.md](STATUS.md) 不能代替本登记。一行记录表示该产品文件与这一行处于同一提交。第三方 GUI 插件可以使用自己的设计体系，不为它们补写“来自四个指定来源”的行。它们的安全、兼容性和性能证据没有实现计划编号。计划 053 记录本登记，并紧接在 014 之后。它不修改计划 016。

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

main `0accacbaa2dac43db3845cc8c08f23b4e831c413` 是计划 014、GitHub PR #20 的合并。本计划列出工作树，并搜索 `.css`、`.scss`、`.tsx`、`.jsx`、`.vue`、`.html` 文件，排除 `.git` 和 `target`。搜索没有返回文件。树中没有 `apps/`、`packages/`、`plugins/`、`sdk/`。没有可以登记的官方 GUI 组件或布局。

| ID | 产品路径 | 上游仓库 | 版本 | 源码路径 | 许可证 | 复用方式 | 修改内容 | 引入计划 | 状态 |
|---|---|---|---|---|---|---|---|---|---|
| — | 无 | — | — | — | — | — | — | — | 产品树中没有 GUI 文件 |

计划 004 在产品工作区之外测量了 Amicro、Glasswork 和 shadcn-admin。那份报告不是产品行。UI Moments 已指定，没有产品文件。见 [OI-009](OPEN_ITEMS.md#oi-009)。

## 未合并计划 016 的检查

本节是证据。它不是验收，也不增加产品行。本计划不修改那个分支。

2026-11-28，本计划读取未合并的 [GitHub PR #21](https://github.com/zemeng5208/winstrata/pull/21)，分支 `pr/016-desktop-gui`，头 `64bd627c362ad025869905441bee7802dda19ac1`。它从 `raw.githubusercontent.com` 下载下表所列修订的上游 blob 并计算 SHA-256。每个摘要都等于该头上的 blob，也等于该头 `packages/ui/upstream.json` 中的摘要。

| ID | 上游仓库 | 版本与修订 | 源码路径 | SHA-256 | 许可证文件 | 该头上的复用 | 修改内容 |
|---|---|---|---|---|---|---|---|
| amicro-fade-in | Subhan-code/Amicro--Micro-transitions- | 1.0.1，`fa5f8ffbe39537a8db191806066b8773c47c82e9` | `registry/ui/entrance/fade-in.tsx` | `742d0118dfb2237da82e62fc8740f1144e270bcb921ed561ec896f97b4d89612` | 上游 `LICENSE`，`4b2e0abfc3fdc8722545c3772ee7c6fd0bcdbe8af76112af50066689fca4a9c2` | `packages/ui/vendor/amicro/fade-in.tsx` | 无 |
| glasswork-lens | kubai087/glasswork | 1.1.0，`4f0b6744b898b3a9b98fbd913ab24fdb83c53162` | `lens.js` | `7dc1c4b5c37fc3aec7ee4903f643f1be774d680dcae79a95e38461cb00bb9c31` | 上游 `LICENSE`，`b38c1defe9e46be8baa663da135c74b3edb0d2ae8c930e9b41d9405f6013e512` | `packages/ui/vendor/glasswork/lens.js` | 无 |
| glasswork-css | kubai087/glasswork | 1.1.0，`4f0b6744b898b3a9b98fbd913ab24fdb83c53162` | `glass.css` | `d13548bf70660fbb25c5361d7dc60c1fc47fc0803e4b4937d062753b664d2290` | 同一 LICENSE。上游 `NOTICE` 等于 `824058026d7338161460df1fda278d593e7a1682d9601c3780167c7fd6144a29` | `packages/ui/vendor/glasswork/glass.css` | 无 |
| shadcn-main | satnaing/shadcn-admin | 2.2.1，`a156d9b90519348c4d71f0f0e6fcea179c49a43f` | `src/components/layout/main.tsx` | `e200dbc0e1b8cf383fdf6b7f7588ac1c9419de65f2658692e7941324bd09c572` | 上游 `LICENSE`，`d28c723c33a18dfed6af67b1a0ac368724f79bcb80750d3e080dbcf8dedb9e4b` | `packages/ui/vendor/shadcn-admin/src/components/layout/main.tsx` | 无 |
| shadcn-utils | satnaing/shadcn-admin | 2.2.1，`a156d9b90519348c4d71f0f0e6fcea179c49a43f` | `src/lib/utils.ts` | `cc244af40baa5fe88abff3f80b424a7aa654b3137a7db612a8fe94861b61e04e` | 同一 LICENSE | `packages/ui/vendor/shadcn-admin/src/lib/utils.ts` | 无。`main.tsx` 引用它 |

本登记不转述这些 LICENSE 或 NOTICE 正文。本次执行的检查是字节是否相等，不是新的许可意见。

同一头上还有并非这些上游来源的文件：

| ID | 该头上的路径 | 发现 | 状态 |
|---|---|---|---|
| host-chrome | `packages/ui/static/shell.js` | 手写的导航、顶部区域和对话框。这不是 shadcn-admin 的侧栏或顶部。SHA-256 `498b7e08f895d32094f5478cbe52b9c82c7bac5e95f48d7b7fa80aa8e096db79` | 缺口。不作为官方布局验收 |
| host-acrylic-paint | `packages/ui/static/themes.css` | 文件写明这是暂停的手写绘制，不是 Amicro、Glasswork、shadcn-admin 或 UI Moments。SHA-256 `8ddf4f054baa696bbb02769ce66151573b3386043824b2d1a27d0410c5d8850c` | 缺口。不作为三套官方材质验收 |
| ui-moments | 无 | 该头上没有 UI Moments 文件。已指定修订仍是 `c9eece0fe425ee7dfe97d162d39d1665c3bf9368` | 未纳入。[OI-009](OPEN_ITEMS.md#oi-009) 仍开放 |

`packages/ui/static/page.html` 链接 `layout.css`、`amicro.js`、`shadcn.js` 和 `glass.js`。这四个路径不是该提交中的文件。本次变更没有运行 GUI，没有把该页面当作验收，也没有把文件复制到 main。缺口留在那个未合并分支上。本次变更不修改它。计划 053 是紧接 014 之后的规范行；它不写入 015 或 016 的直接依赖格。见 [OI-036](OPEN_ITEMS.md#oi-036)。
