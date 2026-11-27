# GUI source register

[简体中文](../zh-CN/records/GUI_SOURCES.md) · [AG-64](../../AGENTS.md#ag-64) · [Frontend standard](../architecture/FRONTEND.md#section-9) · [Index](../INDEX.md)

This register is required by [AG-64](../../AGENTS.md#ag-64) for the official GUI and for official plugins. [STATUS.md](STATUS.md) is not a substitute. A row is a claim that the named product file is in the same commit as the row. Third-party GUI plugins may use their own design system; they do not receive a row that pretends they came from the four approved sources. Their security, compatibility, and performance evidence has no implementation plan ID. Plan 053 records this register and sits immediately after 014. It does not edit plan 016.

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

## Product rows at this commit

Main `0accacbaa2dac43db3845cc8c08f23b4e831c413` is the merge of plan 014, GitHub PR #20. This plan listed the worktree and searched it for `.css`, `.scss`, `.tsx`, `.jsx`, `.vue`, and `.html` files, excluding `.git` and `target`. The search returned no files. `apps/`, `packages/`, `plugins/`, and `sdk/` are not in the tree. There is no official GUI component or layout to register.

| ID | Product path | Upstream repository | Version | Source path | License | Reuse method | Modifications | Introducing plan | State |
|---|---|---|---|---|---|---|---|---|---|
| — | none | — | — | — | — | — | — | — | The product tree has no GUI file |

Plan 004 measured Amicro, Glasswork, and shadcn-admin outside the product workspace. That report is not a product row. UI Moments is designated and has no product file. See [OI-009](OPEN_ITEMS.md#oi-009).

## Unmerged plan 016 inspection

This section is evidence. It is not acceptance, and it does not add product rows. This plan does not edit that branch.

On 2026-11-28 this plan read open [GitHub PR #21](https://github.com/zemeng5208/winstrata/pull/21), branch `pr/016-desktop-gui`, head `64bd627c362ad025869905441bee7802dda19ac1`. It downloaded the upstream blobs below from `raw.githubusercontent.com` at the named revisions and computed SHA-256. Each digest equals the blob on that head and the digest in that head's `packages/ui/upstream.json`.

| ID | Upstream repository | Version and revision | Source path | SHA-256 | License file | Reuse on that head | Modifications |
|---|---|---|---|---|---|---|---|
| amicro-fade-in | Subhan-code/Amicro--Micro-transitions- | 1.0.1, `fa5f8ffbe39537a8db191806066b8773c47c82e9` | `registry/ui/entrance/fade-in.tsx` | `742d0118dfb2237da82e62fc8740f1144e270bcb921ed561ec896f97b4d89612` | Upstream `LICENSE`, `4b2e0abfc3fdc8722545c3772ee7c6fd0bcdbe8af76112af50066689fca4a9c2` | `packages/ui/vendor/amicro/fade-in.tsx` | none |
| glasswork-lens | kubai087/glasswork | 1.1.0, `4f0b6744b898b3a9b98fbd913ab24fdb83c53162` | `lens.js` | `7dc1c4b5c37fc3aec7ee4903f643f1be774d680dcae79a95e38461cb00bb9c31` | Upstream `LICENSE`, `b38c1defe9e46be8baa663da135c74b3edb0d2ae8c930e9b41d9405f6013e512` | `packages/ui/vendor/glasswork/lens.js` | none |
| glasswork-css | kubai087/glasswork | 1.1.0, `4f0b6744b898b3a9b98fbd913ab24fdb83c53162` | `glass.css` | `d13548bf70660fbb25c5361d7dc60c1fc47fc0803e4b4937d062753b664d2290` | Same LICENSE. Upstream `NOTICE` matches `824058026d7338161460df1fda278d593e7a1682d9601c3780167c7fd6144a29` | `packages/ui/vendor/glasswork/glass.css` | none |
| shadcn-main | satnaing/shadcn-admin | 2.2.1, `a156d9b90519348c4d71f0f0e6fcea179c49a43f` | `src/components/layout/main.tsx` | `e200dbc0e1b8cf383fdf6b7f7588ac1c9419de65f2658692e7941324bd09c572` | Upstream `LICENSE`, `d28c723c33a18dfed6af67b1a0ac368724f79bcb80750d3e080dbcf8dedb9e4b` | `packages/ui/vendor/shadcn-admin/src/components/layout/main.tsx` | none |
| shadcn-utils | satnaing/shadcn-admin | 2.2.1, `a156d9b90519348c4d71f0f0e6fcea179c49a43f` | `src/lib/utils.ts` | `cc244af40baa5fe88abff3f80b424a7aa654b3137a7db612a8fe94861b61e04e` | Same LICENSE | `packages/ui/vendor/shadcn-admin/src/lib/utils.ts` | none. `main.tsx` imports it |

This register does not restate those LICENSE or NOTICE texts. Byte equality is the check that was run. It is not a new license opinion.

The same head also contains files that are not those upstream sources:

| ID | Path on that head | Finding | State |
|---|---|---|---|
| host-chrome | `packages/ui/static/shell.js` | Handwritten navigation, header, and dialogs. This is not the shadcn-admin sidebar or header. SHA-256 `498b7e08f895d32094f5478cbe52b9c82c7bac5e95f48d7b7fa80aa8e096db79` | Gap. Not accepted as official layout |
| host-acrylic-paint | `packages/ui/static/themes.css` | The file says it is paused host-authored paint and not Amicro, Glasswork, shadcn-admin, or UI Moments. SHA-256 `8ddf4f054baa696bbb02769ce66151573b3386043824b2d1a27d0410c5d8850c` | Gap. Not accepted as the three official materials |
| ui-moments | none | No UI Moments file is on that head. The designated revision remains `c9eece0fe425ee7dfe97d162d39d1665c3bf9368` | Not included. [OI-009](OPEN_ITEMS.md#oi-009) stays open |

`packages/ui/static/page.html` links `layout.css`, `amicro.js`, `shadcn.js`, and `glass.js`. Those four paths are not files in that commit. This change did not run the GUI, did not treat the page as acceptance, and did not copy the files onto main. The gap stays on that unmerged branch. This change does not edit it. Plan 053 is the standards row immediately after 014; it is not added to the dependency cells of 015 or 016. See [OI-036](OPEN_ITEMS.md#oi-036).
