# 桌面壳

[English](../../architecture/DESKTOP.md) · [前端](FRONTEND.md) · [服务 IPC](SERVICE-KERNEL.md) · [插件](PLUGINS.md) · [计划 016](../roadmap/PLAN.md#pr-016)

计划 016 增加 `apps/desktop` 和 `packages/ui`。GUI 进程使用当前用户的普通权限。它不订阅驱动，不启动采集，也不运行初始化向导。

## 宿主

`winstrata-desktop` 在 `127.0.0.1` 上用一个不可猜测的路径前缀提供页面。没有该前缀的请求会被拒绝。页面通过回环 HTTP 访问宿主。宿主用现有的 `GuiClient` 访问受控服务，该客户端只能查询状态。

`Endpoint::client` 连接到已经在监听的地址，并且不删除它。`Endpoint::user_address` 是默认服务地址，不会拆除正在监听的套接字。服务不存在时，页面显示状态不可用，不编造空闲驱动。

壳层列出 25 个官方插件。提供了运行时文件并且已经选择存储目录时，它打开 `PluginHost` 并显示 `list()`。否则它显示同一目录，可用性为 `implementation_absent`，并说明宿主没有打开。它不启用插件。

设置可以调用 `Core::select_directory`。被拒绝的路径保持未选择，并显示错误。主题、语言和关闭偏好保存在定位目录旁的 `gui.json`，不写入 `core.json`。该文件的版本 1 初始没有主题，语言是英文，关闭方式是每次询问。

## 上游组件和主题

三个已测量来源按[平台报告](PLATFORM_FEASIBILITY.md)中的修订复制：

| 来源 | 修订 | 产品文件 |
|---|---|---|
| Amicro 1.0.1 | `fa5f8ffbe39537a8db191806066b8773c47c82e9` | `registry/ui/entrance/fade-in.tsx` |
| Glasswork 1.1.0 | `4f0b6744b898b3a9b98fbd913ab24fdb83c53162` | `lens.js`、`glass.css` |
| shadcn-admin 2.2.1 | `a156d9b90519348c4d71f0f0e6fcea179c49a43f` | `src/components/layout/main.tsx`、`src/lib/utils.ts` |

Glasswork NOTICE 点名的 Apple 媒体没有复制。构建保持两套锁定的 React 分开：Amicro 使用 19.2.7，shadcn-admin 使用 19.2.5。打包器是 Amicro 锁文件中的 esbuild 0.25.12。Vite 不是依赖。[GHSA-587h-7wmj-h5pm](https://github.com/advisories/GHSA-587h-7wmj-h5pm) 是 Vite 开发服务器绕过，不适用于这个打包器。

用户选择之前不保存任何主题标识。白色圆角亚克力是建议的初始主题，不是默认值。该建议仍需所有者确认。见 [OI-023](../records/OPEN_ITEMS.md#oi-023)。

液态玻璃按钮调用未经修改的 Glasswork `liquidGlass` 和 `glass.css`。`themes.css` 里的面板颜色是宿主自己画的。它不是上游材质，已经暂停，也不能当作视觉验收。`shell.js` 里的导航和顶栏也是宿主自己写的，不是 shadcn-admin 的侧栏或顶栏。shadcn-admin 只贡献了 `Main` 和 `utils.ts`。每个界面的来源列在 `packages/ui/upstream.json`。

## 与 2026-11-28 GUI 决定的关系

所有者在 2026-11-28 描述了插件替换整个 GUI。该决定要等独立的 AG-60 规范 PR，也就是 AG-64，获得批准并合并后才生效。本计划不实现它。没有第二个市场，也没有前端插件 SDK。回环命令是这个页面自己的宿主通道，不是扩展接口。进程为了设置和插件目录会打开 Core。这是当前宿主，不是在声称以后可以不碰这个进程就换掉 GUI。第三方 GUI 替换没有开始。

隐藏页面和 `prefers-reduced-motion` 会停止透镜和淡入。首页布局不能拖拽。那仍属于计划 042。

## 语言、托盘和窗口

英文和简体中文覆盖同一组文案键。缺少的翻译回退到英文。关闭窗口时可以留在托盘或退出，并可以记住选择。在 Linux 上，托盘控件报告不可用。在 Windows 上，宿主创建通知图标：左键要求页面显示，右键退出。可见窗口是 `apps/desktop/windows/host.ps1` 中的已签名 WebView2 宿主。如果没有该 SDK，宿主以退出码 2 结束，不声称窗口已经运行。

## 本计划不做什么

已读取 UI Moments 提交 `c9eece0fe425ee7dfe97d162d39d1665c3bf9368`。其项目许可证是 MIT。没有打包任何 UI Moments 组件。Fluent UI 和 Astryx 不是产品依赖。它们的 WebView2 和性能测量仍是 [OI-009](../records/OPEN_ITEMS.md#oi-009)。

初始化、实时与历史切换、仪表盘编辑和市场仍属于后续计划。这些区域的页面写明它们不属于这个版本。E2E-11 要等真实 Windows WebView2 会话记录三个主题、托盘和权限之后才完成。编写本计划的宿主是 Linux，没有运行该会话。见 [OI-013](../records/OPEN_ITEMS.md#oi-013)。

在使用 `--assets` 之前，用 `node packages/ui/build.mjs target/plan016-ui` 构建页面资源。该命令核对固定的 SHA-512，并且不运行包安装脚本。
