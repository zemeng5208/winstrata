# 平台可行性验证

[English](../../architecture/PLATFORM_FEASIBILITY.md) · [计划 004](../roadmap/PLAN.md#pr-004) · [工程记录](../records/STATUS.md)

## 原生采集与运行时边界

Windows 探针通过 CIM 读取系统版本和构建号，用 Windows 事件日志接口最多读取 8 条最近的 System 记录，检查每条记录有编号和时间戳。事件记录仅输出计数与耗时。实测环境为 x64 Windows Server 2022 Datacenter，构建号 20348，进程持有管理员令牌。8 条记录均通过，耗时 98 ms。普通用户权限、持续订阅、事件损失以及 Windows 10/11 客户端版需分别实测。事件日志和 CIM 可用于历史查询和系统清单；高频 ETW、抓包和内核回调的覆盖范围与成本须单独评估。[Windows 运行记录](https://github.com/zemeng5208/winstrata/actions/runs/45965865720) · [Windows 事件日志](https://learn.microsoft.com/en-us/windows/win32/wes/windows-event-log)

Windows Job Object 的进程数上限为 1，且已纳入一个存活子进程时，加入第二个进程被系统拒绝，错误码 1816。上限改为 2 后，新的对照进程成功加入。关闭最后一个句柄，两个成员进程均在时限内退出。同一 runner 的此项操作耗时 27 ms。Job Object 可用于生命周期和配额管理。文件访问还需要独立的系统权限边界。[Windows 运行记录](https://github.com/zemeng5208/winstrata/actions/runs/45965865720) · [Job Object](https://learn.microsoft.com/en-us/windows/win32/procthread/job-objects) · [AppContainer 隔离](https://learn.microsoft.com/en-us/windows/win32/secauthz/appcontainer-isolation)

在同一 Windows 版本上，测试创建本用户的临时 AppContainer，不请求附加 capability。每次启动只读子进程时先从令牌核验包 SID，再读取仅含已知标记的本次测试文件。单独的始终授权文件可读。对同一目标文件的三次新打开分别返回 Win32 5（拒绝）、0（授权后内容正确）、5（撤销后拒绝）。初次启动用自定义环境时返回 203；改用 Windows 为该用户生成的标准环境后启动成功。旧比较工具对正负对照均返回 2，随后改用镜像预装的微软 x64 C 编译器生成只读诊断程序，直接检查打开、读取、内容和系统错误。退出时清理临时文件和配置。此结论只覆盖新打开的文件；已打开句柄的撤销、网络、注册表和完整 Python/Node 运行时仍待验证。[AppContainer 运行记录](https://github.com/zemeng5208/winstrata/actions/runs/45967512493)

在 Linux x64 上，为两个临时插件分别执行 `python -m venv --without-pip`，在各自的 site-packages 中设置同名私有测试模块；用每个环境的 `python -I` 启动。Node 24.22.3 分别从两个插件的 node_modules 加载同名 ESM 模块，再各运行一份 .mjs 和带类型标注的 .mts 文件。6 个新进程均解析到各自模块，仅为第一个插件提供的包在第二个插件内按名称导入失败，进程正常退出后清理临时环境。这项测试覆盖按名称依赖查找。绝对文件访问、网络、系统 API 和跨用户授权仍需产品级 OS 边界验收。

## 本机 MCP

诊断程序通过标准输入输出启动独立 Node 子进程，用发现请求核对协议版本，验证 JSON-RPC 请求编号、一条进度通知和工具返回的真实子进程 PID。测试覆盖缺失及不支持的版本元数据、无授权调用被拒绝、按范围授权、撤权和管道关闭后的正常退出。合成授权文件由测试程序控制。产品服务还需可信授权存储、调用进程身份校验、跨用户拒绝、审计与资源预算。仅上报 PID 无法证明进程身份。回环传输也需独立于监听地址的身份认证。[现行 stdio 传输规范](https://modelcontextprotocol.io/specification/2026-07-28/basic/transports/stdio) · [发现协议](https://modelcontextprotocol.io/specification/2026-07-28/server/discover)

本地 Node 24.22.3 的双进程测试通过。用固定版本的官方 JSON Schema 核验了 3 条请求、3 条响应和 1 条通知。另以环境已安装的 Python MCP SDK 1.29.0 作互操作对照：该版本拒绝现代发现请求，协商旧版 2025-11-25 的 initialize 并发送 initialized 通知后，列举工具、调用并比对真实 PID 和管道关闭均成功。正式 Rust 实现可进一步评估官方 rmcp 的 stdio、异步运行时和序列化依赖，需审批版本、许可与依赖树。[官方 Schema](https://github.com/modelcontextprotocol/modelcontextprotocol/blob/024d5255eb4466ba4fab21e044b5ba84d6a79327/schema/2026-07-28/schema.json) · [Rust SDK 候选](https://github.com/modelcontextprotocol/rust-sdk/releases/tag/rmcp-v3.12.0)

## 界面来源与许可

2026-11-27，所有者批准临时使用以下精确修订及其上游锁定的构建依赖进行组件实测。项目工作区未添加产品依赖。三个修订均提供 MIT 许可证。分发前需保留版权并核对所选依赖链的许可。

| 来源与核验修订 | 可复用内容与集成成本 |
|---|---|
| [Amicro 1.0.1](https://github.com/Subhan-code/Amicro--Micro-transitions-/tree/fa5f8ffbe39537a8db191806066b8773c47c82e9) | 注册表的 FadeIn 用 React/Motion 实现透明度过渡。可选用单个动效，并适配用户的减少动态效果偏好与应用状态。完整示例中的支付和分析功能不在使用范围内。 |
| [Glasswork 1.1.0](https://github.com/kubai087/glasswork/tree/4f0b6744b898b3a9b98fbd913ab24fdb83c53162) | 独立的 glass.css 和 lens.js 可为无障碍原生按钮提供玻璃材质。SVG 位移图和滤镜需验证可见区域内的渲染开销与卸载。NOTICE 明确排除了 Apple 壁纸和视频。测试和交付均排除演示媒体、远程图片和演示工具。[NOTICE](https://github.com/kubai087/glasswork/blob/4f0b6744b898b3a9b98fbd913ab24fdb83c53162/NOTICE) |
| [shadcn-admin 2.2.1](https://github.com/satnaing/shadcn-admin/tree/a156d9b90519348c4d71f0f0e6fcea179c49a43f) | Main 提供固定、流式及响应式内容布局。完整外壳还依赖多个 Provider、侧栏、TanStack Router 与 Tailwind。建议选择所需布局组件，接入 WinStrata 的统一主题控制；演示认证、示例数据和基于 Cookie 的明暗或系统主题偏好需分别处理。 |

在 Server 2022 的真实嵌入式 WebView2 131.0.2903.86 中，使用预装的微软 SDK 与加载器 1.0.1661.34 打开自编离线按钮页，控制器启动耗时 6,249 ms。可见页面采得 30 个动画帧间隔，中位数 15.6 ms，p95 为 16.7 ms。计算样式显示背景模糊 6 px，最终水平偏移 30 px。此为诊断宿主单次测量。产品启动、CPU、GPU、I/O 及整套主题预算仍需在实际桌面应用内测量。[WebView2 运行记录](https://github.com/zemeng5208/winstrata/actions/runs/45965942453)

在 Linux 的 Chromium 156 中，离线页面三次冷启动均通过 6 项检查。Amicro 动效（含设置的延时和持续时间）653.7–664.9 ms，Glasswork 滤镜准备 65.1–65.5 ms，shadcn-admin Main 挂载布局 10.6–13.0 ms。Main 的测试将命名容器由 1,440 px 缩至 640 px，再切到流式布局。每次采样 30 个帧间隔，p95 为 16.7–16.8 ms。构建仅提取固定修订中的所需文件并校验所需 npm 包的 SHA-512；篡改摘要的测试被拒绝。临时目录单独保存两个上游锁定的 React 版本和版权声明。无上游演示媒体、分发包或安装脚本进入产品工作区。

修正 Windows 缺少 Git blob 的取件方式后，重新从固定提交核验原始文件，再在真实 WebView2 131.0.2903.86 中离线运行相同的三个组件。6 项检查均通过，30 个帧间隔的中位数 15.6 ms，p95 为 15.7 ms。Amicro 实际过渡 661.4 ms，Glasswork 折射滤镜准备 78.6 ms，shadcn-admin Main 挂载布局 9.2 ms。运行顺序为先打开基础页、再启动新的组件宿主，此次 271 ms 的宿主启动耗时包含预热效应。在 Linux 重新抓取和构建的三次复测中有一轮 p95 达到 33.2 ms。[Windows 组件与 MCP 实测](https://github.com/zemeng5208/winstrata/actions/runs/45966969310)

## 复现

在 Linux 或 Windows x64 的仓库根目录，用获批的 Node 24.22.3 运行以下命令。输出目录须为空；源码与依赖取自指定上游，生成的页面可离线使用。

```sh
node --test --test-isolation=none .github/scripts/pr004-mcp.test.mjs
node .github/scripts/pr004-ui-build.mjs target/pr004-components
```

在 64 位 Windows PowerShell 5.1 的 STA 进程执行以下命令。WebView2 探针使用镜像中预装且经签名校验的微软程序集和加载器，并通过仅允许本地资源的虚拟主机装载组件。AppContainer 探针创建当前用户下的临时配置和夹具，结束时清理。

```powershell
.\.github\scripts\pr004-windows.ps1
.\.github\scripts\pr004-appcontainer.ps1
.\.github\scripts\pr004-webview.ps1 -AssetDirectory target/pr004-components
```

## 选型与验收

建议从上述已实测的三个项目按需复用组件。2026-11-27，所有者在[前端规范](FRONTEND.md#section-9)中把 [UI Moments](https://github.com/guanmo-ai/ui-moments) 追加为指定来源。该指定不是已完成的实测：UI Moments、Fluent UI 与 Astryx 尚未接受本报告所记录的修订核验、许可链核对或 WebView2 测量。不得把其演示页当作已验证组件，也不得把本报告读成批准交付 Fluent UI 或 Astryx。已实测来源的研究用包和资源保留在交付物之外。试验关闭开发服务器。上游锁定的 Vite 开发工具存在已知安全公告，选择产品工具链前须重新核对。[前端边界](FRONTEND.md#section-9) · [Vite 安全公告](https://github.com/advisories/GHSA-587h-7wmj-h5pm)

建议继续评估 Windows 内置采集、AppContainer 文件授权搭配 Job Object 配额、本机子进程 stdio 和所选界面组件。正式采用运行时、IPC 契约、库或界面项目版本须由所有者选定。Windows 10/11 x64、混合运行时的完整隔离、长时间运行的性能和整机产品仍待验证。Phase 0 须在所有者复核剩余风险及生效合并门禁后验收。
