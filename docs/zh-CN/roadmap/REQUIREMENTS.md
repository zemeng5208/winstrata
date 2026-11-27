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
| REQ-03.01 | GUI 与 Core 普通权限，受控服务与内核权限分离 | [SYSTEM](../architecture/SYSTEM.md#section-3) | [008](PLAN.md#pr-008) | [14](ACCEPTANCE.md#e2e-14), [15](ACCEPTANCE.md#e2e-15) | 已实现[服务权限分离](../architecture/SERVICE-KERNEL.md)。Linux 不能订阅，`GuiClient` 没有变更接口。Windows Server 2022 上，受限令牌被拒绝 SCM 创建权限和服务管道。E2E-14 与 E2E-15 未完成。 |
| REQ-03.02 | 版本化 IPC、验证身份与参数并审计 | [SYSTEM](../architecture/SYSTEM.md#section-3) | [005](PLAN.md#pr-005), [008](PLAN.md#pr-008) | [14](ACCEPTANCE.md#e2e-14), [15](ACCEPTANCE.md#e2e-15) | [版本与错误契约](../architecture/PROTOCOLS.md)仍然有效。[服务 IPC](../architecture/SERVICE-KERNEL.md)已在 Linux 和 Windows 命名管道上检查对端身份、角色、参数和审计。E2E-14 与 E2E-15 仍待完成。 |
| REQ-03.03 | 官方接口优先的混合采集与驱动独立按需生命周期 | [SYSTEM](../architecture/SYSTEM.md#section-3) | [008–011](PLAN.md#pr-008) | [02](ACCEPTANCE.md#e2e-02), [04](ACCEPTANCE.md#e2e-04), [14](ACCEPTANCE.md#e2e-14), [15](ACCEPTANCE.md#e2e-15) | [订阅管理](../architecture/SERVICE-KERNEL.md)跟踪独立引用计数和被限制的故障。由于没有已签名映像，目录宿主拒绝全部模块。[文件与注册表采集](../architecture/FILE-REGISTRY.md)在订阅时为单个目录和单个 HKCU 子键启动用户态监视，并在订阅归零时停止。[进程采样](../architecture/PROCESS.md)使用 procfs 或 Toolhelp，不加载 `strata-proc`。用户态网络观测属于 [PR 011](../architecture/NETWORK-SOURCES.md)。尚未加载或卸载已签名驱动。 |
| REQ-03.04 | 签名、Secure Boot 和 HVCI 保持启用 | [SECURITY](../standards/SECURITY.md#ag-47) | [008](PLAN.md#pr-008) | [14](ACCEPTANCE.md#e2e-14), [15](ACCEPTANCE.md#e2e-15) | 计划 008 不关闭签名、Secure Boot 或 HVCI，也不加载未签名映像。尚未在这些机制启用时加载已签名的 INF/CAT。 |
| REQ-04.01 | 当前用户、相关公共活动与用户选择的存储范围 | [PRODUCT](../architecture/PRODUCT.md#section-2) | [006](PLAN.md#pr-006), [017](PLAN.md#pr-017) | [01](ACCEPTANCE.md#e2e-01), [02](ACCEPTANCE.md#e2e-02) | [已实现当前用户身份、私有目录和监视范围配置](../architecture/CORE-STORAGE.md)。计划 017 在发布前过滤路径、扩展名、进程、软件、注册表键和接口。其他用户的私有配置目录会被排除。当前用户自己的配置目录即使被写成 8.3 短路径，也仍纳入范围。被拒绝的路径不会写入错误。E2E-01 和 E2E-02 未完成。 |
| REQ-04.02 | 初始化选择启用，默认手动启动且不开机自启 | [FRONTEND](../architecture/FRONTEND.md#section-9) | [017](PLAN.md#pr-017) | [01](ACCEPTANCE.md#e2e-01), [11](ACCEPTANCE.md#e2e-11) | 官方外壳记录语言、调用方给出的位置、插件启用选择、范围、保留期限、登录启动偏好、可选加密、MCP 和基线。位置和范围不能跳过。除非用户选择，登录启动保持关闭。界面会话不会改选其他磁盘。没有向操作系统注册启动项。E2E-01 和 E2E-11 未完成。 |
| REQ-04.03 | 建立选定范围基线并标注重建历史 | [PRODUCT](../architecture/PRODUCT.md#section-4) | [017](PLAN.md#pr-017), [021](PLAN.md#pr-021) | [01](ACCEPTANCE.md#e2e-01), [02](ACCEPTANCE.md#e2e-02) | 基线遍历所选根目录，跳过符号链接，并以状态 `reconstructed` 保存文件元数据。没有匹配文件时仍保存对象，并把文件数记为 0。软件、注册表、环境、服务、驱动和任务记为 `not_scanned`。自动快照保持关闭。更完整的历史查询仍属于计划 021。E2E-01 和 E2E-02 未完成。 |
| REQ-04.04 | 实时与历史切换、暂停恢复、缺失记录和安全退出 | [FRONTEND](../architecture/FRONTEND.md#section-9) | [017](PLAN.md#pr-017) | [11](ACCEPTANCE.md#e2e-11) | `RunState` 仍是 `Stopped`、`Live` 或 `History`。暂停是单独的标志。暂停、历史和停止写入不同的活动。丢失记录保持 `GapReason::Lost`。历史和停止只在驱动已有订阅时要求服务释放。暂停不会。Windows 宿主在退出前发送停止。Linux 覆盖了这些标记。没有运行 Windows GUI 会话。E2E-11 未完成。 |
| REQ-05.01 | 事件稳定标识和版本、时间顺序、来源与完整性 | [SYSTEM](../architecture/SYSTEM.md#section-3) | [005](PLAN.md#pr-005) | [02](ACCEPTANCE.md#e2e-02), [15](ACCEPTANCE.md#e2e-15) | [事件标识、排序、来源与缺口契约](../architecture/PROTOCOLS.md)已通过真实 Node 子进程、Windows 日志、兼容用例与确定性回放验证；产品规模的多来源对齐留待集成场景。 |
| REQ-05.02 | 有界批处理、队列、背压、吞吐及丢失记录 | [SYSTEM](../architecture/SYSTEM.md#section-3) | [007](PLAN.md#pr-007) | [08](ACCEPTANCE.md#e2e-08), [15](ACCEPTANCE.md#e2e-15) | [Core 运行时](../architecture/CORE-RUNTIME.md)已限制队列和批量，在记录丢失前等待，并发布协议缺口。Linux release 在 39.925 毫秒内传送 20,000 条事件。E2E-08 与 E2E-15 仍待完成。 |
| REQ-05.03 | 统一优先级、超时、取消和资源保护 | [ARCHITECTURE](../standards/ARCHITECTURE.md#ag-45) | [007](PLAN.md#pr-007), [025](PLAN.md#pr-025), [026](PLAN.md#pr-026) | [08](ACCEPTANCE.md#e2e-08), [15](ACCEPTANCE.md#e2e-15) | 调度器已实现优先级、超时、取消和固定的工作者与队列上限。动态预算和健康诊断仍属 PR 025 与 PR 026。E2E-08 与 E2E-15 仍待完成。 |
| REQ-06.01 | 文件操作、所有权与权限；不存文件内容版本 | [PRODUCT](../architecture/PRODUCT.md#section-4) | [009](PLAN.md#pr-009), [021](PLAN.md#pr-021) | [02](ACCEPTANCE.md#e2e-02) | [文件与注册表采集](../architecture/FILE-REGISTRY.md)报告单个目录的创建、修改、重命名和删除，以及单个 HKCU 子键的值新增、修改和删除。不读取文件字节。所有者和 SDDL 仅在能放入标识符时写入。这些 API 不报告变更时刻的精度，因此省略 `precision_ns`。进程归因属于计划 010。插件呈现仍属于 PR 021。E2E-02 尚未完成。 |
| REQ-06.02 | 观测进程、线程、模块、来源及 PID 生命周期 | [PRODUCT](../architecture/PRODUCT.md#section-4) | [010](PLAN.md#pr-010), [018](PLAN.md#pr-018), [052](PLAN.md#pr-052) | [02](ACCEPTANCE.md#e2e-02), [03](ACCEPTANCE.md#e2e-03) | [进程来源](../architecture/PROCESS.md)采样线程、模块和父子链接。Linux 观察到真实子进程的生命周期并完成去重。Windows Server 2022 在计划 010 和计划 013 的送审头上通过了这些测试。main `a2471d5` 随后有一次 `real_process_tree_pid_lifecycle_and_dedup` 失败；同一文件树在几分钟前通过。计划 052 等到完整采样，并且仍然要求这次退出。合并提交 `004974215e239f96160b47d3eef40a84c316e802` 的 Windows 工作区检查通过。持久编号是 [ERR-005](../records/modules/core.md#err-005)。ERR-013 是同一次缺陷。计划 007 的线程释放失败是 ERR-016。这台 Linux 主机没有服务单元。计划 018 在 `strata.process` 中运行该采样器，记录同样的协议事件和父子树，并在禁用时结束采样线程。已签名 `strata-proc` 加载、ETW、命令行、服务停止记录、Windows 10/11 客户端、E2E-02 和 E2E-03 仍待完成。 |
| REQ-06.03 | 归因注册表与环境变量并呈现前后值 | [PRODUCT](../architecture/PRODUCT.md#section-4) | [019](PLAN.md#pr-019), [020](PLAN.md#pr-020) | [02](ACCEPTANCE.md#e2e-02) | [注册表插件](../architecture/REGISTRY.md)记录当前用户和本机的前后值。[环境插件](../architecture/ENV.md)记录当前用户和系统环境以及 PATH 的前后值；完整 PATH 放不进标识符时记录变化的段。Linux 启动返回 `unsupported`，不发布事件。送审头 `579f85d63b338e4502f95abdbd122c3a815e2902` 的 Windows Server 2022 工作区作业通过了 `environment_is_in_process_and_linux_does_not_publish`。E2E-02 未完成。 |
| REQ-06.04 | 观测服务、驱动、启动项与计划任务变化 | [PRODUCT](../architecture/PRODUCT.md#section-4) | [022](PLAN.md#pr-022), [023](PLAN.md#pr-023) | [03](ACCEPTANCE.md#e2e-03), [14](ACCEPTANCE.md#e2e-14) | 规划中 |
| REQ-07.01 | 识别自动与手动的软件生命周期会话 | [PRODUCT](../architecture/PRODUCT.md#section-4) | [035](PLAN.md#pr-035) | [03](ACCEPTANCE.md#e2e-03) | 规划中 |
| REQ-07.02 | 归因软件档案、残留依据及磁盘占用 | [PRODUCT](../architecture/PRODUCT.md#section-4) | [035](PLAN.md#pr-035) | [03](ACCEPTANCE.md#e2e-03), [09](ACCEPTANCE.md#e2e-09) | 规划中 |
| REQ-07.03 | 追溯真实依赖、签名、完整性和来源 | [PRODUCT](../architecture/PRODUCT.md#section-4) | [033](PLAN.md#pr-033), [034](PLAN.md#pr-034) | [03](ACCEPTANCE.md#e2e-03), [09](ACCEPTANCE.md#e2e-09) | 规划中 |
| REQ-08.01 | 展示受支持的 CPU、GPU、内存、电源与传感器历史 | [PRODUCT](../architecture/PRODUCT.md#section-4) | [024](PLAN.md#pr-024) | [08](ACCEPTANCE.md#e2e-08) | 规划中 |
| REQ-08.02 | 诊断和调节资源，保留可追溯降级记录 | [DATA](../architecture/DATA.md#section-8) | [025](PLAN.md#pr-025), [026](PLAN.md#pr-026) | [08](ACCEPTANCE.md#e2e-08), [15](ACCEPTANCE.md#e2e-15) | 规划中 |
| REQ-08.03 | 计量磁盘健康、逻辑及物理写入和 I/O | [DATA](../architecture/DATA.md#section-8) | [032](PLAN.md#pr-032), [048](PLAN.md#pr-048) | [08](ACCEPTANCE.md#e2e-08), [15](ACCEPTANCE.md#e2e-15) | 规划中 |
| REQ-09.01 | 观测可用接口并归因入站和出站连接 | [DATA](../architecture/DATA.md#section-7) | [011](PLAN.md#pr-011), [031](PLAN.md#pr-031) | [04](ACCEPTANCE.md#e2e-04) | [网络源](../architecture/NETWORK-SOURCES.md)在用户态观测接口和 TCP/UDP 方向。只有采样到的 TLS 记录才记录加密，否则快照记录采样缺口。原始包留在内存，除非一个合格标记点名一个报文。拓扑、发现和 PCAPNG 导出仍属于 PR 031。E2E-04 未完成。Windows Server 2022 运行了现场交换。环回流量对原始套接字不可见，因此没有在那里观察到加密。没有运行 Windows 10/11。 |
| REQ-09.02 | 授权发现、拓扑与可见性边界 | [DATA](../architecture/DATA.md#section-7) | [031](PLAN.md#pr-031) | [04](ACCEPTANCE.md#e2e-04) | 规划中 |
| REQ-10.01 | 记录风险标记来源、依据、时间与可信度 | [DATA](../architecture/DATA.md#section-7) | [029](PLAN.md#pr-029) | [04](ACCEPTANCE.md#e2e-04) | 规划中 |
| REQ-10.02 | 未标记原始数据包从采集到释放均不落盘 | [DATA](../architecture/DATA.md#section-7) | [031](PLAN.md#pr-031) | [04](ACCEPTANCE.md#e2e-04), [05](ACCEPTANCE.md#e2e-05) | 规划中 |
| REQ-10.03 | 配置有界抓包缓冲并导出风险抓包 | [DATA](../architecture/DATA.md#section-7) | [031](PLAN.md#pr-031) | [04](ACCEPTANCE.md#e2e-04) | 规划中 |
| REQ-11.01 | 默认 D 盘、手动回退与安全迁移 | [DATA](../architecture/DATA.md#section-8) | [006](PLAN.md#pr-006) | [01](ACCEPTANCE.md#e2e-01), [10](ACCEPTANCE.md#e2e-10) | [Core 选择存储、在失卷后暂停写入并校验文件迁移](../architecture/CORE-STORAGE.md)。Windows 客户端界面按 PR 016 验证。 |
| REQ-11.02 | 支持结构化、分段、时序、抓包和快照存储 | [DATA](../architecture/DATA.md#section-8) | [012](PLAN.md#pr-012) | [05](ACCEPTANCE.md#e2e-05), [10](ACCEPTANCE.md#e2e-10) | [Core 混合存储](../architecture/CORE-STORAGE.md)已在 Linux 测试写入、查询、分段、索引、容量和损坏回退。E2E-05、E2E-10 和本机 Windows 测试未完成。 |
| REQ-11.03 | 共享标识、唯一源记录与统一索引 | [ARCHITECTURE](../standards/ARCHITECTURE.md#ag-44) | [012](PLAN.md#pr-012), [040](PLAN.md#pr-040) | [09](ACCEPTANCE.md#e2e-09) | 同一标识可以查到对象、关系和事件主体，载荷只保存一份。跨插件搜索仍属 PR 040。E2E-09 未完成。 |
| REQ-12.01 | 合格数据默认永久；可配置保留期限 | [DATA](../architecture/DATA.md#section-8) | [012](PLAN.md#pr-012), [030](PLAN.md#pr-030) | [05](ACCEPTANCE.md#e2e-05) | 默认永久保留。各类天数会保存并在重新打开后读回，本计划不删除数据。确认和清理仍属 PR 030。 |
| REQ-12.02 | 抓包 1 GB、日志 2 GB 提醒；持续记录 1 GB 和 500 MB 按阶重复 | [DATA](../architecture/DATA.md#section-8) | [030](PLAN.md#pr-030) | [05](ACCEPTANCE.md#e2e-05) | 规划中 |
| REQ-12.03 | 清理须用户确认，删除后同步索引 | [DATA](../architecture/DATA.md#section-8) | [030](PLAN.md#pr-030) | [05](ACCEPTANCE.md#e2e-05) | 规划中 |
| REQ-12.04 | 保留 Core 告警，标记存储不可用，禁止清理外部数据 | [DATA](../architecture/DATA.md#section-8) | [030](PLAN.md#pr-030) | [05](ACCEPTANCE.md#e2e-05), [15](ACCEPTANCE.md#e2e-15) | 规划中 |
| REQ-13.01 | 避免自监视递归，保留相关外部修改 | [DATA](../architecture/DATA.md#section-8) | [028](PLAN.md#pr-028) | [05](ACCEPTANCE.md#e2e-05), [08](ACCEPTANCE.md#e2e-08) | 规划中 |
| REQ-13.02 | GUI 与批处理脚本过滤，失败安全回退 | [DATA](../architecture/DATA.md#section-8) | [027](PLAN.md#pr-027), [028](PLAN.md#pr-028) | [04](ACCEPTANCE.md#e2e-04), [07](ACCEPTANCE.md#e2e-07) | 规划中 |
| REQ-13.03 | 减少无效 I/O 和写入放大，验证完整性 | [DATA](../architecture/DATA.md#section-8) | [032](PLAN.md#pr-032), [048](PLAN.md#pr-048) | [08](ACCEPTANCE.md#e2e-08), [15](ACCEPTANCE.md#e2e-15) | 规划中 |
| REQ-14.01 | 可选本地加密及锁定，锁定时保持监视 | [DATA](../architecture/DATA.md#section-8) | [013](PLAN.md#pr-013), [042](PLAN.md#pr-042) | [10](ACCEPTANCE.md#e2e-10) | 加密在启用前保持关闭。锁定挡住读取，实时提交可以继续。界面锁定显示仍属于 PR 042。密钥轮换不是崩溃安全的，也不标成已验收。见 [OI-008](../records/OPEN_ITEMS.md#oi-008)。E2E-10 未完成。 |
| REQ-14.02 | 验证备份、恢复、完整性、迁移及设备标识 | [DATA](../architecture/DATA.md#section-8) | [013](PLAN.md#pr-013), [038](PLAN.md#pr-038) | [10](ACCEPTANCE.md#e2e-10) | Linux 测试覆盖完整包和增量包、校验、路径穿越、用户不匹配、恢复、设备标识，以及保留源目录的迁移。没有运行第二台设备。插件导出和按时间范围导出仍属于 PR 038。E2E-10 未完成。 |
| REQ-14.03 | 授权并验证导入导出，明文转换须确认 | [DATA](../architecture/DATA.md#section-8) | [038](PLAN.md#pr-038), [042](PLAN.md#pr-042) | [09](ACCEPTANCE.md#e2e-09), [10](ACCEPTANCE.md#e2e-10) | 规划中 |
| REQ-15.01 | 手动自选范围的完整及增量快照与差异视图 | [PRODUCT](../architecture/PRODUCT.md#section-4) | [038](PLAN.md#pr-038) | [07](ACCEPTANCE.md#e2e-07) | 规划中 |
| REQ-15.02 | 自动快照默认关闭，支持用户定义自动化 | [EXTENSIONS](../architecture/EXTENSIONS.md#section-5) | [027](PLAN.md#pr-027), [038](PLAN.md#pr-038) | [07](ACCEPTANCE.md#e2e-07) | 规划中 |
| REQ-16.01 | 统一 Python、TS、JS 脚本工作台、调试、触发和历史 | [EXTENSIONS](../architecture/EXTENSIONS.md#section-5) | [027](PLAN.md#pr-027) | [07](ACCEPTANCE.md#e2e-07), [12](ACCEPTANCE.md#e2e-12) | 规划中 |
| REQ-16.02 | 只读运行可视化规则、阈值与通知 | [PRODUCT](../architecture/PRODUCT.md#section-4) | [039](PLAN.md#pr-039) | [07](ACCEPTANCE.md#e2e-07) | 规划中 |
| REQ-16.03 | 执行脚本权限及依赖检查且不内置预设 | [EXTENSIONS](../architecture/EXTENSIONS.md#section-5) | [015](PLAN.md#pr-015), [027](PLAN.md#pr-027) | [07](ACCEPTANCE.md#e2e-07), [12](ACCEPTANCE.md#e2e-12) | 语言包声明权限和项目模块。SDK 目录没有用户脚本预设。编辑器、触发器和计划 027 的空预设检查仍在后续。E2E-07 和 E2E-12 未完成。 |
| REQ-17.01 | 只读展示系统更新、KB、驱动、功能、重启及错误 | [PRODUCT](../architecture/PRODUCT.md#section-4) | [036](PLAN.md#pr-036) | [14](ACCEPTANCE.md#e2e-14) | 规划中 |
| REQ-17.02 | 只读展示可观测安全与证书状态 | [PRODUCT](../architecture/PRODUCT.md#section-4) | [037](PLAN.md#pr-037) | [14](ACCEPTANCE.md#e2e-14) | 规划中 |
| REQ-18.01 | 检索和关联授权来源，并追溯对象证据 | [FRONTEND](../architecture/FRONTEND.md#section-9) | [040](PLAN.md#pr-040) | [09](ACCEPTANCE.md#e2e-09) | 规划中 |
| REQ-18.02 | 多轨时间线、仅供分析的回放及区间导出 | [PRODUCT](../architecture/PRODUCT.md#section-4) | [041](PLAN.md#pr-041) | [09](ACCEPTANCE.md#e2e-09) | 规划中 |
| REQ-19.01 | 验证指定 UI 来源，按许可小幅适配 shadcn-admin | [FRONTEND](../architecture/FRONTEND.md#section-9) | [004](PLAN.md#pr-004), [016](PLAN.md#pr-016) | [11](ACCEPTANCE.md#e2e-11) | 计划 016 按已记录修订入库 shadcn-admin 布局、Glasswork 的 `lens.js` 和 `glass.css`、Amicro 的 `fade-in.tsx`，以及 UI Moments 的原创 Ink Switch 和 Signal Return。四个 shadcn 文件接受侧栏、标题或文案。未纳入 Fluent UI 和 Astryx 示例。见[来源登记](../records/GUI_SOURCES.md)。E2E-11 未完成。 |
| REQ-19.02 | 三套共享主题可运行时切换并持久保存 | [FRONTEND](../architecture/FRONTEND.md#section-9) | [016](PLAN.md#pr-016) | [11](ACCEPTANCE.md#e2e-11) | 共用外壳把白色亚克力、黑色亚克力和液态玻璃映射到 shadcn 令牌和 Glasswork 材质。选择写入 `gui.json`。用户选择之前不保存。[OI-023](../records/OPEN_ITEMS.md#oi-023) 仍开放。没有运行 Windows WebView2 会话。E2E-11 未完成。 |
| REQ-19.03 | 完整双语与 Windows WebView2 可读的真实交互 | [FRONTEND](../architecture/FRONTEND.md#section-9) | [016](PLAN.md#pr-016), [042](PLAN.md#pr-042) | [11](ACCEPTANCE.md#e2e-11) | 英文和 zh-CN 共用页面。未设置语言时使用英文。没有运行 WebView2 可读性检查。E2E-11 未完成。 |
| REQ-19.04 | 持久化单首页布局并接入动态插件界面 | [FRONTEND](../architecture/FRONTEND.md#section-9) | [042](PLAN.md#pr-042) | [11](ACCEPTANCE.md#e2e-11) | 计划 016 展示空仪表盘，不编造图表。拖拽、持久化和插件小部件仍属于计划 042。 |
| REQ-19.05 | 提供托盘继续、完整退出和真实界面状态 | [FRONTEND](../architecture/FRONTEND.md#section-9) | [016](PLAN.md#pr-016), [017](PLAN.md#pr-017) | [01](ACCEPTANCE.md#e2e-01), [11](ACCEPTANCE.md#e2e-11) | 关闭偏好 `ask`、`tray` 或 `exit` 会被保存。未设置表示询问。退出会在窗口关闭前要求当前会话停止实时采集。留在托盘不会。实时、历史、暂停和继续位于官方外壳，并在 Linux 环回桥上执行过。没有运行 Windows 托盘和 WebView2。E2E-11 未完成。 |
| REQ-19.06 | 官方默认 GUI 完整内置、不可卸载，并作为默认界面和恢复界面 | [AG-64](../AGENTS.md#ag-64) | [053](PLAN.md#pr-053) 记录规则。[016](PLAN.md#pr-016) 只负责内置默认界面。替换之后的恢复没有实现计划编号 | [11](ACCEPTANCE.md#e2e-11), [17](ACCEPTANCE.md#e2e-17) | `uninstall_official_gui` 返回 `builtin_retained`。官方页面保持安装。第三方替换之后的恢复没有计划编号。E2E-11 未完成。E2E-17 没有实现计划编号。 |
| REQ-19.07 | 插件可以修改全局主题、颜色、材质、字体、动效和交互方式 | [AG-64](../AGENTS.md#ag-64) | 无路线图行 | [17](ACCEPTANCE.md#e2e-17) | 未实现。不是计划 016 的验收。 |
| REQ-19.08 | 插件可以修改全局布局、导航栏、侧边栏、顶部区域和仪表盘 | [AG-64](../AGENTS.md#ag-64) | 无路线图行 | [17](ACCEPTANCE.md#e2e-17) | 未实现。不是计划 016 的验收。 |
| REQ-19.09 | 插件可以替换共享 UI 组件及其他插件的界面呈现 | [AG-64](../AGENTS.md#ag-64) | 无路线图行 | [17](ACCEPTANCE.md#e2e-17) | 未实现。不是计划 016 的验收。 |
| REQ-19.10 | 第三方插件可以替换整个 GUI，并支持安装、启用、禁用、更新、卸载和恢复 | [AG-64](../AGENTS.md#ag-64) | 无路线图行。现有市场仍是 [043](PLAN.md#pr-043)，它本身不验收这项能力 | [17](ACCEPTANCE.md#e2e-17) | 未实现。不是计划 016 的验收。 |
| REQ-19.11 | 经过授权的能力、数据和事件只通过统一前端 SDK/API 获取 | [AG-64](../AGENTS.md#ag-64) | 无路线图行。这不是 [015](PLAN.md#pr-015) 的语言 SDK | [17](ACCEPTANCE.md#e2e-17) | 未实现。没有选定技术栈。计划 016 的本机桥只是官方页面的命令白名单。它不是这个 SDK。 |
| REQ-19.12 | GUI 解耦，更换它不中断监视、不重启无关服务、不丢失数据 | [AG-64](../AGENTS.md#ag-64) | [016](PLAN.md#pr-016) 不得把官方外壳绑死。替换证明没有路线图行 | [17](ACCEPTANCE.md#e2e-17) | GUI 进程不持有 core 会话锁，不订阅驱动，关闭窗口时也不停止服务。没有运行第二套 GUI。E2E-17 不是这次验收。 |
| REQ-19.13 | 权限、隔离、救援授权和删除确认由可信宿主执行 | [AG-64](../AGENTS.md#ag-64) | 无路线图行。救援仍是 [047](PLAN.md#pr-047)，删除确认仍是 [030](PLAN.md#pr-030) | [18](ACCEPTANCE.md#e2e-18) | 未实现。第三方 GUI 不得伪造这些授权。 |
| REQ-19.14 | 前端插件只由现有市场管理，同一时间只运行一套完整 GUI 渲染环境 | [AG-64](../AGENTS.md#ag-64) | 市场仍是 [043](PLAN.md#pr-043)。单一渲染环境的证明没有路线图行 | [17](ACCEPTANCE.md#e2e-17) | 未实现。 |
| REQ-19.15 | 官方 GUI 和官方插件的每个组件和布局都写入来源登记 | [AG-64](../AGENTS.md#ag-64) | 不新增路线图行。本变更建立登记。产品行随文件一起进入 | [19](ACCEPTANCE.md#e2e-19) | 本提交的登记和 `packages/ui/upstream.json` 与入库文件一致。UI Moments 是缺口。E2E-19 没有实现计划编号，也没有关闭。见 [GUI_SOURCES.md](../records/GUI_SOURCES.md)。 |
| REQ-19.16 | 实现可替换 GUI 之前，先调研、验证并取得所有者对技术路线的批准 | [AG-64](../AGENTS.md#ag-64) | 无路线图行 | [方案](ACCEPTANCE.md#gui-proposal) | 未开始。本次变更不选定替换用的技术栈。官方外壳用计划 004 的 esbuild 方法，从已批准来源自己的锁文件打包。[OI-038](../records/OPEN_ITEMS.md#oi-038) 仍开放。 |
| REQ-20.01 | 随附 25 个插件，用户选择启用，内置项不可卸载 | [PRODUCT](../architecture/PRODUCT.md#section-4) | [014](PLAN.md#pr-014), [017](PLAN.md#pr-017) | [06](ACCEPTANCE.md#e2e-06), [11](ACCEPTANCE.md#e2e-11) | [插件宿主](../architecture/PLUGINS.md)保留 25 个官方身份并拒绝卸载。计划 017 把启用选择写入 `onboarding.json`，不调用 `enable`，也不启动采集器。救援不进入该批量选择。页面命令 `enable` 仍然被拒绝。计划 018 提供进程采集实现，并在启用前保持禁用。`strata.registry` 是计划 019 的进程内监视。`strata.env` 是计划 020 的进程内监视。其余采集实现仍属于计划 021–047。E2E-06 和 E2E-11 未完成。 |
| REQ-20.02 | 独立生命周期、资源释放与兼容替换 | [ARCHITECTURE](../standards/ARCHITECTURE.md#ag-43) | [014](PLAN.md#pr-014) | [06](ACCEPTANCE.md#e2e-06) | 原生第三方插件在 Linux 上可以加载、启用、暂停、恢复、停止、禁用和卸载。历史保留，该插件的总线订阅被移除。没有运行界面和 Windows 重启行为。E2E-06 未完成。 |
| REQ-20.03 | 验证动态升级、依赖状态与故障隔离 | [EXTENSIONS](../architecture/EXTENSIONS.md#section-5) | [014](PLAN.md#pr-014), [044](PLAN.md#pr-044) | [06](ACCEPTANCE.md#e2e-06), [15](ACCEPTANCE.md#e2e-15) | 拒绝启动或无效的更新会留下正在运行的包。依赖故障不会停止另一个插件进程。恢复使用最后一份健康的包副本。签名和事务回退仍属于计划 044。E2E-06 和 E2E-15 未完成。 |
| REQ-21.01 | 能力声明与最小权限管理、撤销和审计 | [EXTENSIONS](../architecture/EXTENSIONS.md#section-5) | [014](PLAN.md#pr-014), [015](PLAN.md#pr-015) | [06](ACCEPTANCE.md#e2e-06), [07](ACCEPTANCE.md#e2e-07) | 清单列出权限。安装时授予这些权限。更新新增的权限留在待授权。对正在运行的 Python 插件撤销 `event.subscribe` 会立刻移除总线订阅。语言 SDK 使用同一套帧检查。E2E-06 和 E2E-07 未完成。 |
| REQ-21.02 | 按需且隔离的 Python、TS/JS 运行时及依赖环境 | [EXTENSIONS](../architecture/EXTENSIONS.md#section-5) | [004](PLAN.md#pr-004), [015](PLAN.md#pr-015) | [06](ACCEPTANCE.md#e2e-06), [07](ACCEPTANCE.md#e2e-07), [12](ACCEPTANCE.md#e2e-12) | Linux 只在项目版本声明匹配后启动真实 Python venv 和 Node 24.22.3。每个插件看到自己的模块。目录模块只对声明了它的插件可见。禁用会删除 venv 和进程。撰写主机没有运行 Windows 容器测试。E2E-06、E2E-07 和 E2E-12 未完成。 |
| REQ-21.03 | 提供指定语言 SDK 并验证操作系统级隔离 | [EXTENSIONS](../architecture/EXTENSIONS.md#section-5) | [015](PLAN.md#pr-015) | [06](ACCEPTANCE.md#e2e-06), [12](ACCEPTANCE.md#e2e-12) | Python、JavaScript 和 TypeScript SDK 在 `sdk/`。Linux 上的真实脚本被拒绝读取规则集之外的文件、打开套接字、启动新程序和使用继承的描述符。状态目录内已经打开的句柄仍然可读。Windows 测试已写入，撰写主机没有执行。E2E-06 和 E2E-12 未完成。 |
| REQ-22.01 | 本地优先统一市场，含官方、社区、已安装和更新管理 | [EXTENSIONS](../architecture/EXTENSIONS.md#section-5) | [043](PLAN.md#pr-043) | [06](ACCEPTANCE.md#e2e-06), [12](ACCEPTANCE.md#e2e-12) | 规划中 |
| REQ-22.02 | 安装复合包并管理来源、警告和校验 | [EXTENSIONS](../architecture/EXTENSIONS.md#section-5) | [043](PLAN.md#pr-043) | [06](ACCEPTANCE.md#e2e-06) | 规划中 |
| REQ-22.03 | 自由提交与自动检查，正式发布须明确同意 | [EXTENSIONS](../architecture/EXTENSIONS.md#section-5) | [043](PLAN.md#pr-043), [046](PLAN.md#pr-046) | [06](ACCEPTANCE.md#e2e-06), [12](ACCEPTANCE.md#e2e-12) | 规划中 |
| REQ-23.01 | 核心组件和扩展分别版本化并可锁定 | [EXTENSIONS](../architecture/EXTENSIONS.md#section-5) | [014](PLAN.md#pr-014), [044](PLAN.md#pr-044) | [06](ACCEPTANCE.md#e2e-06), [15](ACCEPTANCE.md#e2e-15) | 每个插件有自己的版本和可选锁定。锁定必须等于已安装版本，并挡住其他更新，直到解除锁定。Core、服务和驱动签名仍属于计划 044。E2E-06 和 E2E-15 未完成。 |
| REQ-23.02 | 验证签名更新、回退并保留配置与历史 | [EXTENSIONS](../architecture/EXTENSIONS.md#section-5) | [044](PLAN.md#pr-044) | [06](ACCEPTANCE.md#e2e-06), [15](ACCEPTANCE.md#e2e-15) | 规划中 |
| REQ-24.01 | 认证本机 MCP，动态注册和撤销授权能力 | [EXTENSIONS](../architecture/EXTENSIONS.md#section-6) | [045](PLAN.md#pr-045) | [12](ACCEPTANCE.md#e2e-12) | 规划中 |
| REQ-24.02 | 按资源预算与审计提供授权实时、历史查询 | [EXTENSIONS](../architecture/EXTENSIONS.md#section-6) | [045](PLAN.md#pr-045) | [12](ACCEPTANCE.md#e2e-12) | 规划中 |
| REQ-24.03 | 通过本机 MCP 开发、验证、安装及发布插件与脚本 | [EXTENSIONS](../architecture/EXTENSIONS.md#section-6) | [046](PLAN.md#pr-046) | [12](ACCEPTANCE.md#e2e-12) | 规划中 |
| REQ-25.01 | 救援默认关闭并排除批量启用 | [EXTENSIONS](../architecture/EXTENSIONS.md#section-6) | [047](PLAN.md#pr-047) | [13](ACCEPTANCE.md#e2e-13) | 规划中 |
| REQ-25.02 | 仅许可列明的本机救援动作，校验目标并审计 | [EXTENSIONS](../architecture/EXTENSIONS.md#section-6) | [047](PLAN.md#pr-047) | [13](ACCEPTANCE.md#e2e-13) | 规划中 |
| REQ-25.03 | 拒绝任意命令、越权目标及撤销后操作 | [EXTENSIONS](../architecture/EXTENSIONS.md#section-6) | [047](PLAN.md#pr-047) | [13](ACCEPTANCE.md#e2e-13) | 规划中 |
| REQ-26.01 | 监视数据、遥测、文件和哈希留在本机 | [PRODUCT](../architecture/PRODUCT.md#section-2) | [013](PLAN.md#pr-013), [044](PLAN.md#pr-044), [045](PLAN.md#pr-045), [046](PLAN.md#pr-046) | [04](ACCEPTANCE.md#e2e-04), [06](ACCEPTANCE.md#e2e-06), [10](ACCEPTANCE.md#e2e-10), [12](ACCEPTANCE.md#e2e-12) | 计划 013 把保险库、数据密钥和备份包留在本机。没有增加网络导出。计划 044、045 和 046 仍待完成。E2E-10 未完成。 |
| REQ-26.02 | 展示真实来源、不确定性、缺失及不支持状态 | [DEVELOPMENT](../standards/DEVELOPMENT.md#ag-34) | [014](PLAN.md#pr-014), [040](PLAN.md#pr-040), [045](PLAN.md#pr-045) | [06](ACCEPTANCE.md#e2e-06), [09](ACCEPTANCE.md#e2e-09), [12](ACCEPTANCE.md#e2e-12) | 23 个官方插件报告 `implementation_absent`。`strata.registry` 和 `strata.env` 报告自己的阶段，Linux 启动是 `unsupported`。无法识别的运行时是 `unsupported_runtime`。绑定项目工具链后接受 Python、JavaScript 和 TypeScript。拒绝启动是 `unavailable`。缺少依赖是 `dependency_missing`。搜索和 MCP 展示仍属于计划 040 和 045。E2E-06、E2E-09 和 E2E-12 未完成。 |
| REQ-26.03 | 外部 Agent 的决策、模型和凭据边界 | [EXTENSIONS](../architecture/EXTENSIONS.md#section-6) | [043](PLAN.md#pr-043), [045](PLAN.md#pr-045), [046](PLAN.md#pr-046) | [06](ACCEPTANCE.md#e2e-06), [12](ACCEPTANCE.md#e2e-12) | 规划中 |
| REQ-27.01 | 实测后台 150 MB 内存及 1% CPU 目标 | [ACCEPTANCE](../roadmap/ACCEPTANCE.md#section-10) | [048](PLAN.md#pr-048) | [08](ACCEPTANCE.md#e2e-08), [15](ACCEPTANCE.md#e2e-15) | 规划中 |
| REQ-27.02 | 实测 GUI 350 MB 内存、产物 250 MB 及冷启动 3 秒目标 | [ACCEPTANCE](../roadmap/ACCEPTANCE.md#section-10) | [048](PLAN.md#pr-048), [050](PLAN.md#pr-050) | [08](ACCEPTANCE.md#e2e-08), [16](ACCEPTANCE.md#e2e-16) | 规划中 |
| REQ-27.03 | 验证未标记包零写入、禁用插件近零专属资源及 I/O 开销 | [ACCEPTANCE](../roadmap/ACCEPTANCE.md#section-10) | [031](PLAN.md#pr-031), [048](PLAN.md#pr-048) | [04](ACCEPTANCE.md#e2e-04), [08](ACCEPTANCE.md#e2e-08), [15](ACCEPTANCE.md#e2e-15) | 规划中 |
| REQ-28.01 | 集中工具链、锁文件及项目依赖解析 | [SYSTEM](../architecture/SYSTEM.md#section-11) | [001](PLAN.md#pr-001) | [01](ACCEPTANCE.md#e2e-01), [16](ACCEPTANCE.md#e2e-16) | Linux 已核验工作区版本及路径。 |
| REQ-28.02 | 集中配置、迁移与敏感项管理 | [DEPENDENCIES](../standards/DEPENDENCIES.md#ag-40) | [001](PLAN.md#pr-001), [006](PLAN.md#pr-006) | [01](ACCEPTANCE.md#e2e-01), [16](ACCEPTANCE.md#e2e-16) | 已实现工具链配置、[按用户隔离的版本化运行配置](../architecture/CORE-STORAGE.md)，以及计划 013 的本地密钥封装和加密迁移。其他敏感项界面仍属于后续计划。 |
| REQ-28.03 | 统一开发入口并验证仓库洁净度 | [DEPENDENCIES](../standards/DEPENDENCIES.md#ag-19) | [001](PLAN.md#pr-001), [002](PLAN.md#pr-002) | [16](ACCEPTANCE.md#e2e-16) | 开发入口、忽略规则、本地及托管工作区检查已验证。 |
| REQ-29.01 | 保护 main，经 PR 由所有者审核且按序合并 | [GIT_AND_PR](../standards/GIT_AND_PR.md#section-13) | [000](PLAN.md#pr-000), [002](PLAN.md#pr-002) | [16](ACCEPTANCE.md#e2e-16) | 已核验所有者合并 PR 004。两项 GitHub Actions 检查和分支保持最新均已强制，绕过名单为空。[工程记录](../records/STATUS.md) |
| REQ-29.02 | 验证强制门禁、缺陷阻断、最新合并候选和合并后回归 | [GIT_AND_PR](../standards/GIT_AND_PR.md#ag-48) | [002](PLAN.md#pr-002), [050](PLAN.md#pr-050) | [16](ACCEPTANCE.md#e2e-16) | PR 004 通过两项强制检查及三个合并后工作流，ERR-004 已解决。PR 050 仍需执行交付回归。[工程记录](../records/STATUS.md) |
| REQ-29.03 | 完成真实 Windows、安全、性能与端到端验收 | [ACCEPTANCE](../roadmap/ACCEPTANCE.md#section-15) | [049](PLAN.md#pr-049), [050](PLAN.md#pr-050) | [01–16](ACCEPTANCE.md#e2e-01) | 规划中 |
| REQ-30.01 | 维护相互引用的双语规范与可追溯记录 | [DOCUMENTATION](../standards/DOCUMENTATION.md#section-12) | [000](PLAN.md#pr-000) | [16](ACCEPTANCE.md#e2e-16) | 规划中 |
| REQ-30.02 | 逐项追踪需求、25 个插件和 64 项规范及证据 | [DOCUMENTATION](../standards/DOCUMENTATION.md#ag-61) | [000](PLAN.md#pr-000), [050](PLAN.md#pr-050), [051](PLAN.md#pr-051), [053](PLAN.md#pr-053) | [16](ACCEPTANCE.md#e2e-16) | 计划 000 核对的是当时的 61 项规范。AG-62 和 AG-63 由计划 051 提出，尚未验收。AG-64 是计划 053，紧接 014 之后，尚未验收。 |

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

## 64 项 Agent 规范

每条独立编号和权威位置见 [AG-01 至 AG-64 索引](../AGENTS.md)。PR [000](PLAN.md#pr-000) 核对的是当时的双语映射。计划 [051](PLAN.md#pr-051) 增加 AG-62 和 AG-63。计划 [053](PLAN.md#pr-053) 增加 AG-64，并紧接在 014 之后。后续 PR 按 [E2E-16](ACCEPTANCE.md#e2e-16) 逐项核查实际证据并维护规范。
