# 需求追踪矩阵

[English](../../roadmap/REQUIREMENTS.md) · [文档索引](../INDEX.md)

范围和准则以关联的权威条款为准。所有计划中项目的运行证据待取得。仓库基线与已取得的开发工具链证据分别标记。变更时为受影响条目更新实际证据及状态。

## 产品与工程需求

| ID | 要求 | 权威条款 | 计划 PR | 验收 | 状态或证据 |
|---|---|---|---|---|---|
| REQ-01.01 | Windows 系统可观测与变更追踪 | [PRODUCT](../architecture/PRODUCT.md#section-2) | [000](PLAN.md#pr-000) | [16](ACCEPTANCE.md#e2e-16) | 规划中 |
| REQ-01.02 | 公开单仓库、main 和 Apache-2.0 基线 | [PRODUCT](../architecture/PRODUCT.md#section-2) | [000](PLAN.md#pr-000) | [16](ACCEPTANCE.md#e2e-16) | [基线已核验](../records/STATUS.md#baseline) |
| REQ-01.03 | 交付源码、SDK、调试资料与开发版 | [ACCEPTANCE](../roadmap/ACCEPTANCE.md#section-20) | [050](PLAN.md#pr-050) | [16](ACCEPTANCE.md#e2e-16) | 规划中 |
| REQ-02.01 | 在真实 Windows 10 和 11 x64 运行 | [SYSTEM](../architecture/SYSTEM.md#section-3) | [004](PLAN.md#pr-004), [049](PLAN.md#pr-049) | [14](ACCEPTANCE.md#e2e-14), [16](ACCEPTANCE.md#e2e-16) | Windows Server 2022 x64 已实测，Windows 10/11 客户端验收待办。 |
| REQ-02.02 | 纯 Linux 构建完整 Windows x64 产物 | [SYSTEM](../architecture/SYSTEM.md#section-3) | [003](PLAN.md#pr-003), [050](PLAN.md#pr-050) | [16](ACCEPTANCE.md#e2e-16) | Linux 控制台与内核链接已验证，完整产物链待验证。 |
| REQ-02.03 | 保留双环境构建能力，实际验证当前环境 | [DEPENDENCIES](../standards/DEPENDENCIES.md#ag-35) | [001](PLAN.md#pr-001), [050](PLAN.md#pr-050) | [16](ACCEPTANCE.md#e2e-16) | Linux 工具链已核验，双环境构建验收待完成。 |
| REQ-03.01 | GUI 与 Core 普通权限，受控服务与内核权限分离 | [SYSTEM](../architecture/SYSTEM.md#section-3) | [008](PLAN.md#pr-008) | [14](ACCEPTANCE.md#e2e-14), [15](ACCEPTANCE.md#e2e-15) | 已实现[服务权限分离](../architecture/SERVICE-KERNEL.md)。Linux 不能订阅，`GuiClient` 没有变更接口。Windows 已拒绝受限令牌的 SCM 创建权限。管道 ACL 拒绝了该令牌，随后普通客户端遇到已关闭的实例。E2E-14 与 E2E-15 未完成。 |
| REQ-03.02 | 版本化 IPC、验证身份与参数并审计 | [SYSTEM](../architecture/SYSTEM.md#section-3) | [005](PLAN.md#pr-005), [008](PLAN.md#pr-008) | [14](ACCEPTANCE.md#e2e-14), [15](ACCEPTANCE.md#e2e-15) | [版本与错误契约](../architecture/PROTOCOLS.md)仍然有效。[服务 IPC](../architecture/SERVICE-KERNEL.md)已在 Linux 检查对端身份、角色、参数和审计。第一次 Windows 管道运行在这些检查之前失败，因为实例创建过晚。 |
| REQ-03.03 | 官方接口优先的混合采集与驱动独立按需生命周期 | [SYSTEM](../architecture/SYSTEM.md#section-3) | [008–011](PLAN.md#pr-008) | [02](ACCEPTANCE.md#e2e-02), [04](ACCEPTANCE.md#e2e-04), [14](ACCEPTANCE.md#e2e-14), [15](ACCEPTANCE.md#e2e-15) | [订阅管理](../architecture/SERVICE-KERNEL.md)跟踪独立引用计数和被限制的故障。由于没有已签名映像，目录宿主拒绝全部模块。采集仍属于 PR 009–011。尚未在 Windows 上真实加载或卸载。 |
| REQ-03.04 | 签名、Secure Boot 和 HVCI 保持启用 | [SECURITY](../standards/SECURITY.md#ag-47) | [008](PLAN.md#pr-008) | [14](ACCEPTANCE.md#e2e-14), [15](ACCEPTANCE.md#e2e-15) | 计划 008 不关闭签名、Secure Boot 或 HVCI，也不加载未签名映像。尚未在这些机制启用时加载已签名的 INF/CAT。 |
| REQ-04.01 | 当前用户、相关公共活动与用户选择的存储范围 | [PRODUCT](../architecture/PRODUCT.md#section-2) | [006](PLAN.md#pr-006), [017](PLAN.md#pr-017) | [01](ACCEPTANCE.md#e2e-01), [02](ACCEPTANCE.md#e2e-02) | [已实现当前用户身份、私有目录和监视范围配置](../architecture/CORE-STORAGE.md)。采集端过滤按 PR 017 验证。 |
| REQ-04.02 | 初始化选择启用，默认手动启动且不开机自启 | [FRONTEND](../architecture/FRONTEND.md#section-9) | [017](PLAN.md#pr-017) | [01](ACCEPTANCE.md#e2e-01), [11](ACCEPTANCE.md#e2e-11) | 规划中 |
| REQ-04.03 | 建立选定范围基线并标注重建历史 | [PRODUCT](../architecture/PRODUCT.md#section-4) | [017](PLAN.md#pr-017), [021](PLAN.md#pr-021) | [01](ACCEPTANCE.md#e2e-01), [02](ACCEPTANCE.md#e2e-02) | 规划中 |
| REQ-04.04 | 实时与历史切换、暂停恢复、缺失记录和安全退出 | [FRONTEND](../architecture/FRONTEND.md#section-9) | [017](PLAN.md#pr-017) | [11](ACCEPTANCE.md#e2e-11) | 规划中 |
| REQ-05.01 | 事件稳定标识和版本、时间顺序、来源与完整性 | [SYSTEM](../architecture/SYSTEM.md#section-3) | [005](PLAN.md#pr-005) | [02](ACCEPTANCE.md#e2e-02), [15](ACCEPTANCE.md#e2e-15) | [事件标识、排序、来源与缺口契约](../architecture/PROTOCOLS.md)已通过真实 Node 子进程、Windows 日志、兼容用例与确定性回放验证；产品规模的多来源对齐留待集成场景。 |
| REQ-05.02 | 有界批处理、队列、背压、吞吐及丢失记录 | [SYSTEM](../architecture/SYSTEM.md#section-3) | [007](PLAN.md#pr-007) | [08](ACCEPTANCE.md#e2e-08), [15](ACCEPTANCE.md#e2e-15) | [Core 运行时](../architecture/CORE-RUNTIME.md)已限制队列和批量，在记录丢失前等待，并发布协议缺口。Linux release 在 39.925 毫秒内传送 20,000 条事件。E2E-08 与 E2E-15 仍待完成。 |
| REQ-05.03 | 统一优先级、超时、取消和资源保护 | [ARCHITECTURE](../standards/ARCHITECTURE.md#ag-45) | [007](PLAN.md#pr-007), [025](PLAN.md#pr-025), [026](PLAN.md#pr-026) | [08](ACCEPTANCE.md#e2e-08), [15](ACCEPTANCE.md#e2e-15) | 调度器已实现优先级、超时、取消和固定的工作者与队列上限。动态预算和健康诊断仍属 PR 025 与 PR 026。E2E-08 与 E2E-15 仍待完成。 |
| REQ-06.01 | 文件操作、所有权与权限；不存文件内容版本 | [PRODUCT](../architecture/PRODUCT.md#section-4) | [009](PLAN.md#pr-009), [021](PLAN.md#pr-021) | [02](ACCEPTANCE.md#e2e-02) | 规划中 |
| REQ-06.02 | 观测进程、线程、模块、来源及 PID 生命周期 | [PRODUCT](../architecture/PRODUCT.md#section-4) | [010](PLAN.md#pr-010), [018](PLAN.md#pr-018) | [02](ACCEPTANCE.md#e2e-02), [03](ACCEPTANCE.md#e2e-03) | 规划中 |
| REQ-06.03 | 归因注册表与环境变量并呈现前后值 | [PRODUCT](../architecture/PRODUCT.md#section-4) | [019](PLAN.md#pr-019), [020](PLAN.md#pr-020) | [02](ACCEPTANCE.md#e2e-02) | 规划中 |
| REQ-06.04 | 观测服务、驱动、启动项与计划任务变化 | [PRODUCT](../architecture/PRODUCT.md#section-4) | [022](PLAN.md#pr-022), [023](PLAN.md#pr-023) | [03](ACCEPTANCE.md#e2e-03), [14](ACCEPTANCE.md#e2e-14) | 规划中 |
| REQ-07.01 | 识别自动与手动的软件生命周期会话 | [PRODUCT](../architecture/PRODUCT.md#section-4) | [035](PLAN.md#pr-035) | [03](ACCEPTANCE.md#e2e-03) | 规划中 |
| REQ-07.02 | 归因软件档案、残留依据及磁盘占用 | [PRODUCT](../architecture/PRODUCT.md#section-4) | [035](PLAN.md#pr-035) | [03](ACCEPTANCE.md#e2e-03), [09](ACCEPTANCE.md#e2e-09) | 规划中 |
| REQ-07.03 | 追溯真实依赖、签名、完整性和来源 | [PRODUCT](../architecture/PRODUCT.md#section-4) | [033](PLAN.md#pr-033), [034](PLAN.md#pr-034) | [03](ACCEPTANCE.md#e2e-03), [09](ACCEPTANCE.md#e2e-09) | 规划中 |
| REQ-08.01 | 展示受支持的 CPU、GPU、内存、电源与传感器历史 | [PRODUCT](../architecture/PRODUCT.md#section-4) | [024](PLAN.md#pr-024) | [08](ACCEPTANCE.md#e2e-08) | 规划中 |
| REQ-08.02 | 诊断和调节资源，保留可追溯降级记录 | [DATA](../architecture/DATA.md#section-8) | [025](PLAN.md#pr-025), [026](PLAN.md#pr-026) | [08](ACCEPTANCE.md#e2e-08), [15](ACCEPTANCE.md#e2e-15) | 规划中 |
| REQ-08.03 | 计量磁盘健康、逻辑及物理写入和 I/O | [DATA](../architecture/DATA.md#section-8) | [032](PLAN.md#pr-032), [048](PLAN.md#pr-048) | [08](ACCEPTANCE.md#e2e-08), [15](ACCEPTANCE.md#e2e-15) | 规划中 |
| REQ-09.01 | 观测可用接口并归因入站和出站连接 | [DATA](../architecture/DATA.md#section-7) | [011](PLAN.md#pr-011), [031](PLAN.md#pr-031) | [04](ACCEPTANCE.md#e2e-04) | 规划中 |
| REQ-09.02 | 授权发现、拓扑与可见性边界 | [DATA](../architecture/DATA.md#section-7) | [031](PLAN.md#pr-031) | [04](ACCEPTANCE.md#e2e-04) | 规划中 |
| REQ-10.01 | 记录风险标记来源、依据、时间与可信度 | [DATA](../architecture/DATA.md#section-7) | [029](PLAN.md#pr-029) | [04](ACCEPTANCE.md#e2e-04) | 规划中 |
| REQ-10.02 | 未标记原始数据包从采集到释放均不落盘 | [DATA](../architecture/DATA.md#section-7) | [031](PLAN.md#pr-031) | [04](ACCEPTANCE.md#e2e-04), [05](ACCEPTANCE.md#e2e-05) | 规划中 |
| REQ-10.03 | 配置有界抓包缓冲并导出风险抓包 | [DATA](../architecture/DATA.md#section-7) | [031](PLAN.md#pr-031) | [04](ACCEPTANCE.md#e2e-04) | 规划中 |
| REQ-11.01 | 默认 D 盘、手动回退与安全迁移 | [DATA](../architecture/DATA.md#section-8) | [006](PLAN.md#pr-006) | [01](ACCEPTANCE.md#e2e-01), [10](ACCEPTANCE.md#e2e-10) | [Core 选择存储、在失卷后暂停写入并校验文件迁移](../architecture/CORE-STORAGE.md)。Windows 客户端界面按 PR 016 验证。 |
| REQ-11.02 | 支持结构化、分段、时序、抓包和快照存储 | [DATA](../architecture/DATA.md#section-8) | [012](PLAN.md#pr-012) | [05](ACCEPTANCE.md#e2e-05), [10](ACCEPTANCE.md#e2e-10) | 规划中 |
| REQ-11.03 | 共享标识、唯一源记录与统一索引 | [ARCHITECTURE](../standards/ARCHITECTURE.md#ag-44) | [012](PLAN.md#pr-012), [040](PLAN.md#pr-040) | [09](ACCEPTANCE.md#e2e-09) | 规划中 |
| REQ-12.01 | 合格数据默认永久；可配置保留期限 | [DATA](../architecture/DATA.md#section-8) | [012](PLAN.md#pr-012), [030](PLAN.md#pr-030) | [05](ACCEPTANCE.md#e2e-05) | 规划中 |
| REQ-12.02 | 抓包 1 GB、日志 2 GB 提醒；持续记录 1 GB 和 500 MB 按阶重复 | [DATA](../architecture/DATA.md#section-8) | [030](PLAN.md#pr-030) | [05](ACCEPTANCE.md#e2e-05) | 规划中 |
| REQ-12.03 | 清理须用户确认，删除后同步索引 | [DATA](../architecture/DATA.md#section-8) | [030](PLAN.md#pr-030) | [05](ACCEPTANCE.md#e2e-05) | 规划中 |
| REQ-12.04 | 保留 Core 告警，标记存储不可用，禁止清理外部数据 | [DATA](../architecture/DATA.md#section-8) | [030](PLAN.md#pr-030) | [05](ACCEPTANCE.md#e2e-05), [15](ACCEPTANCE.md#e2e-15) | 规划中 |
| REQ-13.01 | 避免自监视递归，保留相关外部修改 | [DATA](../architecture/DATA.md#section-8) | [028](PLAN.md#pr-028) | [05](ACCEPTANCE.md#e2e-05), [08](ACCEPTANCE.md#e2e-08) | 规划中 |
| REQ-13.02 | GUI 与批处理脚本过滤，失败安全回退 | [DATA](../architecture/DATA.md#section-8) | [027](PLAN.md#pr-027), [028](PLAN.md#pr-028) | [04](ACCEPTANCE.md#e2e-04), [07](ACCEPTANCE.md#e2e-07) | 规划中 |
| REQ-13.03 | 减少无效 I/O 和写入放大，验证完整性 | [DATA](../architecture/DATA.md#section-8) | [032](PLAN.md#pr-032), [048](PLAN.md#pr-048) | [08](ACCEPTANCE.md#e2e-08), [15](ACCEPTANCE.md#e2e-15) | 规划中 |
| REQ-14.01 | 可选本地加密及锁定，锁定时保持监视 | [DATA](../architecture/DATA.md#section-8) | [013](PLAN.md#pr-013), [042](PLAN.md#pr-042) | [10](ACCEPTANCE.md#e2e-10) | 规划中 |
| REQ-14.02 | 验证备份、恢复、完整性、迁移及设备标识 | [DATA](../architecture/DATA.md#section-8) | [013](PLAN.md#pr-013), [038](PLAN.md#pr-038) | [10](ACCEPTANCE.md#e2e-10) | 规划中 |
| REQ-14.03 | 授权并验证导入导出，明文转换须确认 | [DATA](../architecture/DATA.md#section-8) | [038](PLAN.md#pr-038), [042](PLAN.md#pr-042) | [09](ACCEPTANCE.md#e2e-09), [10](ACCEPTANCE.md#e2e-10) | 规划中 |
| REQ-15.01 | 手动自选范围的完整及增量快照与差异视图 | [PRODUCT](../architecture/PRODUCT.md#section-4) | [038](PLAN.md#pr-038) | [07](ACCEPTANCE.md#e2e-07) | 规划中 |
| REQ-15.02 | 自动快照默认关闭，支持用户定义自动化 | [EXTENSIONS](../architecture/EXTENSIONS.md#section-5) | [027](PLAN.md#pr-027), [038](PLAN.md#pr-038) | [07](ACCEPTANCE.md#e2e-07) | 规划中 |
| REQ-16.01 | 统一 Python、TS、JS 脚本工作台、调试、触发和历史 | [EXTENSIONS](../architecture/EXTENSIONS.md#section-5) | [027](PLAN.md#pr-027) | [07](ACCEPTANCE.md#e2e-07), [12](ACCEPTANCE.md#e2e-12) | 规划中 |
| REQ-16.02 | 只读运行可视化规则、阈值与通知 | [PRODUCT](../architecture/PRODUCT.md#section-4) | [039](PLAN.md#pr-039) | [07](ACCEPTANCE.md#e2e-07) | 规划中 |
| REQ-16.03 | 执行脚本权限及依赖检查且不内置预设 | [EXTENSIONS](../architecture/EXTENSIONS.md#section-5) | [015](PLAN.md#pr-015), [027](PLAN.md#pr-027) | [07](ACCEPTANCE.md#e2e-07), [12](ACCEPTANCE.md#e2e-12) | 规划中 |
| REQ-17.01 | 只读展示系统更新、KB、驱动、功能、重启及错误 | [PRODUCT](../architecture/PRODUCT.md#section-4) | [036](PLAN.md#pr-036) | [14](ACCEPTANCE.md#e2e-14) | 规划中 |
| REQ-17.02 | 只读展示可观测安全与证书状态 | [PRODUCT](../architecture/PRODUCT.md#section-4) | [037](PLAN.md#pr-037) | [14](ACCEPTANCE.md#e2e-14) | 规划中 |
| REQ-18.01 | 检索和关联授权来源，并追溯对象证据 | [FRONTEND](../architecture/FRONTEND.md#section-9) | [040](PLAN.md#pr-040) | [09](ACCEPTANCE.md#e2e-09) | 规划中 |
| REQ-18.02 | 多轨时间线、仅供分析的回放及区间导出 | [PRODUCT](../architecture/PRODUCT.md#section-4) | [041](PLAN.md#pr-041) | [09](ACCEPTANCE.md#e2e-09) | 规划中 |
| REQ-19.01 | 验证三项指定来源，按许可小幅适配 shadcn-admin | [FRONTEND](../architecture/FRONTEND.md#section-9) | [004](PLAN.md#pr-004), [016](PLAN.md#pr-016) | [11](ACCEPTANCE.md#e2e-11) | 已核查三个指定修订，组件实测见[平台报告](../architecture/PLATFORM_FEASIBILITY.md)。产品布局仍待验收。 |
| REQ-19.02 | 三套共享主题可运行时切换并持久保存 | [FRONTEND](../architecture/FRONTEND.md#section-9) | [016](PLAN.md#pr-016) | [11](ACCEPTANCE.md#e2e-11) | 规划中 |
| REQ-19.03 | 完整双语与 Windows WebView2 可读的真实交互 | [FRONTEND](../architecture/FRONTEND.md#section-9) | [016](PLAN.md#pr-016), [042](PLAN.md#pr-042) | [11](ACCEPTANCE.md#e2e-11) | 规划中 |
| REQ-19.04 | 持久化单首页布局并接入动态插件界面 | [FRONTEND](../architecture/FRONTEND.md#section-9) | [042](PLAN.md#pr-042) | [11](ACCEPTANCE.md#e2e-11) | 规划中 |
| REQ-19.05 | 提供托盘继续、完整退出和真实界面状态 | [FRONTEND](../architecture/FRONTEND.md#section-9) | [016](PLAN.md#pr-016), [017](PLAN.md#pr-017) | [01](ACCEPTANCE.md#e2e-01), [11](ACCEPTANCE.md#e2e-11) | 规划中 |
| REQ-20.01 | 随附 25 个插件，用户选择启用，内置项不可卸载 | [PRODUCT](../architecture/PRODUCT.md#section-4) | [014](PLAN.md#pr-014), [017](PLAN.md#pr-017) | [06](ACCEPTANCE.md#e2e-06), [11](ACCEPTANCE.md#e2e-11) | 规划中 |
| REQ-20.02 | 独立生命周期、资源释放与兼容替换 | [ARCHITECTURE](../standards/ARCHITECTURE.md#ag-43) | [014](PLAN.md#pr-014) | [06](ACCEPTANCE.md#e2e-06) | 规划中 |
| REQ-20.03 | 验证动态升级、依赖状态与故障隔离 | [EXTENSIONS](../architecture/EXTENSIONS.md#section-5) | [014](PLAN.md#pr-014), [044](PLAN.md#pr-044) | [06](ACCEPTANCE.md#e2e-06), [15](ACCEPTANCE.md#e2e-15) | 规划中 |
| REQ-21.01 | 能力声明与最小权限管理、撤销和审计 | [EXTENSIONS](../architecture/EXTENSIONS.md#section-5) | [014](PLAN.md#pr-014), [015](PLAN.md#pr-015) | [06](ACCEPTANCE.md#e2e-06), [07](ACCEPTANCE.md#e2e-07) | 规划中 |
| REQ-21.02 | 按需且隔离的 Python、TS/JS 运行时及依赖环境 | [EXTENSIONS](../architecture/EXTENSIONS.md#section-5) | [004](PLAN.md#pr-004), [015](PLAN.md#pr-015) | [06](ACCEPTANCE.md#e2e-06), [07](ACCEPTANCE.md#e2e-07), [12](ACCEPTANCE.md#e2e-12) | 本机隔离候选已有[实测记录](../architecture/PLATFORM_FEASIBILITY.md)，产品隔离待验收。 |
| REQ-21.03 | 提供指定语言 SDK 并验证操作系统级隔离 | [EXTENSIONS](../architecture/EXTENSIONS.md#section-5) | [015](PLAN.md#pr-015) | [06](ACCEPTANCE.md#e2e-06), [12](ACCEPTANCE.md#e2e-12) | 规划中 |
| REQ-22.01 | 本地优先统一市场，含官方、社区、已安装和更新管理 | [EXTENSIONS](../architecture/EXTENSIONS.md#section-5) | [043](PLAN.md#pr-043) | [06](ACCEPTANCE.md#e2e-06), [12](ACCEPTANCE.md#e2e-12) | 规划中 |
| REQ-22.02 | 安装复合包并管理来源、警告和校验 | [EXTENSIONS](../architecture/EXTENSIONS.md#section-5) | [043](PLAN.md#pr-043) | [06](ACCEPTANCE.md#e2e-06) | 规划中 |
| REQ-22.03 | 自由提交与自动检查，正式发布须明确同意 | [EXTENSIONS](../architecture/EXTENSIONS.md#section-5) | [043](PLAN.md#pr-043), [046](PLAN.md#pr-046) | [06](ACCEPTANCE.md#e2e-06), [12](ACCEPTANCE.md#e2e-12) | 规划中 |
| REQ-23.01 | 核心组件和扩展分别版本化并可锁定 | [EXTENSIONS](../architecture/EXTENSIONS.md#section-5) | [014](PLAN.md#pr-014), [044](PLAN.md#pr-044) | [06](ACCEPTANCE.md#e2e-06), [15](ACCEPTANCE.md#e2e-15) | 规划中 |
| REQ-23.02 | 验证签名更新、回退并保留配置与历史 | [EXTENSIONS](../architecture/EXTENSIONS.md#section-5) | [044](PLAN.md#pr-044) | [06](ACCEPTANCE.md#e2e-06), [15](ACCEPTANCE.md#e2e-15) | 规划中 |
| REQ-24.01 | 认证本机 MCP，动态注册和撤销授权能力 | [EXTENSIONS](../architecture/EXTENSIONS.md#section-6) | [045](PLAN.md#pr-045) | [12](ACCEPTANCE.md#e2e-12) | 规划中 |
| REQ-24.02 | 按资源预算与审计提供授权实时、历史查询 | [EXTENSIONS](../architecture/EXTENSIONS.md#section-6) | [045](PLAN.md#pr-045) | [12](ACCEPTANCE.md#e2e-12) | 规划中 |
| REQ-24.03 | 通过本机 MCP 开发、验证、安装及发布插件与脚本 | [EXTENSIONS](../architecture/EXTENSIONS.md#section-6) | [046](PLAN.md#pr-046) | [12](ACCEPTANCE.md#e2e-12) | 规划中 |
| REQ-25.01 | 救援默认关闭并排除批量启用 | [EXTENSIONS](../architecture/EXTENSIONS.md#section-6) | [047](PLAN.md#pr-047) | [13](ACCEPTANCE.md#e2e-13) | 规划中 |
| REQ-25.02 | 仅许可列明的本机救援动作，校验目标并审计 | [EXTENSIONS](../architecture/EXTENSIONS.md#section-6) | [047](PLAN.md#pr-047) | [13](ACCEPTANCE.md#e2e-13) | 规划中 |
| REQ-25.03 | 拒绝任意命令、越权目标及撤销后操作 | [EXTENSIONS](../architecture/EXTENSIONS.md#section-6) | [047](PLAN.md#pr-047) | [13](ACCEPTANCE.md#e2e-13) | 规划中 |
| REQ-26.01 | 监视数据、遥测、文件和哈希留在本机 | [PRODUCT](../architecture/PRODUCT.md#section-2) | [013](PLAN.md#pr-013), [044](PLAN.md#pr-044), [045](PLAN.md#pr-045), [046](PLAN.md#pr-046) | [04](ACCEPTANCE.md#e2e-04), [06](ACCEPTANCE.md#e2e-06), [10](ACCEPTANCE.md#e2e-10), [12](ACCEPTANCE.md#e2e-12) | 规划中 |
| REQ-26.02 | 展示真实来源、不确定性、缺失及不支持状态 | [DEVELOPMENT](../standards/DEVELOPMENT.md#ag-34) | [014](PLAN.md#pr-014), [040](PLAN.md#pr-040), [045](PLAN.md#pr-045) | [06](ACCEPTANCE.md#e2e-06), [09](ACCEPTANCE.md#e2e-09), [12](ACCEPTANCE.md#e2e-12) | 规划中 |
| REQ-26.03 | 外部 Agent 的决策、模型和凭据边界 | [EXTENSIONS](../architecture/EXTENSIONS.md#section-6) | [043](PLAN.md#pr-043), [045](PLAN.md#pr-045), [046](PLAN.md#pr-046) | [06](ACCEPTANCE.md#e2e-06), [12](ACCEPTANCE.md#e2e-12) | 规划中 |
| REQ-27.01 | 实测后台 150 MB 内存及 1% CPU 目标 | [ACCEPTANCE](../roadmap/ACCEPTANCE.md#section-10) | [048](PLAN.md#pr-048) | [08](ACCEPTANCE.md#e2e-08), [15](ACCEPTANCE.md#e2e-15) | 规划中 |
| REQ-27.02 | 实测 GUI 350 MB 内存、产物 250 MB 及冷启动 3 秒目标 | [ACCEPTANCE](../roadmap/ACCEPTANCE.md#section-10) | [048](PLAN.md#pr-048), [050](PLAN.md#pr-050) | [08](ACCEPTANCE.md#e2e-08), [16](ACCEPTANCE.md#e2e-16) | 规划中 |
| REQ-27.03 | 验证未标记包零写入、禁用插件近零专属资源及 I/O 开销 | [ACCEPTANCE](../roadmap/ACCEPTANCE.md#section-10) | [031](PLAN.md#pr-031), [048](PLAN.md#pr-048) | [04](ACCEPTANCE.md#e2e-04), [08](ACCEPTANCE.md#e2e-08), [15](ACCEPTANCE.md#e2e-15) | 规划中 |
| REQ-28.01 | 集中工具链、锁文件及项目依赖解析 | [SYSTEM](../architecture/SYSTEM.md#section-11) | [001](PLAN.md#pr-001) | [01](ACCEPTANCE.md#e2e-01), [16](ACCEPTANCE.md#e2e-16) | Linux 已核验工作区版本及路径。 |
| REQ-28.02 | 集中配置、迁移与敏感项管理 | [DEPENDENCIES](../standards/DEPENDENCIES.md#ag-40) | [001](PLAN.md#pr-001), [006](PLAN.md#pr-006) | [01](ACCEPTANCE.md#e2e-01), [16](ACCEPTANCE.md#e2e-16) | 已实现工具链配置与[按用户隔离的版本化运行配置](../architecture/CORE-STORAGE.md)。密钥管理和加密迁移留待 PR 013。 |
| REQ-28.03 | 统一开发入口并验证仓库洁净度 | [DEPENDENCIES](../standards/DEPENDENCIES.md#ag-19) | [001](PLAN.md#pr-001), [002](PLAN.md#pr-002) | [16](ACCEPTANCE.md#e2e-16) | 开发入口、忽略规则、本地及托管工作区检查已验证。 |
| REQ-29.01 | 保护 main，经 PR 由所有者审核且按序合并 | [GIT_AND_PR](../standards/GIT_AND_PR.md#section-13) | [000](PLAN.md#pr-000), [002](PLAN.md#pr-002) | [16](ACCEPTANCE.md#e2e-16) | 已核验所有者合并 PR 004。两项 GitHub Actions 检查和分支保持最新均已强制，绕过名单为空。[工程记录](../records/STATUS.md) |
| REQ-29.02 | 验证强制门禁、缺陷阻断、最新合并候选和合并后回归 | [GIT_AND_PR](../standards/GIT_AND_PR.md#ag-48) | [002](PLAN.md#pr-002), [050](PLAN.md#pr-050) | [16](ACCEPTANCE.md#e2e-16) | PR 004 通过两项强制检查及三个合并后工作流，ERR-004 已解决。PR 050 仍需执行交付回归。[工程记录](../records/STATUS.md) |
| REQ-29.03 | 完成真实 Windows、安全、性能与端到端验收 | [ACCEPTANCE](../roadmap/ACCEPTANCE.md#section-15) | [049](PLAN.md#pr-049), [050](PLAN.md#pr-050) | [01–16](ACCEPTANCE.md#e2e-01) | 规划中 |
| REQ-30.01 | 维护相互引用的双语规范与可追溯记录 | [DOCUMENTATION](../standards/DOCUMENTATION.md#section-12) | [000](PLAN.md#pr-000) | [16](ACCEPTANCE.md#e2e-16) | 规划中 |
| REQ-30.02 | 逐项追踪需求、25 个插件和 61 项规范及证据 | [DOCUMENTATION](../standards/DOCUMENTATION.md#ag-61) | [000](PLAN.md#pr-000), [050](PLAN.md#pr-050) | [16](ACCEPTANCE.md#e2e-16) | 规划中 |

## 原生插件

全部插件目前为规划中，待获取运行证据。职责见[官方插件契约](../architecture/PRODUCT.md#section-4)。

| ID | 组件 | 计划 PR | 验收 |
|---|---|---|---|
| PL-01 | StrataPulse | [024](PLAN.md#pr-024) | [08](ACCEPTANCE.md#e2e-08) |
| PL-02 | StrataNet | [031](PLAN.md#pr-031) | [04](ACCEPTANCE.md#e2e-04) |
| PL-03 | StrataTrace | [021](PLAN.md#pr-021) | [02](ACCEPTANCE.md#e2e-02), [03](ACCEPTANCE.md#e2e-03) |
| PL-04 | StrataApps | [035](PLAN.md#pr-035) | [03](ACCEPTANCE.md#e2e-03), [09](ACCEPTANCE.md#e2e-09) |
| PL-05 | StrataRegistry | [019](PLAN.md#pr-019) | [02](ACCEPTANCE.md#e2e-02) |
| PL-06 | StrataEnv | [020](PLAN.md#pr-020) | [02](ACCEPTANCE.md#e2e-02) |
| PL-07 | StrataProcess | [018](PLAN.md#pr-018) | [02](ACCEPTANCE.md#e2e-02), [03](ACCEPTANCE.md#e2e-03) |
| PL-08 | StrataServices | [022](PLAN.md#pr-022) | [03](ACCEPTANCE.md#e2e-03), [14](ACCEPTANCE.md#e2e-14) |
| PL-09 | StrataStartup | [023](PLAN.md#pr-023) | [14](ACCEPTANCE.md#e2e-14) |
| PL-10 | StrataSecurity | [037](PLAN.md#pr-037) | [14](ACCEPTANCE.md#e2e-14) |
| PL-11 | StrataSnapshot | [038](PLAN.md#pr-038) | [07](ACCEPTANCE.md#e2e-07), [09](ACCEPTANCE.md#e2e-09) |
| PL-12 | StrataRules | [039](PLAN.md#pr-039) | [07](ACCEPTANCE.md#e2e-07) |
| PL-13 | StrataHealth | [025](PLAN.md#pr-025) | [08](ACCEPTANCE.md#e2e-08), [15](ACCEPTANCE.md#e2e-15) |
| PL-14 | StrataTimeline | [041](PLAN.md#pr-041) | [09](ACCEPTANCE.md#e2e-09) |
| PL-15 | StrataBudget | [026](PLAN.md#pr-026) | [08](ACCEPTANCE.md#e2e-08), [15](ACCEPTANCE.md#e2e-15) |
| PL-16 | StrataMCP | [045](PLAN.md#pr-045) | [12](ACCEPTANCE.md#e2e-12) |
| PL-17 | StrataRescue | [047](PLAN.md#pr-047) | [13](ACCEPTANCE.md#e2e-13) |
| PL-18 | StrataFilter | [028](PLAN.md#pr-028) | [04](ACCEPTANCE.md#e2e-04), [08](ACCEPTANCE.md#e2e-08) |
| PL-19 | StrataDisk | [032](PLAN.md#pr-032) | [08](ACCEPTANCE.md#e2e-08), [15](ACCEPTANCE.md#e2e-15) |
| PL-20 | StrataScript | [027](PLAN.md#pr-027) | [07](ACCEPTANCE.md#e2e-07), [12](ACCEPTANCE.md#e2e-12) |
| PL-21 | StrataDependency | [034](PLAN.md#pr-034) | [03](ACCEPTANCE.md#e2e-03), [09](ACCEPTANCE.md#e2e-09) |
| PL-22 | StrataTrust | [033](PLAN.md#pr-033) | [03](ACCEPTANCE.md#e2e-03), [09](ACCEPTANCE.md#e2e-09) |
| PL-23 | StrataUpdate | [036](PLAN.md#pr-036) | [14](ACCEPTANCE.md#e2e-14) |
| PL-24 | StrataTag | [029](PLAN.md#pr-029) | [04](ACCEPTANCE.md#e2e-04), [05](ACCEPTANCE.md#e2e-05) |
| PL-25 | StrataStorage | [030](PLAN.md#pr-030) | [05](ACCEPTANCE.md#e2e-05) |

## 61 项 Agent 规范

每条独立编号和权威位置见 [AG-01 至 AG-61 索引](../AGENTS.md)。PR [000](PLAN.md#pr-000) 核对全量条款及双语映射。后续 PR 按 [E2E-16](ACCEPTANCE.md#e2e-16) 逐项核查实际证据并维护规范。
