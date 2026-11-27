# Desktop shell

[简体中文](../zh-CN/architecture/DESKTOP.md) · [Frontend](FRONTEND.md) · [Service IPC](SERVICE-KERNEL.md) · [Plugins](PLUGINS.md) · [Plan 016](../roadmap/PLAN.md#pr-016)

Plan 016 adds `apps/desktop` and `packages/ui`. The GUI process uses the current user's ordinary privileges. It does not subscribe to drivers, start collectors, or run onboarding.

## Host

`winstrata-desktop` serves the page on `127.0.0.1` at an unguessable path prefix. A request without that prefix is rejected. The page talks to the host with loopback HTTP. The host talks to the privileged service with the existing `GuiClient`, which exposes status only.

`Endpoint::client` connects to an address that is already listening and does not delete it. `Endpoint::user_address` is the default service address and does not unlink a live socket. If the service is absent, the page says status is unavailable and does not invent idle drivers.

The shell lists the 25 official plugins. With a runtime file and a selected storage directory it opens `PluginHost` and shows `list()`. Otherwise it shows the same catalog with `implementation_absent` and says the host was not opened. It does not enable plugins.

Settings can call `Core::select_directory`. A rejected path stays unselected and the error is shown. Theme, language, and the close preference are stored in `gui.json` next to the locator, not in `core.json`. Version 1 of that file starts with no theme, English, and close set to ask.

## Upstream components and themes

The three measured sources are copied at the revisions in the [platform report](PLATFORM_FEASIBILITY.md):

| Source | Revision | Product files |
|---|---|---|
| Amicro 1.0.1 | `fa5f8ffbe39537a8db191806066b8773c47c82e9` | `registry/ui/entrance/fade-in.tsx` |
| Glasswork 1.1.0 | `4f0b6744b898b3a9b98fbd913ab24fdb83c53162` | `lens.js`, `glass.css` |
| shadcn-admin 2.2.1 | `a156d9b90519348c4d71f0f0e6fcea179c49a43f` | `src/components/layout/main.tsx`, `src/lib/utils.ts` |

Apple media named in the Glasswork NOTICE is not copied. The bundle keeps the two locked React versions apart: 19.2.7 with Amicro and 19.2.5 with shadcn-admin. The bundler is esbuild 0.25.12 from the Amicro lock. Vite is not a dependency. [GHSA-587h-7wmj-h5pm](https://github.com/advisories/GHSA-587h-7wmj-h5pm) is a Vite development-server bypass and does not apply to this bundler.

No theme id is stored until the user chooses one. White Acrylic is the proposed initial theme and is not the default. That proposal still needs the owner's confirmation. See [OI-023](../records/OPEN_ITEMS.md#oi-023).

Liquid Glass buttons call the verbatim Glasswork `liquidGlass` function and `glass.css`. The panel colors in `themes.css` are host-authored paint. They are not an upstream material, they are paused, and they are not visual acceptance. The navigation and header in `shell.js` are also host-authored. They are not the shadcn-admin sidebar or header. shadcn-admin contributes `Main` and `utils.ts` only. Each surface is listed in `packages/ui/upstream.json`.

## Compatibility with the 2026-11-28 GUI decision

The owner described plugin replacement of the whole GUI on 2026-11-28. That decision is not in effect until a separate AG-60 standards PR, AG-64, is approved and merged. This plan does not implement it. There is no second marketplace and no frontend plugin SDK. The loopback commands are this page's host channel, not an extension API. The process opens Core for settings and the plugin catalog. That is the current host, not a claim that a later GUI can be replaced without touching this process. Third-party GUI replacement is not started.

Hidden pages and `prefers-reduced-motion` stop the lens and the fade. The home layout is not draggable. That remains plan 042.

## Languages, tray, and window

English and Simplified Chinese cover the same message keys. A missing translation uses English. The window-close dialog offers tray continuation or exit and can remember the choice. On Linux the tray control reports that it is unavailable. On Windows the host creates a notification icon: left click asks the page to show, right click exits. The visible window is the signed WebView2 host in `apps/desktop/windows/host.ps1`. If that SDK is absent, the host exits 2 and does not claim the window ran.

## What this plan does not do

UI Moments commit `c9eece0fe425ee7dfe97d162d39d1665c3bf9368` was read. Its project license is MIT. No UI Moments component was bundled. Fluent UI and Astryx are not product dependencies. Their WebView2 and performance measurement remains [OI-009](../records/OPEN_ITEMS.md#oi-009).

Onboarding, live and history switching, dashboard editing, and the market remain later plans. The pages for those areas say they are not in this version. E2E-11 is not complete until a real Windows WebView2 session records the three themes, the tray, and the privilege. This authoring host is Linux and did not run that session. See [OI-013](../records/OPEN_ITEMS.md#oi-013).

Build the page assets with `node packages/ui/build.mjs target/plan016-ui` before `--assets`. The command checks the pinned SHA-512 values and does not run package install scripts.
