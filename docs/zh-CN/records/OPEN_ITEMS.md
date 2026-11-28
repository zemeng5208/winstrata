# 未验收事项登记

[English](../../records/OPEN_ITEMS.md) · [错误](ERRORS.md) · [进度](STATUS.md) · [索引](../INDEX.md)

本登记由 [AG-62](../AGENTS.md#ag-62) 要求。计划 000 至 013 的工程记录、验收计划或架构说明里仍未补齐这些字段的开放事项都列在这里。[STATUS.md](STATUS.md) 不能代替本登记。已确认的缺陷是错误，不写进本文件。[ERR-005](modules/core.md#err-005) 已在计划 052 的合并上复测。

每一行的交接负责人都是仓库所有者 zemeng5208。负责计划的开发者在指定环境执行并记录结果。所有者接受关闭。只写“等待验证”不是关闭条件。

计划 052 已合并，它是进程退出修复。计划 051 写本登记，不关闭这些事项。计划 014 已合并为 `0accacbaa2dac43db3845cc8c08f23b4e831c413`。计划 053 在 014 之后记录 AG-64，不关闭这些事项。

## 摘要

| ID | 状态 | 负责计划 |
|---|---|---|
| [OI-001](#oi-001) | 未测试 | 049 |
| [OI-002](#oi-002) | 未测试 | 049 |
| [OI-003](#oi-003) | 未测试 | 049 |
| [OI-004](#oi-004) | 未测试 | 049 |
| [OI-005](#oi-005) | 未测试 | 042 |
| [OI-006](#oi-006) | 未测试 | 031 |
| [OI-007](#oi-007) | 未验收 | 008，Linux 交叉构建属于 050 |
| [OI-008](#oi-008) | 已知限制 | 052 之后的存储计划，不是 051 |
| [OI-009](#oi-009) | 未测试 | 016 |
| [OI-010](#oi-010) | 部分已测试 | 050 |
| [OI-011](#oi-011) | 未测试 | 050 |
| [OI-012](#oi-012) | 未测试 | 050 |
| [OI-013](#oi-013) | 未测试 | 016 |
| [OI-014](#oi-014) | 未验收 | 008 的后续 |
| [OI-015](#oi-015) | 已知限制 | 018、022 |
| [OI-016](#oi-016) | 已关闭 | 021 |
| [OI-017](#oi-017) | 未测试 | 031 |
| [OI-018](#oi-018) | 已关闭 | 021 |
| [OI-019](#oi-019) | 部分已测 | 015 |
| [OI-020](#oi-020) | 未测试 | 015、048 |
| [OI-021](#oi-021) | 已知限制 | 050 |
| [OI-022](#oi-022) | 待决定 | 所有者 |
| [OI-023](#oi-023) | 待决定 | 016 |
| [OI-024](#oi-024) | 待决定 | 030 |
| [OI-025](#oi-025) | 待决定 | 所有者，在 031 持久化之前 |
| [OI-026](#oi-026) | 待决定 | 048 |
| [OI-027](#oi-027) | 待决定 | 049 |
| [OI-028](#oi-028) | 已记录的约束 | 保持到有依赖批准 |
| [OI-029](#oi-029) | 已知限制 | 014、015 |
| [OI-030](#oi-030) | 已记录的约束 | 保持到有依赖批准 |
| [OI-031](#oi-031) | 已记录的约束 | REQ-14.03 属于 038 和 042 |
| [OI-032](#oi-032) | 证据缺口 | 下一次记录更新 |
| [OI-033](#oi-033) | 开放流程事项 | 所有者关闭 GitHub PR #11 |
| [OI-034](#oi-034) | 已知限制 | 031 |
| [OI-035](#oi-035) | 已知限制 | 016 |
| [OI-036](#oi-036) | 登记与本提交一致；E2E-19 仍开放 | 016，不新增前驱 |
| [OI-037](#oi-037) | 未实现 | 无路线图行 |
| [OI-038](#oi-038) | 待决定 | 所有者 |
| [OI-039](#oi-039) | 编写主机未测试 | 020 |
| [OI-042](#oi-042) | 已在 Windows Server 2022 关闭 | 客户端仍是 [OI-001](#oi-001)，计划 049 |
| [OI-043](#oi-043) | 未测试 | 022 |
| [OI-044](#oi-044) | 未测试 | 027 |
| [OI-045](#oi-045) | 已知限制 | 027 |
| [OI-046](#oi-046) | 未测试 | 024，负载场景属于 048 |
| [OI-047](#oi-047) | 未实现 | 022、026、030、032、042 |
| [OI-048](#oi-048) | 已知限制 | 无路线图行 |
| [OI-049](#oi-049) | `01f26d1` 上的 Server 2022 探测已记录；Windows 10/11 仍属于 [OI-001](#oi-001) | 023 |
| [OI-050](#oi-050) | 编写主机未测试 | 026 |
| [OI-051](#oi-051) | 未实现 | 030、031、032、016、042 |
| [OI-052](#oi-052) | 头 `bc009ea` 的 Windows Server 2022 已关闭 | 028 |
| [OI-053](#oi-053) | 未测试 | 028 |
| [OI-054](#oi-054) | 未测试 | 029 |
| [OI-055](#oi-055) | 头 `a4f2edd` 的 Windows Server 2022 已关闭 | 029 |
| [OI-056](#oi-056) | 未实现 | 045 |
| [OI-057](#oi-057) | 未实现 | 030 |
| [OI-058](#oi-058) | 编写主机上未测试 | 030 |
| [OI-059](#oi-059) | 未测试 | 030 |
| [OI-060](#oi-060) | 未实现 | 045 |
| [E2E-01](#e2e-01) 至 [E2E-16](#e2e-16) | 未完成 | 见各行 |
| [E2E-17](#e2e-17) 至 [E2E-19](#e2e-19) | 未完成 | 无路线图行 |

<a id="oi-001"></a>

## OI-001

- 关联需求: [REQ-29.03](../roadmap/REQUIREMENTS.md)
- 来源 PR: 计划 004 以及 006 至 013。已记录的 Windows 作业是 Windows Server 2022。
- 当前状态: 未测试
- 未完成原因: 没有记录 Windows 10 或 Windows 11 客户端运行。Server 2022 的成功不是那次运行。
- 负责 PR: 049
- 交接负责人: 仓库所有者 zemeng5208
- 验证环境: Windows 10 x64 和 Windows 11 x64 客户端安装，不是 Server 2022
- 验收标准: 当时已有的工作区测试和产品流程在两个客户端版本上运行，并记录版本
- 关闭条件: 这些记录存在，或者所有者根据证据写下更窄的支持范围。Server 2022 作业不能关闭它。

<a id="oi-002"></a>

## OI-002

- 关联需求: [REQ-11.01](../roadmap/REQUIREMENTS.md)
- 来源 PR: 计划 006、012 和 013。[Core 存储](../architecture/CORE-STORAGE.md) 写明尚未在 ReFS 上执行。
- 当前状态: 未测试
- 未完成原因: 已记录的文件系统测试使用 Windows 上的 NTFS 和 Linux 宿主文件系统
- 负责 PR: 049
- 交接负责人: 仓库所有者 zemeng5208
- 验证环境: 格式化为 ReFS 的 Windows 卷
- 验收标准: 在 ReFS 上执行私有目录检查、迁移和混合存储创建，或者尝试失败并把限制记录下来
- 关闭条件: 该证据进入仓库。NTFS 结果不能关闭它。

<a id="oi-003"></a>

## OI-003

- 关联需求: [REQ-14.02](../roadmap/REQUIREMENTS.md)
- 来源 PR: 计划 013。计划 013 的验收文本包含跨设备完整性。没有运行第二台物理设备。
- 当前状态: 未测试
- 未完成原因: Linux 跨位置迁移把源留在同一台宿主上。它不是第二台机器。
- 负责 PR: 049。计划 038 仍然是插件和时间范围导出。计划 051 不运行这台设备。
- 交接负责人: 仓库所有者 zemeng5208
- 验证环境: 两台物理机器，使用可迁移保险库
- 验收标准: 密文在第二台机器上恢复或迁移并解密，源目录仍按计划 013 的规则保留
- 关闭条件: 该运行被记录。同一宿主上的 Linux 迁移不能关闭它。

<a id="oi-004"></a>

## OI-004

- 关联需求: [REQ-14.02](../roadmap/REQUIREMENTS.md)
- 来源 PR: 计划 013
- 当前状态: 未测试
- 未完成原因: 没有运行第二个操作系统账户。Linux 用户不匹配测试不是那个账户。
- 负责 PR: 049
- 交接负责人: 仓库所有者 zemeng5208
- 验证环境: 同一宿主上的两个操作系统账户
- 验收标准: 在两个账户之间执行迁移或恢复，并记录用户检查
- 关闭条件: 该运行被记录

<a id="oi-005"></a>

## OI-005

- 关联需求: [REQ-14.01](../roadmap/REQUIREMENTS.md)
- 来源 PR: 计划 013。界面锁定显示仍属于计划 042。
- 当前状态: 未测试
- 未完成原因: 没有运行 GUI
- 负责 PR: 042
- 交接负责人: 仓库所有者 zemeng5208
- 验证环境: 能够锁定和解锁保险库的 Windows GUI
- 验收标准: 锁定状态可见，锁定后的读取被拒绝，实时提交仍遵循计划 013 的规则
- 关闭条件: 该 GUI 运行被记录。仅有 Core 测试不能关闭它。

<a id="oi-006"></a>

## OI-006

- 关联需求: [REQ-11.02](../roadmap/REQUIREMENTS.md)
- 来源 PR: 计划 012。该宿主没有 `tshark`、`tcpdump` 或 `editcap`。
- 当前状态: 未测试
- 未完成原因: 仓库内解析器核对了节头魔数 `0x0A0D0D0A` 并读回报文。没有运行外部 PCAPNG 工具。
- 负责 PR: 031
- 交接负责人: 仓库所有者 zemeng5208
- 验证环境: 装有这三种工具之一的宿主
- 验收标准: 该工具读取存储写出的带风险标记的捕获
- 关闭条件: 记录工具名称、版本和结果。仓库内解析器不能关闭本项。

<a id="oi-007"></a>

## OI-007

- 关联需求: [AS-001](STATUS.md) 和计划 008 验收，后者要求 Windows 加载和卸载
- 来源 PR: 计划 003、008、009、010 和 011。没有构建或加载已签名的 INF/CAT 包。没有执行 Secure Boot 和 HVCI。`CatalogHost` 返回 `image_absent`。
- 当前状态: 未验收
- 未完成原因: Linux 链接了计划 003 的探针。Windows 作业拒绝启动缺失映像。那不是已签名加载。
- 负责 PR: 未完成的计划 008 验收。计划 050 仍要核验完整的 Linux 交叉构建，它本身不关闭 Windows 加载。所有者指定驱动 PR。计划 051 不加载驱动。
- 交接负责人: 仓库所有者 zemeng5208
- 验证环境: 启用 Secure Boot 和 HVCI 的 Windows，以及已签名的 INF/CAT 包
- 验收标准: 一个代表性驱动完成加载和卸载，证据写明包、Windows 版本和保护状态
- 关闭条件: 该证据被记录。测试宿主返回的 `ready` 不能关闭它。不标成通过。

<a id="oi-008"></a>

## OI-008

- 关联需求: [REQ-14.01](../roadmap/REQUIREMENTS.md)
- 来源 PR: 计划 013。[Core 存储](../architecture/CORE-STORAGE.md) 写明，分段重写和保险库重新包装之间如果崩溃，新密文可能仍由旧包装保护。进程内重新包装失败时，Core 会尝试写回先前的密钥。这不是崩溃安全的轮换。
- 当前状态: 已知限制
- 未完成原因: 崩溃窗口已经描述，但没有关闭。它没有被验收。
- 负责 PR: 计划 052 之后的存储计划。不是计划 051，也不是计划 014 的插件工作。
- 交接负责人: 仓库所有者 zemeng5208
- 验证环境: 在重写和重新包装之间注入故障，环境为 Linux 和 Windows Server 2022
- 验收标准: 这次崩溃不会让新密文留在旧包装下，并且复测被记录
- 关闭条件: 两个环境上的复测都通过。DPAPI 在 [OI-031](#oi-031) 下仍然不使用。[REQ-14.03](../roadmap/REQUIREMENTS.md) 仍属于计划 038 和 042。

<a id="oi-009"></a>

## OI-009

- 关联需求: [REQ-19.01](../roadmap/REQUIREMENTS.md)
- 来源 PR: 2026-11-27 的所有者指定，由计划 051 记录。计划 004 只测量了 Amicro、Glasswork 和 shadcn-admin。指定时读到的上游提交是 `c9eece0fe425ee7dfe97d162d39d1665c3bf9368`。
- 当前状态: 未测试
- 未完成原因: 计划 016 挂载了修订 `c9eece0fe425ee7dfe97d162d39d1665c3bf9368` 的原创 Ink Switch 和 Signal Return。Fluent UI、Astryx 和 Geist 字体文件不是产品文件。该修订仍然没有做过计划 004 那种 WebView2 或性能测量。
- 负责 PR: 016
- 交接负责人: 仓库所有者 zemeng5208
- 验证环境: 与三个已测量来源同类的离线 WebView2 测量，加上所选修订的许可证和版本审查
- 验收标准: 所选 UI Moments 修订的证据被记录，或者所有者撤回指定
- 关闭条件: 两种结果之一被记录。指定本身不关闭它。GitHub PR #11 留给所有者。见 [OI-033](#oi-033)。

<a id="oi-010"></a>

## OI-010

- 关联需求: [REQ-28.03](../roadmap/REQUIREMENTS.md)
- 来源 PR: 计划 007 至 013。那些宿主上的 Node.js 是 v22.23.4。固定引擎是 24.22.3。那里没有运行 `cargo dev doctor`。计划 012 和 013 也没有在该宿主上运行 `cargo dev verify`。工作区测试命令是直接运行的。
- 当前状态: 部分已测试
- 未完成原因: Linux CI 运行 `.github/scripts/check-workspace.sh`，它调用 `cargo dev verify`，verify 再调用 doctor 检查。Windows CI 运行 `cargo test --locked --workspace`，不运行 `cargo dev doctor`。Node 版本不符的本机运行不是固定 Node 的 doctor 结果。
- 负责 PR: 050
- 交接负责人: 仓库所有者 zemeng5208
- 验证环境: 安装 Node.js 24.22.3 和固定 Rust 工具链的 Windows
- 验收标准: 该 Windows 宿主上记录 `cargo dev doctor` 和 `cargo dev verify`
- 关闭条件: 该记录存在。Linux CI 不关闭 Windows 的 doctor 缺口。

<a id="oi-011"></a>

## OI-011

- 关联需求: [REQ-28.01](../roadmap/REQUIREMENTS.md)
- 来源 PR: 计划 001。Linux 的 doctor、init、build 和 verify 通过。Windows 宿主执行仍未核验。
- 当前状态: 未测试
- 未完成原因: 没有记录开发工具的 Windows 运行
- 负责 PR: 050
- 交接负责人: 仓库所有者 zemeng5208
- 验证环境: 装有固定工具链的 Windows x64
- 验收标准: `cargo dev doctor`、`cargo dev init`、`cargo dev build` 和 `cargo dev verify` 在该宿主上通过，或者失败被登记为错误
- 关闭条件: 命令结果被记录

<a id="oi-012"></a>

## OI-012

- 关联需求: 计划 003 验收，链接并检查真实产物
- 来源 PR: 计划 003。Linux 探针完成链接。没有执行 Windows 程序或驱动。映像留在忽略目录。
- 当前状态: 未测试
- 未完成原因: 探针映像没有在 Windows 上执行。签名和加载仍是 [OI-007](#oi-007)。
- 负责 PR: 050 负责交叉构建证据。OI-007 仍然是加载。
- 交接负责人: 仓库所有者 zemeng5208
- 验证环境: 能够检查已链接 PE 映像、且不把它们当成产品驱动的 Windows 宿主
- 验收标准: 映像在 Windows 上被检查，或者所有者记录为何 Linux 检查仍然是唯一证据
- 关闭条件: 该记录存在。它不表示驱动已加载。

<a id="oi-013"></a>

## OI-013

- 关联需求: [AS-002](STATUS.md) 和 [E2E-11](#e2e-11)
- 来源 PR: 计划 008。Windows 作业不是交互式未提升 GUI。
- 当前状态: 未测试
- 未完成原因: 计划 016 对 Windows 窗口、托盘和 WebView2 加载器路径做了类型检查。这台 Linux 主机没有运行普通用户的 Windows GUI 会话。E2E-11 未完成。
- 负责 PR: 016
- 交接负责人: 仓库所有者 zemeng5208
- 验证环境: 普通用户的 Windows GUI 会话
- 验收标准: GUI 在未提升用户下运行，并记录权限
- 关闭条件: 该会话被记录

<a id="oi-014"></a>

## OI-014

- 关联需求: 计划 008 对服务控制面的验收
- 来源 PR: 计划 008。`winstrata-service` 没有注册到服务控制管理器。
- 当前状态: 未验收
- 未完成原因: 管道和令牌测试已运行。服务注册没有运行。
- 负责 PR: 未完成的计划 008 验收。所有者安排服务变更。计划 047 不代替本项。
- 交接负责人: 仓库所有者 zemeng5208
- 验证环境: Windows Server 2022 服务控制管理器
- 验收标准: `winstrata-service` 通过服务控制管理器创建、启动和停止，或者所有者在计划 008 可以被称作已验收之前指定另一个负责注册的计划
- 关闭条件: 两种记录之一存在

<a id="oi-015"></a>

## OI-015

- 关联需求: [AS-007](STATUS.md) 和 [REQ-06.02](../roadmap/REQUIREMENTS.md)
- 来源 PR: 计划 010。采样不会为两次采样之间启动并退出的进程发出事件。退出时间是采样时间。ETW、内核回调、命令行和服务停止记录不属于计划 010。
- 当前状态: 已知限制
- 未完成原因: 计划 018 通过 `strata.process` 呈现计划 010 的采样，包括真实进程、线程、模块、树和归属事件，并在禁用时结束采样线程。服务停止记录现在是计划 022 的 `service.stopped`，并由跟踪测试覆盖。这台 Linux 主机没有观察到服务控制管理器上一次进入停止的真实转换。ETW、内核回调、命令行，以及只存在于两次采样之间的进程仍开放，并且不属于计划 022。
- 负责 PR: 计划 022 记录 `service.stopped`，跟踪测试覆盖这条记录。这台 Linux 主机没有观察到服务控制管理器上一次进入停止的真实转换（[OI-043](#oi-043)）。ETW、命令行、内核回调，以及只存在于两次采样之间的进程仍开放，并且不属于计划 022。它们保持开放，直到所有者确认它们留在计划 018 之外，或后续计划测试它们。
- 交接负责人: 仓库所有者 zemeng5208
- 验证环境: Windows，使用计划 018 或 022 实际实现的来源
- 验收标准: 被推迟的来源被测试，或者所有者确认它留在那些计划之外
- 关闭条件: 该确认或测试被记录。采样器测试不能关闭它。

<a id="oi-016"></a>

## OI-016

- 关联需求: [AS-006](STATUS.md)
- 来源 PR: 计划 009。`ReadDirectoryChangesW` 和 `RegNotifyChangeKeyValue` 省略 `precision_ns` 和执行进程。没有加载已签名的微过滤驱动或注册表回调。
- 当前状态: 已关闭
- 未完成原因: Windows 实机结果已经记录。证据是用户态的大小和权限元数据。`precision_ns` 和执行进程仍然省略。没有加载已签名的文件微过滤驱动。这不是内核归因。
- 负责 PR: 021。计划 019 的用户态注册表证据已记录，并且不是内核归因。
- 交接负责人: 仓库所有者 zemeng5208
- 验证环境: Windows 文件监视，或以后批准的内核回调
- 验收标准: 计划 021 测试自己的前后归因，或者省略字段继续被记录
- 关闭条件: 已在 `dc9d9e378f9f585929f6acfc18944411f1bbcc75` 满足。Windows Server 2022 通过 `windows_trace_records_metadata_without_content`：观察到真实的创建、大小变化和删除，秘密文件字节没有出现，`precision_ns` 被省略，进程字段被省略。[Windows 作业](https://github.com/zemeng5208/winstrata/actions/runs/46065282125/job/140728981512)。计划 019 不关闭本项。不是内核归因。[ERR-019](ERRORS.md#err-019) 是这次结果之前的失败运行。

<a id="oi-017"></a>

## OI-017

- 关联需求: 计划 011 验收和 [E2E-04](#e2e-04)
- 来源 PR: 计划 011。没有运行已签名 `strata-net.sys` 加载、拓扑、发现和 PCAPNG 导出。未标记连接元数据没有落盘。`CatalogHost` 仍然返回 `image_absent`。
- 当前状态: 未测试
- 未完成原因: 用户态观测已运行。产品网络体验没有运行。
- 负责 PR: 031。未标记元数据的持久化还要等 [OI-025](#oi-025)。
- 交接负责人: 仓库所有者 zemeng5208
- 验证环境: Windows，针对计划 031 负责的产品捕获路径
- 验收标准: 运行计划 031 用来替代这些缺口的场景；除非 [OI-007](#oi-007) 已关闭，否则不声称已签名驱动
- 关闭条件: 这些结果被记录

<a id="oi-018"></a>

## OI-018

- 关联需求: 计划 009 的文件监视行为
- 来源 PR: 计划 009。没有故意撑满 16 KiB 文件缓冲区。
- 当前状态: 已关闭
- 未完成原因: 真实溢出已经记录。生产缓冲区仍是 16 KiB。
- 负责 PR: 021
- 交接负责人: 仓库所有者 zemeng5208
- 验证环境: Windows Server 2022，GitHub Actions `windows-2022`
- 验收标准: 测试迫使溢出并记录缺口行为
- 关闭条件: 已满足。在头 `0b46a2edba97a587d774d1b721f714dde840f6a6` 上，`windows_file_watch_overflow_records_a_lost_gap` 通过。[Windows 作业](https://github.com/zemeng5208/winstrata/actions/runs/46064663019/job/140726967687)。同一作业随后在跟踪插件里失败，见 [ERR-019](ERRORS.md#err-019)。那次失败不重新打开这次溢出结果。

<a id="oi-019"></a>

## OI-019

- 关联需求: 计划 004 的隔离证据和 [REQ-21.02](../roadmap/REQUIREMENTS.md)
- 来源 PR: 计划 004。平台报告写明，已经打开的句柄的撤销、网络访问、注册表，以及托管完整 Python 或 Node 运行时，仍未核验。
- 当前状态: Windows 上未测试
- 未完成原因: Linux 上的计划 015 对真实 Python 和 Node 进程拒绝了套接字、新程序、规则集之外的文件和继承的描述符。脚本在允许的状态目录内打开的句柄随后仍然可读。Windows 的 AppContainer、注册表和网络测试已在套件中，撰写主机没有执行。
- 负责 PR: 015
- 交接负责人: 仓库所有者 zemeng5208
- 验证环境: Windows AppContainer，或计划 015 选择的隔离
- 验收标准: 每个点名的缺口被测试，或者明确留下不支持并附证据
- 关闭条件: 该记录存在

<a id="oi-020"></a>

## OI-020

- 关联需求: [E2E-08](#e2e-08) 和 [E2E-15](#e2e-15)
- 来源 PR: 计划 007。该计划没有运行 Windows 10/11 客户端行为、GUI 延迟、持续压力、动态预算、带认证的 IPC、ReFS 和加密迁移。加密迁移后来由计划 013 在 Linux 和 Windows Server 2022 审查头 `36b5cd6` 上执行。其余事项没有。
- 当前状态: 未测试
- 未完成原因: 调度器测试不是持续负载或 GUI 运行。计划 026 在一次短的 Linux 测试里调整 CPU、内存和队列限额。那次测试不是本行所说的持续负载或 GUI 延迟。
- 负责 PR: 015 负责带认证的 IPC，048 负责持续压力和 GUI 延迟。计划 026 不关闭本行。
- 交接负责人: 仓库所有者 zemeng5208
- 验证环境: 每个负责计划所指定的环境
- 验收标准: 点名的测量或 IPC 检查被记录
- 关闭条件: 这些记录存在。20,000 条事件的 Linux release 样本不能关闭它们。

<a id="oi-021"></a>

## OI-021

- 关联需求: [REQ-28.03](../roadmap/REQUIREMENTS.md)
- 来源 PR: 计划 006。整个工作区的 rustfmt 仍报告未改动的开发工具源码中的旧排版差异。
- 当前状态: 已知限制
- 未完成原因: 差异已被记录。没有保存 rustfmt 的失败 CI 检查，因此这不是缺陷。
- 负责 PR: 050
- 交接负责人: 仓库所有者 zemeng5208
- 验证环境: 固定的 Rust 工具链
- 验收标准: `cargo fmt --check` 对 `toolchains/dev` 干净，或者所有者接受当前排版
- 关闭条件: 两种记录之一存在

<a id="oi-022"></a>

## OI-022

- 关联需求: [未决事项](../roadmap/PLAN.md#section-19) 中的仓库名称一行
- 来源 PR: 计划 000
- 当前状态: 待决定
- 未完成原因: 另一家软件业务使用 winstrata.com。正式商标核验仍待完成。
- 负责 PR: 所有者，不在功能计划内
- 交接负责人: 仓库所有者 zemeng5208
- 验证环境: 所有者选择的商标检索
- 验收标准: 正式结果被记录
- 关闭条件: 该结果进入仓库

<a id="oi-023"></a>

## OI-023

- 关联需求: [AS-003](STATUS.md) 和 [REQ-19.02](../roadmap/REQUIREMENTS.md)
- 来源 PR: 未决事项列表。初始主题尚未选定。
- 当前状态: 待决定
- 未完成原因: 计划 016 在用户选择之前不保存主题。所有者尚未确认初始主题，因此没有任何主题被呈现为产品默认。
- 负责 PR: 016
- 交接负责人: 仓库所有者 zemeng5208
- 验证环境: 计划 016 的 GUI
- 验收标准: 所有者在初始主题被呈现为默认之前确认它
- 关闭条件: 该决定被记录

<a id="oi-024"></a>

## OI-024

- 关联需求: [AS-003](STATUS.md) 和 [未决事项](../roadmap/PLAN.md#section-19) 中的容量一行
- 来源 PR: 计划 000 和 012。保留天数可以存储。清理属于计划 030。
- 当前状态: 待决定
- 未完成原因: 单位、已保存日志的记账、默认清理选择和过期授权尚未确认
- 负责 PR: 030
- 交接负责人: 仓库所有者 zemeng5208
- 验证环境: 计划 030 的清理流程
- 验收标准: 清理删除数据之前确认这些语义
- 关闭条件: 该决定和计划 030 的测试被记录

<a id="oi-025"></a>

## OI-025

- 关联需求: [AS-003](STATUS.md) 和 [REQ-10.02](../roadmap/REQUIREMENTS.md)
- 来源 PR: 计划 011 不持久化未标记连接元数据。未标记的原始报文仍只在内存中。
- 当前状态: 待决定
- 未完成原因: 是否持久化未标记连接元数据尚未决定
- 负责 PR: 所有者在计划 031 持久化任何此类数据之前决定
- 交接负责人: 仓库所有者 zemeng5208
- 验证环境: 决定存在之前不是运行时
- 验收标准: 决定写入路线图
- 关闭条件: 该决定被记录。计划 011 拒绝持久化它，并不选择长期策略。

<a id="oi-026"></a>

## OI-026

- 关联需求: [AS-003](STATUS.md) 和 [REQ-27.01](../roadmap/REQUIREMENTS.md)
- 来源 PR: 验收计划给出建议步骤。所有者尚未确认。
- 当前状态: 待决定
- 未完成原因: 代表性工作负载、平均值和峰值、记账，以及具体硬件仍开放
- 负责 PR: 048
- 交接负责人: 仓库所有者 zemeng5208
- 验证环境: 所有者确认方法时点名的硬件
- 验收标准: 在宣布结果之前确认测量定义
- 关闭条件: 该确认被记录

<a id="oi-027"></a>

## OI-027

- 关联需求: [未决事项](../roadmap/PLAN.md#section-19) 中的 Windows 一行
- 来源 PR: 计划 000。范围是 Windows 10/11 x64。最低构建号尚未确认。
- 当前状态: 待决定
- 未完成原因: 客户端运行还没有产生证据。见 [OI-001](#oi-001)。
- 负责 PR: 049
- 交接负责人: 仓库所有者 zemeng5208
- 验证环境: OI-001 中的客户端版本
- 验收标准: 根据那些运行或所有者决定记录最低支持构建
- 关闭条件: 该记录存在

<a id="oi-028"></a>

## OI-028

- 关联需求: [AS-004](STATUS.md)
- 来源 PR: 计划 007。工作线程池使用 Rust 标准库，因为没有批准异步运行时 crate。
- 当前状态: 已记录的约束
- 未完成原因: 这是持续约束，不是“异步运行时已经测试”的未核验声称
- 负责 PR: 只有在提议替换运行时时，才属于未来的依赖计划
- 交接负责人: 仓库所有者 zemeng5208
- 验证环境: 实现仍是标准库时不适用
- 验收标准: 在单独的依赖批准之前，该约束继续有效
- 关闭条件: 已批准的依赖替换它，或者它继续作为当前设计被记录。它不是缺陷。

<a id="oi-029"></a>

## OI-029

- 关联需求: [AS-005](STATUS.md)
- 来源 PR: 计划 008。服务认证的是操作系统用户，不是进程角色。以服务所有者身份运行的进程可以发送角色 `core`。
- 当前状态: 已知限制
- 未完成原因: 插件能力分离尚未实现
- 负责 PR: 014 和 015
- 交接负责人: 仓库所有者 zemeng5208
- 验证环境: Windows 管道或 Linux 对端，使用同一操作系统用户的两个进程
- 验收标准: 同一用户的进程不能获得另一角色的订阅权，或者所有者为该版本接受这一限制
- 关闭条件: 测试或接受被记录

<a id="oi-030"></a>

## OI-030

- 关联需求: [AS-008](STATUS.md)
- 来源 PR: 计划 012。没有增加 SQLite crate。替换分段格式需要单独的依赖决定。
- 当前状态: 已记录的约束
- 未完成原因: 这是已批准的存储形态，不是缺少 SQLite 测试
- 负责 PR: 只有在提议 SQLite 时，才属于未来的依赖计划
- 交接负责人: 仓库所有者 zemeng5208
- 验证环境: 存储仍是分段时不适用
- 验收标准: 在依赖批准之前该约束继续有效
- 关闭条件: 该批准出现，或者该约束继续作为当前设计。它不是缺陷。

<a id="oi-031"></a>

## OI-031

- 关联需求: [AS-009](STATUS.md) 和 [REQ-14.03](../roadmap/REQUIREMENTS.md)
- 来源 PR: 计划 013。可迁移保险库不使用 DPAPI，因为那份密文绑在一个 Windows 配置文件上，无法在另一台机器上恢复。
- 当前状态: 已记录的约束
- 未完成原因: 没有使用 DPAPI，也不把它声称成已验收的保护方法
- 负责 PR: 038 和 042 仍然负责 REQ-14.03。DPAPI 设计会是新的依赖和产品决定。
- 交接负责人: 仓库所有者 zemeng5208
- 验证环境: 不是一次 DPAPI 运行
- 验收标准: 除非所有者批准另一条不可迁移的路径，可迁移保险库保持不使用 DPAPI
- 关闭条件: 约束继续存在，或者新的已批准设计替换它。REQ-14.03 没有被验收。

<a id="oi-032"></a>

## OI-032

- 关联需求: 计划 009 和计划 010 合并提交的证据
- 来源 PR: 合并 `8fe3014846605a726249f64d58eaca9362eaacd4` 和合并 `145617f466fbb446085ec237a0593da50422e5d7`。写那些记录时，提交状态接口没有返回上下文。工作流运行本身成功。[计划 009 主分支 CI](https://github.com/zemeng5208/winstrata/actions/runs/46012223713)。[计划 010 主分支 CI](https://github.com/zemeng5208/winstrata/actions/runs/46014395187)。
- 当前状态: 证据缺口
- 未完成原因: 没有为这些 SHA 保存已发布的状态说明。本计划没有再次查询该接口，因此不编造说明。
- 负责 PR: 下一次记录更新
- 交接负责人: 仓库所有者 zemeng5208
- 验证环境: 这两个 SHA 的 GitHub 提交状态 API
- 验收标准: 后来的读取记录该说明，或者所有者接受工作流成功而没有保存的说明
- 关闭条件: 两种结果之一被写下来。这不是产品缺陷。

<a id="oi-033"></a>

## OI-033

- 关联需求: [AG-12](../standards/GIT_AND_PR.md#ag-12)。关闭另一个 PR 需要所有者批准。
- 来源 PR: [GitHub PR #11](https://github.com/zemeng5208/winstrata/pull/11)，头 `4017bc7aa8219ea317aa27fb2b65c0d2cd8ef9a3`，仍然开放
- 当前状态: 开放流程事项
- 未完成原因: 计划 051 承接文档意图。它不关闭 PR #11。
- 负责 PR: 所有者关闭 PR #11
- 交接负责人: 仓库所有者 zemeng5208
- 验证环境: GitHub
- 验收标准: 文档进入 main 后由所有者关闭 PR #11，或者所有者说明保留开放的理由
- 关闭条件: 所有者的行动被记录

<a id="oi-034"></a>

## OI-034

- 关联需求: 计划 011 的捕获证据
- 来源 PR: 计划 011。无特权现场测试记录 `capture_inaccessible`，因为 `AF_PACKET` 返回 `EPERM`。已提交测试的一次本机 root 运行在 0.42 秒内通过。该 root 结果不是 CI。Windows 环回交换没有被声称成加密证据。
- 当前状态: 已知限制
- 未完成原因: CI 没有捕获权限，root 运行是本机的
- 负责 PR: 031
- 交接负责人: 仓库所有者 zemeng5208
- 验证环境: 被授权的捕获环境，并标明是 CI 还是已批准的实验室
- 验收标准: 记录写明实际使用的权限，以及是否分类了 TLS 头
- 关闭条件: 该记录存在，并且不把本机 root 运行引成 CI

<a id="oi-035"></a>

## OI-035

- 关联需求: [平台报告](../architecture/PLATFORM_FEASIBILITY.md) 中计划 004 的工具链说明
- 来源 PR: 计划 004。已知 Vite 公告 [GHSA-587h-7wmj-h5pm](https://github.com/advisories/GHSA-587h-7wmj-h5pm) 影响上游锁定的开发工具。
- 当前状态: 已知限制
- 未完成原因: 计划 016 不采用 Vite。官方外壳用计划 004 的完整性校验 esbuild 解包，从已批准来源自己的锁文件构建。生产工具链仍未选定，因此没有针对某一套生产工具链重新审查该公告。
- 负责 PR: 016，在选定生产工具链之前
- 交接负责人: 仓库所有者 zemeng5208
- 验证环境: 该公告和所选工具链的锁文件
- 验收标准: 采用该工具链之前记录一次新的审查
- 关闭条件: 该审查被记录。试验依赖授权不能关闭它。

<a id="e2e-01"></a>

## E2E-01

- 关联需求: [E2E-01](../roadmap/ACCEPTANCE.md#e2e-01)
- 来源 PR: 验收计划。没有记录端到端运行。
- 当前状态: 未完成
- 未完成原因: 计划 017 在 Linux 上记录了引导选择、范围、基线和模式标记。没有运行真实的 Windows 启动。该场景没有关闭。
- 负责 PR: 017
- 交接负责人: 仓库所有者 zemeng5208
- 验证环境: 选择数据位置后的真实 Windows 启动
- 验收标准: 验收计划中的场景文本
- 关闭条件: 真实运行记录预期证据。不标成通过。

<a id="e2e-02"></a>

## E2E-02

- 关联需求: [E2E-02](../roadmap/ACCEPTANCE.md#e2e-02)
- 来源 PR: 计划 009 和 010 记录 E2E-02 未运行
- 当前状态: 未完成
- 未完成原因: 计划 020 监视用户和系统环境以及 PATH。计划 021 增加了文件插件和精确路径的历史投影。这个场景仍需要 GUI 查询、选定磁盘和其他用户。该运行没有做。不标成通过。
- 负责 PR: 021
- 交接负责人: 仓库所有者 zemeng5208
- 验证环境: 带选定磁盘和测试键的 Windows
- 验收标准: 场景文本
- 关闭条件: 该运行被记录

<a id="e2e-03"></a>

## E2E-03

- 关联需求: [E2E-03](../roadmap/ACCEPTANCE.md#e2e-03)
- 来源 PR: 计划 010
- 当前状态: 未完成
- 未完成原因: 软件安装和卸载流程属于后续计划
- 负责 PR: 035
- 交接负责人: 仓库所有者 zemeng5208
- 验证环境: 可以安装和移除合法软件的 Windows
- 验收标准: 场景文本
- 关闭条件: 该运行被记录

<a id="e2e-04"></a>

## E2E-04

- 关联需求: [E2E-04](../roadmap/ACCEPTANCE.md#e2e-04)
- 来源 PR: 计划 011
- 当前状态: 未完成
- 未完成原因: 计划 029 给一条真实流打了标记，并证明了未标记报文门禁。没有运行拓扑、发现和 PCAPNG 导出
- 负责 PR: 031
- 交接负责人: 仓库所有者 zemeng5208
- 验证环境: 被授权的 Windows 网络
- 验收标准: 场景文本
- 关闭条件: 该运行被记录

<a id="e2e-05"></a>

## E2E-05

- 关联需求: [E2E-05](../roadmap/ACCEPTANCE.md#e2e-05)
- 来源 PR: 计划 012
- 当前状态: 未完成
- 未完成原因: 可以存储保留期。没有运行阈值通知、删除和索引修复。
- 负责 PR: 030
- 交接负责人: 仓库所有者 zemeng5208
- 验证环境: 有足够采集数据越过阈值的 Windows；必要时先用更低阈值
- 验收标准: 场景文本
- 关闭条件: 该运行被记录

<a id="e2e-06"></a>

## E2E-06

- 关联需求: [E2E-06](../roadmap/ACCEPTANCE.md#e2e-06)
- 来源 PR: 验收计划。插件尚未实现。
- 当前状态: 未完成
- 未完成原因: 计划 014 至 017 和 043 尚未开始
- 负责 PR: 043
- 交接负责人: 仓库所有者 zemeng5208
- 验证环境: 带真实插件的 Windows
- 验收标准: 场景文本
- 关闭条件: 该运行被记录

<a id="e2e-07"></a>

## E2E-07

- 关联需求: [E2E-07](../roadmap/ACCEPTANCE.md#e2e-07)
- 来源 PR: 验收计划
- 当前状态: 未完成
- 未完成原因: 计划 027 在 Linux 上运行编辑器、三种语言、调试、触发器、日志、禁用、权限和空预设检查。手动快照比较属于计划 038。过滤回退属于计划 028。Windows 脚本页没有打开。见 [OI-044](#oi-044) 和 [OI-045](#oi-045)。
- 负责 PR: 工作台属于 027。快照仍属于 038。过滤回退仍属于 028。
- 交接负责人: 仓库所有者 zemeng5208
- 验证环境: Windows 上的脚本工作台
- 验收标准: 场景文本
- 关闭条件: 该运行被记录

<a id="e2e-08"></a>

## E2E-08

- 关联需求: [E2E-08](../roadmap/ACCEPTANCE.md#e2e-08)
- 来源 PR: 计划 007
- 当前状态: 未完成
- 未完成原因: 计划 024 在这台 Linux 主机上记录了 CPU、内存和实际暴露的计数器，并将其余读数标为不支持。计划 025 读取已附着的故障、队列、延迟和丢失。计划 026 在这台 Linux 主机上调整 CPU、内存和队列限额，暂停并恢复，并标记丢失。本场景仍需要授权的持续负载、SSD 计数器和一次 Windows 运行。
- 负责 PR: 048
- 交接负责人: 仓库所有者 zemeng5208
- 验证环境: 被授权负载下的 Windows
- 验收标准: 场景文本
- 关闭条件: 该运行被记录

<a id="e2e-09"></a>

## E2E-09

- 关联需求: [E2E-09](../roadmap/ACCEPTANCE.md#e2e-09)
- 来源 PR: 计划 012
- 当前状态: 未完成
- 未完成原因: 搜索、时间线和导出流程属于后续计划
- 负责 PR: 041
- 交接负责人: 仓库所有者 zemeng5208
- 验证环境: 带跨来源真实应用的 Windows
- 验收标准: 场景文本
- 关闭条件: 该运行被记录

<a id="e2e-10"></a>

## E2E-10

- 关联需求: [E2E-10](../roadmap/ACCEPTANCE.md#e2e-10)
- 来源 PR: 计划 012 和 013
- 当前状态: 未完成
- 未完成原因: Core 加密测试在 Linux 和计划 013 的 Windows Server 2022 审查头上通过。该场景还需要监视期间的锁定和解锁、回滚、插件隔离和导入限制。[OI-003](#oi-003)、[OI-004](#oi-004)、[OI-005](#oi-005) 和 [OI-008](#oi-008) 仍开放。
- 负责 PR: 042
- 交接负责人: 仓库所有者 zemeng5208
- 验证环境: 监视期间锁定保险库的 Windows
- 验收标准: 场景文本，以及它所依赖的四项开放事项已关闭，或被所有者明确排除
- 关闭条件: 该运行被记录。Core 测试不能关闭它。

<a id="e2e-11"></a>

## E2E-11

- 关联需求: [E2E-11](../roadmap/ACCEPTANCE.md#e2e-11)
- 来源 PR: 计划 004 测量了三个组件。UI Moments 是 [OI-009](#oi-009)。
- 当前状态: 未完成
- 未完成原因: Linux 测试保存主题、拒绝驱动命令，并通过环回桥执行引导、暂停、继续和实时/历史。没有运行真实的 Windows WebView2 会话。仪表盘拖拽仍属于计划 042。
- 负责 PR: 016
- 交接负责人: 仓库所有者 zemeng5208
- 验证环境: 真实的 Windows WebView2
- 验收标准: 场景文本，包括三个主题
- 关闭条件: 该运行被记录

<a id="e2e-12"></a>

## E2E-12

- 关联需求: [E2E-12](../roadmap/ACCEPTANCE.md#e2e-12)
- 来源 PR: 验收计划
- 当前状态: 未完成
- 未完成原因: 本机 MCP 产品流程属于后续计划
- 负责 PR: 045
- 交接负责人: 仓库所有者 zemeng5208
- 验证环境: 同一台机器上的真实本机代理
- 验收标准: 场景文本
- 关闭条件: 该运行被记录

<a id="e2e-13"></a>

## E2E-13

- 关联需求: [E2E-13](../roadmap/ACCEPTANCE.md#e2e-13)
- 来源 PR: 验收计划
- 当前状态: 未完成
- 未完成原因: 救援属于计划 047，尚未开始
- 负责 PR: 047
- 交接负责人: 仓库所有者 zemeng5208
- 验证环境: 带无害目标的隔离 Windows 宿主
- 验收标准: 场景文本
- 关闭条件: 该运行被记录

<a id="e2e-14"></a>

## E2E-14

- 关联需求: [E2E-14](../roadmap/ACCEPTANCE.md#e2e-14)
- 来源 PR: 计划 008
- 当前状态: 未完成
- 未完成原因: 更新、服务、驱动、安全和证书观测属于后续计划
- 负责 PR: 036
- 交接负责人: 仓库所有者 zemeng5208
- 验证环境: 被授权的 Windows 宿主
- 验收标准: 场景文本
- 关闭条件: 该运行被记录

<a id="e2e-15"></a>

## E2E-15

- 关联需求: [E2E-15](../roadmap/ACCEPTANCE.md#e2e-15)
- 来源 PR: 计划 007 和 008
- 当前状态: 未完成
- 未完成原因: 没有运行持续、突发、磁盘满和意外退出场景
- 负责 PR: 048
- 交接负责人: 仓库所有者 zemeng5208
- 验证环境: 可以被驱动到这些条件的已授权 Windows 宿主
- 验收标准: 场景文本
- 关闭条件: 该运行被记录

<a id="e2e-16"></a>

## E2E-16

- 关联需求: [E2E-16](../roadmap/ACCEPTANCE.md#e2e-16)
- 来源 PR: 验收计划。计划 000 核对的是当时的 61 项规范和 51 个计划。那次历史核对不是本场景。
- 当前状态: 未完成
- 未完成原因: 最终发布核对尚未运行。main `a2471d5fc13f0a307ee39d2629c454aea4537809` 的 Windows 工作区检查失败。计划 052 后来在 `004974215e239f96160b47d3eef40a84c316e802` 上通过。见 [ERR-005](modules/core.md#err-005)。
- 负责 PR: 050
- 交接负责人: 仓库所有者 zemeng5208
- 验证环境: 精确的合并候选和实际 main
- 验收标准: 场景文本，包括 AG-64 生效后的 64 项规范
- 关闭条件: 该运行记录在必需检查为绿色的 main 提交上

<a id="oi-036"></a>

## OI-036

- 关联需求: [REQ-19.01](../roadmap/REQUIREMENTS.md) 和 [REQ-19.15](../roadmap/REQUIREMENTS.md)
- 来源 PR: 未合并的 GitHub PR #21，`pr/016-desktop-gui`，头 `64bd627c362ad025869905441bee7802dda19ac1`。main `0accacb` 没有 GUI 文件。
- 当前状态: 登记与本提交一致；E2E-19 仍开放
- 未完成原因: 本提交的来源登记与入库文件一致，包括 UI Moments 原创控件。已关闭的 PR #21 中的 `packages/ui/static/shell.js` 和 `packages/ui/static/themes.css` 不在本提交中。WebView2 测量仍是 [OI-009](#oi-009)。E2E-19 没有实现计划编号，也不是计划 016 的验收。
- 负责 PR: 计划 016 的现有行。计划 053 不写入该直接依赖格，也不修改那个分支。
- 交接负责人: 仓库所有者 zemeng5208
- 验证环境: 合并官方 GUI 文件的计划 016 变更，与上游修订对照
- 验收标准: 该提交中的文件满足 [E2E-19](../roadmap/ACCEPTANCE.md#e2e-19)。手写布局和绘制要么改为已批准来源，要么记成所有者批准的缺口。
- 关闭条件: 来源登记行与同一提交中的文件相符。本次规范变更不关闭它。

<a id="oi-037"></a>

## OI-037

- 关联需求: [REQ-19.06](../roadmap/REQUIREMENTS.md) 至 [REQ-19.14](../roadmap/REQUIREMENTS.md)
- 来源 PR: 2026-11-28 的所有者决定，记为提案 [AG-64](../AGENTS.md#ag-64)
- 当前状态: 未实现
- 未完成原因: main 没有 GUI。完整替换、前端 SDK、第二套 GUI 下的解耦，以及可信宿主提示都没有实现。计划 016 既有的官方 GUI 验收未改。
- 负责 PR: 无路线图行。所有者指定后续。它不是计划 015 或计划 016。
- 交接负责人: 仓库所有者 zemeng5208
- 验证环境: 真实 Windows，官方 GUI 仍安装，并有一个第三方 GUI 插件
- 验收标准: [E2E-17](../roadmap/ACCEPTANCE.md#e2e-17) 和 [E2E-18](../roadmap/ACCEPTANCE.md#e2e-18)
- 关闭条件: 所有者批准技术路线并指定路线图行之后，这些运行被记录。静态页面不关闭它。

<a id="oi-038"></a>

## OI-038

- 关联需求: [REQ-19.16](../roadmap/REQUIREMENTS.md)
- 来源 PR: 2026-11-28 的所有者决定
- 当前状态: 待决定
- 未完成原因: 所有者要求技术路线以后再调研和验证。本次变更不选定 Tauri、React、Vite 或任何其他技术栈。
- 负责 PR: 没有实现计划编号。计划 053 是紧接 014 之后的规范行。若所有者以后增加实现计划，本次变更不另编号。
- 交接负责人: 仓库所有者 zemeng5208
- 验证环境: 方案自身的实测证据，在实现之前记录
- 验收标准: [方案一节](../roadmap/ACCEPTANCE.md#gui-proposal)
- 关闭条件: 所有者批准经过验证的方案，或记录撤回该能力。点名一个候选不关闭它。

<a id="oi-039"></a>

## OI-039

- 关联需求: [REQ-06.03](../roadmap/REQUIREMENTS.md) 和 [E2E-02](../roadmap/ACCEPTANCE.md#e2e-02)
- 来源 PR: 计划 020
- 当前状态: 编写主机未测试
- 未完成原因: Linux 编写主机打不开 `HKCU\Environment`，也打不开 `HKLM\SYSTEM\CurrentControlSet\Control\Session Manager\Environment`。Windows 测试 `environment_is_in_process_and_linux_does_not_publish` 在两个真实存储上写入一个探测变量并追加一段 PATH，然后恢复原来的字节。这里没有运行该测试。
- 负责 PR: 020
- 交接负责人: 仓库所有者 zemeng5208
- 验证环境: Windows 工作区作业，Windows Server 2022，运行 `cargo test --locked --workspace`。Windows 10 和 Windows 11 客户端仍属于 [OI-001](#oi-001)。
- 验收标准: 该测试在送审头上通过。它必须展示探测变量的用户和系统前后值、证据包含所追加段的 PATH 事件、不发布被隐藏值的暂停缺口，以及停止期间的变化不会被重放。原来的 PATH 字节被恢复。
- 关闭条件: 本计划送审头的 Windows 工作区日志显示该测试通过，并且本行写上该结果。Linux 运行不关闭它。通过它也不关闭 E2E-02。

<a id="oi-044"></a>

## OI-044

- 关联需求: [E2E-07](../roadmap/ACCEPTANCE.md#e2e-07) 和 [REQ-16.03](../roadmap/REQUIREMENTS.md)
- 来源 PR: 计划 027
- 当前状态: 未测试
- 未完成原因: 头 `a66da7b749ef5657d55f72cccd2d699783dd4a6f` 的 Windows 工作区作业在插件测试里运行了三种语言。没有 Windows 会话打开官方脚本页。计划 025 占用 OI-040、OI-041 和 OI-042，因此这次页面会话是 OI-044。
- 负责 PR: 027
- 交接负责人: 仓库所有者 zemeng5208
- 验证环境: Windows，已选择存储，`winstrata-strata-script` 位于运行时旁边，使用项目固定的 Python 和 Node.js，并打开官方脚本页
- 验收标准: 用 Python、JavaScript 和 TypeScript 分别创建、编辑、运行、调试、触发和禁用脚本。确认预设目录保持为空，并且页面自己不会启用插件。
- 关闭条件: 记录这次 Windows 页面运行。Linux 测试不能关闭它。插件测试也不能关闭它。空闲桥接响应也不能关闭它。

<a id="oi-045"></a>

## OI-045

- 关联需求: [REQ-16.03](../roadmap/REQUIREMENTS.md) 和[计划 015 的隔离](../architecture/RUNTIMES.md)
- 来源 PR: 计划 027
- 当前状态: 已知限制
- 未完成原因: Linux 上的用户脚本子进程使用 Landlock 和 `libwinstrata_shim.so`。Windows 的 AppContainer 和作业对象仍留在宿主的语言插件路径里，并且是私有实现。本计划不复制那段代码，也不新增公开宿主 API。在 Windows 上，脚本子进程启动时没有 AppContainer。
- 负责 PR: 027
- 交接负责人: 仓库所有者 zemeng5208
- 验证环境: Windows。所有者先批准共享隔离入口，或批准与计划 015 一致的脚本插件 AppContainer。
- 验收标准: 用户脚本不能读取已授予的解释器前缀和插件状态之外的路径，也不能写入预设目录。隔离无法安装时不启动子进程。
- 关闭条件: 记录这次 Windows 隔离运行。Linux 的 Landlock 证据不能关闭它。把 Windows 路径标成已通过也不能关闭它。头 `a66da7b749ef5657d55f72cccd2d699783dd4a6f` 上的 Windows 插件测试也不能关闭它。

<a id="oi-043"></a>

## OI-043

- 关联需求: [REQ-06.04](../roadmap/REQUIREMENTS.md)、[E2E-03](../roadmap/ACCEPTANCE.md#e2e-03) 和 [E2E-14](../roadmap/ACCEPTANCE.md#e2e-14)
- 来源 PR: 计划 022
- 当前状态: 未测试
- 未完成原因: 这台 Linux 主机没有服务控制管理器。pid 1 是 tini。`/proc/modules` 为空，也没有正在运行的 cgroup `.service` 单元，因此这里没有观察到真实的驱动加载或真实的服务停止。在头 `1987f4826f92aaf5794d4678d3d4b43872932a97` 上，Windows Server 2022 通过了受限令牌枚举。这关闭了 [ERR-020](modules/strata-services.md#err-020)。同一作业中 `missing_image_probe_records_creation_start_type_and_deletion` 失败：`WinStrata022Probe` 的 `service.start-type` 计数是 0。该缺陷是 [ERR-021](modules/strata-services.md#err-021)。`OpenServiceW` 拿到的服务名没有结尾 NUL，配置读取可能失败，两次采样的启动类型都停在 `Unknown`。那条断言没有执行到创建之后的启动失败和删除。下面的验收条件尚未满足。本项是 OI-043，因为编号时计划 025 占用 OI-040、OI-041 和 OI-042。那两项仍开放的缺口现在是 OI-047 和 OI-048。OI-042 仍只对 Server 2022 的健康作业关闭。计划 020 的环境存储限制仍是 OI-039。
- 负责 PR: 022 负责把 Windows 工作区作业记入记录。E2E-03 和 E2E-14 保持开放。
- 交接负责人: 仓库所有者 zemeng5208
- 验证环境: Windows Server 2022 的服务控制管理器，运行计划 022 的插件测试
- 验收标准: Windows 作业至少观察到一个驱动、一次拒绝访问的枚举、创建、一次启动类型变更、缺失映像探测的启动失败，以及删除 `WinStrata022Probe`，并且只在非停止状态实际变为停止时记录 `service.stopped`
- 关闭条件: 该作业的日志写入 [STATUS.md](STATUS.md)。Linux 跟踪测试不能关闭它。把 E2E-03 或 E2E-14 标成通过不能关闭它。它不能代替 Windows 10/11。

<a id="oi-046"></a>

## OI-046

- 关联需求: [REQ-08.01](../roadmap/REQUIREMENTS.md)
- 来源 PR: 计划 024
- 当前状态: 未测试
- 未完成原因: 这个采集器通过 Win32 读取 Windows 的 CPU、内存、处理器频率、电池百分比和进程工作集。Windows 的 GPU 利用率、专用显存、温度区、RAPL 和 perf 计数器仍是 `unsupported`，原因是 `reader_absent`，因为这个采集器没有这些 API 的读取器。这台 Linux 主机没有执行 Windows 测试。它也没有 sysfs 上的 GPU、cpufreq、温度、hwmon、电池或 RAPL 来源，所以这些 Linux 读数是 `unsupported`，原因是 `no_counter` 或对应的缺失原因。这是这台主机上的如实结果，不是一次 Windows 运行。本项是 OI-046，因为计划 025 占用 OI-040、OI-041 和 OI-042，计划 022 占用 OI-043，开放的计划 027 分支占用 OI-044 和 OI-045。计划 023 占用 [OI-049](#oi-049)。本行不替换那一行。
- 负责 PR: Windows GPU、显存和温度读取器属于 024。磁盘健康仍属于计划 032。授权负载场景仍属于计划 048。
- 交接负责人: 仓库所有者 zemeng5208
- 验证环境: 带 GPU 的 Windows 10 或 11 x64，运行 `cargo test --locked --workspace` 和脉冲观测测试
- 验收标准: 该运行记录测得的 CPU 和内存。GPU 利用率、显存和温度要么来自点名的 API，要么因为该 API 报告计数器不存在而标为 `unsupported`。API 能够读取的计数器不再保留 `reader_absent`。
- 关闭条件: Windows 运行被记录，并且读取器不再对 API 已读到的计数器返回 `reader_absent`。本行不关闭 E2E-08 或计划 032。

<a id="oi-049"></a>

## OI-049

- 关联需求: [REQ-06.04](../roadmap/REQUIREMENTS.md)、[E2E-03](../roadmap/ACCEPTANCE.md#e2e-03) 和 [E2E-14](../roadmap/ACCEPTANCE.md#e2e-14)
- 来源 PR: 计划 023
- 当前状态: Windows Server 2022 探测已记在审查头 `01f26d1a7d09712436b1b27eefdc02a6d49b4e8b`
- 未完成原因: Linux 编写主机打不开 Windows 的 Run 键、启动文件夹、StartupApproved、快捷方式本地路径或任务计划程序。pid 1 是 tini。那台主机上的 Linux 测试没有观察 Windows 启动项。满足本行的结果只属于头 `01f26d1a7d09712436b1b27eefdc02a6d49b4e8b` 的 Windows 工作区作业，镜像 `windows-2022`，系统是 Windows Server 2022。[Windows 作业](https://github.com/zemeng5208/winstrata/actions/runs/46073259939/job/140754957334)。2026-11-27T21:55:44Z（北京时间 05:55:44）的日志显示 `windows_run_folder_link_and_task_changes_are_attributed ... ok`。同一作业显示 `real_startup_and_task_observation_releases_on_disable ... ok`。本项是 OI-049。OI-043 和 [ERR-021](modules/strata-services.md#err-021) 仍属于计划 022。[OI-001](#oi-001)、[E2E-03](../roadmap/ACCEPTANCE.md#e2e-03) 和 [E2E-14](../roadmap/ACCEPTANCE.md#e2e-14) 保持开放。头 `67eeb5f48163574d60ea317a2172473623e8419e` 不是这次结果。
- 负责 PR: 023 负责把 Windows 工作区作业记入记录。E2E-03 和 E2E-14 保持开放。
- 交接负责人: 仓库所有者 zemeng5208
- 验证环境: Windows 工作区作业，Windows Server 2022，运行 `cargo test --locked --workspace`，包括 `windows_run_folder_link_and_task_changes_are_attributed`。Windows 10 和 Windows 11 客户端仍属于 [OI-001](#oi-001)。
- 验收标准: 该测试在送审头上通过。它必须展示一条 HKCU Run 值的增加、变化、通过 StartupApproved 禁用和删除；一个本地路径为 `C:\Probe.exe` 的用户启动快捷方式；以及一条计划任务的创建、变化和删除。Linux 运行不能关闭它。
- 关闭条件: 对审查头 `01f26d1a7d09712436b1b27eefdc02a6d49b4e8b` 上的 Windows Server 2022 探测已经满足。日志是上面的作业，本行写入了该结果。Linux 运行不能满足它。这不关闭 [E2E-03](../roadmap/ACCEPTANCE.md#e2e-03)、[E2E-14](../roadmap/ACCEPTANCE.md#e2e-14)、[OI-043](#oi-043)、[ERR-021](modules/strata-services.md#err-021) 或 [OI-001](#oi-001)。Windows 10 和 Windows 11 仍属于 OI-001。

<a id="oi-050"></a>

## OI-050

- 关联需求: [REQ-08.02](../roadmap/REQUIREMENTS.md) 和 PL-15
- 来源 PR: 计划 026，`plugins/strata-budget`
- 当前状态: 未测试
- 未完成原因: Linux 编写主机没有执行 Windows 限额测试。这里没有运行 `GetProcessMemoryInfo` 和 `GetProcessTimes`。pid 1 是 tini。Linux 通过不是 Windows 通过。
- 负责 PR: 026，本计划审查头上的 Windows 工作区作业
- 交接负责人: 仓库所有者 zemeng5208
- 验证环境: 本计划审查头上的 Windows 工作区作业，`cargo test --locked --workspace`
- 验收标准: 该作业通过 `winstrata-strata-budget` 测试，包括 `pressure_adjusts_limits_pauses_resumes_and_marks_loss`
- 关闭条件: 该作业记在本计划的审查头上。Linux 通过不关闭它。本行不关闭 [OI-001](#oi-001) 或 [E2E-08](#e2e-08)。这不是豁免。

<a id="oi-051"></a>

## OI-051

- 关联需求: [REQ-08.02](../roadmap/REQUIREMENTS.md) 和 PL-15
- 来源 PR: 计划 026，`plugins/strata-budget`
- 当前状态: 未实现
- 未完成原因: 计划 026 调节本进程的 CPU ppm 和驻留集、已附着总线队列加上已附着调度器队列，以及一个正在运行的插件的订阅积压。它对这些读数暂停、恢复并标记丢失。它不实现磁盘 I/O、网络、存储或 GUI 刷新限额，也不为它们编造零读数。
- 负责 PR: 031 负责网络，032 负责磁盘 I/O，030 负责存储，016 和 042 负责 GUI 刷新。计划 048 负责持续压力。本计划不修改 `PLAN.md`，也不把这些编号加进计划 026。
- 交接负责人: 仓库所有者 zemeng5208
- 验证环境: 后续计划自己的运行，使用产生该限额的来源
- 验收标准: 该计划记录来自其自身来源的限额，或所有者收窄 PL-15
- 关闭条件: 这些记录存在。计划 026 的 CPU、内存和队列测试不关闭它。这不是豁免。

<a id="oi-052"></a>

## OI-052

- 关联需求: [REQ-13.01](../roadmap/REQUIREMENTS.md) 和 [REQ-13.02](../roadmap/REQUIREMENTS.md)
- 来源 PR: 计划 028，`plugins/strata-filter`
- 当前状态: 头 `bc009ea2ee3481ae8a9fea295c246380249cd32a` 上的 Windows Server 2022 作业已经关闭本项
- 未完成原因: Linux 编写主机没有执行 Windows 过滤测试。pid 1 是 tini。Linux 上 `gui_script_batch_io_and_self_recursion` 通过不是 Windows 通过。本项是 OI-052，因为 OI-039 至 OI-051 已经占用。满足本行的结果是该头上的 Windows 工作区作业，镜像 `windows-2022`。[Windows 作业](https://github.com/zemeng5208/winstrata/actions/runs/46082494823/job/140784426136)。2026-11-27T23:28:02Z 的日志显示 `gui_script_batch_io_and_self_recursion ... ok`，过滤库测试通过 4 项。同一作业显示 `filter_page_stays_idle_and_does_not_create_plugins_before_a_location ... ok`。这个空闲桥测试不关闭 [OI-053](#oi-053)。
- 负责 PR: 028，本计划审查头上的 Windows 工作区作业
- 交接负责人: 仓库所有者 zemeng5208
- 验证环境: 本计划审查头上的 Windows 工作区作业，`cargo test --locked --workspace`
- 验收标准: 该作业通过 `winstrata-strata-filter` 测试，包括 `gui_script_batch_io_and_self_recursion`
- 关闭条件: 对头 `bc009ea2ee3481ae8a9fea295c246380249cd32a` 已经由上面的作业满足。Linux 通过不关闭它。本行不关闭 [OI-001](#oi-001)、[OI-053](#oi-053)、[E2E-04](#e2e-04)、[E2E-05](#e2e-05)、[E2E-07](#e2e-07) 或 [E2E-08](#e2e-08)。这不是豁免。Windows 10 和 Windows 11 仍属于 [OI-001](#oi-001)。

<a id="oi-053"></a>

## OI-053

- 关联需求: [REQ-13.02](../roadmap/REQUIREMENTS.md) 和 [E2E-07](../roadmap/ACCEPTANCE.md#e2e-07)
- 来源 PR: 计划 028
- 当前状态: 未测试
- 未完成原因: 没有 Windows 会话打开官方过滤页。Linux 桥测试在选择存储位置之前返回空闲结果，并且不创建插件目录。该测试没有在 Windows 上打开页面。本项是 OI-053，因为 [OI-044](#oi-044) 是计划 027 的脚本页。
- 负责 PR: 028
- 交接负责人: 仓库所有者 zemeng5208
- 验证环境: Windows，已选择存储，`winstrata-strata-filter` 位于运行时旁边并且已经运行，以及官方过滤页
- 验收标准: 保存一条包含或排除规则，显示 kept、avoided 和 overhead，并确认页面不发送 `enable`、也不授予权限
- 关闭条件: 该 Windows 页面运行被记录。Linux 测试不是这次关闭。空闲桥响应不是这次关闭。不由 [OI-052](#oi-052) 关闭。

<a id="oi-054"></a>

## OI-054

- 关联需求: [REQ-10.01](../roadmap/REQUIREMENTS.md) 和 PL-24
- 来源 PR: 计划 029，`plugins/strata-tag`
- 当前状态: 未测试
- 未完成原因: 没有 Windows 会话打开官方标记页。Linux 桥测试只检查尚未选择存储位置时 `tag-apply` 保持空闲，以及 `enable` 仍然被拒绝。计划 027 持有 [OI-044](#oi-044)。这个页面会话是 OI-054。上面的 OI-052 和 OI-053 仍是计划 028 写下的行。本行不改它们。
- 负责 PR: 029，在打开 `/tags` 的 Windows 会话上
- 交接负责人: 仓库所有者 zemeng5208
- 验证环境: Windows 10 或 11 x64，官方外壳，页面不启动 `strata.tag`
- 验收标准: 页面显示空闲状态，只发送 `tag-apply`，并且不发送 `enable` 或 `grant`。Linux 桥测试不关闭它。
- 关闭条件: 该会话被记录。本行不关闭 [OI-001](#oi-001)、[E2E-04](#e2e-04) 或 [OI-055](#oi-055)。这不是豁免。

<a id="oi-055"></a>

## OI-055

- 关联需求: [REQ-10.01](../roadmap/REQUIREMENTS.md) 和 PL-24
- 来源 PR: 计划 029，`plugins/strata-tag`
- 当前状态: 对头 `a4f2edd6efd32a504723664f037677f294279592` 上的 Windows Server 2022 作业已关闭
- 未完成原因: Linux 编写主机没有在 Windows 上执行 `winstrata-strata-tag` 的 `cargo test --locked --workspace`。pid 1 是 tini。Linux 通过不是 Windows 通过。这台主机没有安装 `x86_64-pc-windows-msvc` 目标，因此本行也不是类型检查结果。本项是 OI-055，因为计划 028 持有 OI-052 和 OI-053。那两行按计划 028 写下的内容保留。[OI-054](#oi-054) 是 Windows 标记页。满足本行的结果只是头 `a4f2edd6efd32a504723664f037677f294279592` 上的 Windows 工作区作业，镜像 `windows-2022`，作业 140790098053，运行 46084283846。该作业于 2026-11-27T23:47:48Z 成功完成。[Windows 作业](https://github.com/zemeng5208/winstrata/actions/runs/46084283846/job/140790098053)。日志显示 `manual_script_and_plugin_tags_keep_untagged_packets_off_disk ... ok` 和 `tag_stays_absent_until_the_binary_is_beside_the_runtime ... ok`。这不是超出该关闭条件的编写主机通过。这不是豁免。同一作业不关闭 [OI-054](#oi-054) 或 [OI-056](#oi-056)。
- 负责 PR: 029，在本计划审查头的 Windows 工作区作业上
- 交接负责人: 仓库所有者 zemeng5208
- 验证环境: 本计划审查头的 Windows 工作区作业，`cargo test --locked --workspace`
- 验收标准: 该作业通过 `winstrata-strata-tag` 测试，包括 `manual_script_and_plugin_tags_keep_untagged_packets_off_disk` 和 `tag_stays_absent_until_the_binary_is_beside_the_runtime`
- 关闭条件: 对头 `a4f2edd6efd32a504723664f037677f294279592` 已经由作业 140790098053 满足。Linux 通过不关闭它。更后的头不沿用这个作业。本行不关闭 [OI-001](#oi-001)、[E2E-04](#e2e-04)、[OI-054](#oi-054) 或 [OI-056](#oi-056)。这不是豁免。这不是超出该作业的编写主机通过。它不关闭计划 028 的 OI-052。

<a id="oi-056"></a>

## OI-056

- 关联需求: [REQ-10.01](../roadmap/REQUIREMENTS.md) 和 PL-24
- 来源 PR: 计划 029，`plugins/strata-tag`
- 当前状态: 未实现
- 未完成原因: 只有调用方是 `strata.mcp` 时，宿主才会把 actor 标成 `agent`。该插件没有安装。计划 029 不编造一次 Agent 标记。手工、脚本和第三方插件关联已经实现。本项是 OI-056，因为计划 028 持有 OI-052 和 OI-053。那两行按计划 028 写下的内容保留。
- 负责 PR: 045
- 交接负责人: 仓库所有者 zemeng5208
- 验证环境: 计划 045 的运行，`strata.mcp` 已安装并调用 `tag.apply`
- 验收标准: 来自 `strata.mcp` 的调用记录 actor `agent`、`actor_id` `strata.mcp` 和 `safety` `none`，并且仍然不能持久化未标记报文
- 关闭条件: 该运行被记录。计划 029 的用户、脚本或插件标记不关闭它。这不是豁免。它不关闭计划 028 的 OI-053。

<a id="oi-057"></a>

## OI-057

- 关联需求: [REQ-12.02](../roadmap/REQUIREMENTS.md) 和 PL-25
- 来源 PR: 计划 030，`plugins/strata-storage`
- 当前状态: 未实现
- 未完成原因: 官方宿主的托盘提示仍是产品名。计划 030 持久化等待中的阶梯，并在 `/storage` 上显示它。它不改变托盘。等待中的警告不会出现在那里。本项是 OI-057，因为 OI-039 至 OI-056 保持已有记录。
- 负责 PR: 030
- 交接所有者: 仓库所有者 zemeng5208
- 验证环境: Windows，已选择存储，捕获或已保存日志有等待中的阈值，并且官方窗口已隐藏到托盘
- 验收标准: 托盘显示等待中的类别，隐藏窗口不会删除数据
- 关闭条件: 该会话被记录在包含托盘改动的那个头上。阶梯 JSON 不关闭它。这不是豁免。它不关闭 [OI-024](#oi-024) 或 [OI-001](#oi-001)。

<a id="oi-058"></a>

## OI-058

- 关联需求: [REQ-12.02](../roadmap/REQUIREMENTS.md)、[REQ-12.03](../roadmap/REQUIREMENTS.md) 和 PL-25
- 来源 PR: 计划 030，`plugins/strata-storage`
- 当前状态: 编写主机上未测试
- 未完成原因: Linux 编写主机没有在 Windows 上执行 `winstrata-strata-storage` 的 `cargo test --locked --workspace`。pid 1 是 tini。Linux 通过不是 Windows 通过。这台主机没有安装 `x86_64-pc-windows-msvc` 目标，因此本行也不是类型检查结果。本项是 OI-058，因为 OI-039 至 OI-056 保持已有记录。[OI-055](#oi-055) 仍是计划 029 的 Windows 作业。
- 负责 PR: 030，本计划审查头上的 Windows 工作区作业
- 交接所有者: 仓库所有者 zemeng5208
- 验证环境: 本计划审查头上的 Windows 工作区作业，`cargo test --locked --workspace`
- 验收标准: 该作业通过 `thresholds_survive_restart_and_confirmed_deletion_updates_indexes`
- 关闭条件: 该作业被记录在确切的头上。Linux 通过不关闭它。更后的头不沿用这个作业。本行不关闭 [OI-001](#oi-001)、[E2E-05](#e2e-05)、[OI-024](#oi-024)、[OI-057](#oi-057)、[OI-059](#oi-059) 或 [OI-060](#oi-060)。这不是豁免。

<a id="oi-059"></a>

## OI-059

- 关联需求: [REQ-12.02](../roadmap/REQUIREMENTS.md) 和 PL-25
- 来源 PR: 计划 030，`plugins/strata-storage`
- 当前状态: 未测试
- 未完成原因: 没有 Windows 会话打开官方存储页。选择存储位置之前的 Linux 桥接测试返回 `enabled` false 和 `storage` `unavailable`，并且不创建插件目录。该测试不关闭本行。页面发送 `storage-status`、`storage-configure`、`storage-decline`、`storage-propose` 和 `storage-confirm`。它不发送 `enable` 或 `grant`。本项是 OI-059，因为 OI-054 仍是计划 029 的页面。
- 负责 PR: 030
- 交接所有者: 仓库所有者 zemeng5208
- 验证环境: Windows，已选择存储，并打开官方页面 `/storage`
- 验收标准: 该会话显示用量和等待状态，保存一次阈值，拒绝一次，并且只通过宿主命令确认删除
- 关闭条件: 该会话被记录。Linux 空闲测试不关闭它。它不关闭 [OI-001](#oi-001)、[OI-024](#oi-024) 或 [OI-058](#oi-058)。这不是豁免。

<a id="oi-060"></a>

## OI-060

- 关联需求: [REQ-12.02](../roadmap/REQUIREMENTS.md) 和 PL-25
- 来源 PR: 计划 030，`plugins/strata-storage`
- 当前状态: 未实现
- 未完成原因: 脚本和 MCP 可以配置和提案。脚本路径已经实现。`strata.mcp` 属于计划 045，并且没有安装。计划 030 不编造一个 Agent。本项是 OI-060，因为 [OI-056](#oi-056) 仍是计划 029 的 Agent 标记。
- 负责 PR: 045
- 交接所有者: 仓库所有者 zemeng5208
- 验证环境: 计划 045 的运行，已安装 `strata.mcp` 并调用存储能力
- 验收标准: MCP 调用方可以配置和提案，删除仍然需要受信任宿主的确认
- 关闭条件: 该运行被记录。计划 030 的脚本路径不关闭它。这不是豁免。它不关闭 [OI-056](#oi-056)。

<a id="e2e-17"></a>

## E2E-17

- 关联需求: [E2E-17](../roadmap/ACCEPTANCE.md#e2e-17)
- 来源 PR: 2026-11-28 的所有者决定
- 当前状态: 未完成
- 未完成原因: main 上没有第三方 GUI 插件
- 负责 PR: 无路线图行
- 交接负责人: 仓库所有者 zemeng5208
- 验证环境: 真实 Windows，有一个第三方 GUI 插件，官方 GUI 仍安装
- 验收标准: 场景文本
- 关闭条件: 该运行被记录。计划 016 的 E2E-11 不关闭它。

<a id="e2e-18"></a>

## E2E-18

- 关联需求: [E2E-18](../roadmap/ACCEPTANCE.md#e2e-18)
- 来源 PR: 2026-11-28 的所有者决定
- 当前状态: 未完成
- 未完成原因: 没有实现第三方 GUI 的可信宿主提示
- 负责 PR: 无路线图行
- 交接负责人: 仓库所有者 zemeng5208
- 验证环境: 真实 Windows，救援和删除尝试使用隔离宿主
- 验收标准: 场景文本
- 关闭条件: 该运行被记录

<a id="e2e-19"></a>

## E2E-19

- 关联需求: [E2E-19](../roadmap/ACCEPTANCE.md#e2e-19)
- 来源 PR: 随 AG-64 建立的来源登记。`0accacb` 上的产品表是空的。
- 当前状态: 未完成
- 未完成原因: 本提交没有官方 GUI 文件
- 负责 PR: 加入这些文件的变更。不新增路线图行。
- 交接负责人: 仓库所有者 zemeng5208
- 验证环境: 包含官方 GUI 或官方插件界面的提交
- 验收标准: 场景文本
- 关闭条件: 登记与文件相符。空表不关闭它。

<a id="oi-047"></a>

## OI-047

- 关联需求: [REQ-08.02](../roadmap/REQUIREMENTS.md) 和 PL-13
- 来源 PR: 计划 025，`plugins/strata-health`。这一行原来是 OI-040，随后是 OI-044。现在是 OI-047，因为 [PR #34](https://github.com/zemeng5208/winstrata/pull/34) 的头 `d90fb398e8d700e814e19f7737417f0336a988be` 使用 OI-044 和 OI-045，[PR #33](https://github.com/zemeng5208/winstrata/pull/33) 的头 `e4f2eb637f7d575167ee587629999d81fa6dd0eb` 使用 OI-046。main 占用 OI-043。
- 当前状态: 未实现
- 未完成原因: 计划 025 从插件宿主已附着的总线和调度器以及插件可用性读取故障、队列、延迟和丢失。计划 026 调节本进程的 CPU 和驻留集、已附着的总线和调度器队列，以及一个正在运行的插件的订阅积压，并在调节要求丢失标记时写入 `Lost` 缺口，同时支持暂停和恢复。服务和驱动快照、混合存储容量、磁盘健康和界面页仍然没有。磁盘 I/O、网络、存储和 GUI 刷新限额是 [OI-051](#oi-051)。
- 负责 PR: 022 负责服务和驱动状态，030 和 032 负责存储和磁盘健康，042 负责仪表盘呈现。计划 026 不关闭本行。本计划不修改 `PLAN.md`。
- 交接负责人: 仓库所有者 zemeng5208
- 验证环境: 后续计划自己的运行，使用产生该读数的来源
- 验收标准: 该计划记录来自其自身来源的读数。E2E-08 和 E2E-15 不由计划 025 关闭。
- 关闭条件: 这些记录存在，或所有者收窄 PL-13。计划 025 的队列报告不关闭它。

<a id="oi-048"></a>

## OI-048

- 关联需求: [REQ-08.02](../roadmap/REQUIREMENTS.md) 和 [REQ-05.02](../roadmap/REQUIREMENTS.md)
- 来源 PR: 计划 025。这一行原来是 OI-041，随后是 OI-045。现在是 OI-048，因为 [PR #34](https://github.com/zemeng5208/winstrata/pull/34) 的头 `d90fb398e8d700e814e19f7737417f0336a988be` 使用 OI-044 和 OI-045。
- 当前状态: 已知限制
- 未完成原因: 文件和注册表采集器仍使用计划 009 创建的独立总线。StrataHealth 和 StrataBudget 只读取附着在 `PluginHost` 上的总线和调度器。把那些采集器改到另一条总线会改变它们的投递。计划 026 不改那些总线。
- 负责 PR: 无路线图行。若一条总线必须覆盖采集器队列，由所有者指定后续计划。计划 026 不接收这项工作。
- 交接负责人: 仓库所有者 zemeng5208
- 验证环境: 同时运行文件或注册表采集和 StrataHealth 的主机
- 验收标准: 健康报告包含该采集器的队列和丢失，或所有者记录这些总线保持分开
- 关闭条件: 该证据被记录。计划 025 的已附着总线测试不关闭它。

<a id="oi-042"></a>

## OI-042

- 关联需求: [REQ-08.02](../roadmap/REQUIREMENTS.md)
- 来源 PR: 计划 025
- 当前状态: 已关闭
- 未完成原因: Windows Server 2022 已关闭。头 `4b152ccbaee5346a7f76f55b99ee1efa4c55d0c9` 的文件树是 `2fc04f89c529471eac56736c3f75c480393c68c6`。拉取请求作业检出合并提交 `cf6d7cee42c0f65f873e264713c8a4c137d4c26c`，父提交是 `ac37d400c338fe4364260db76a43eeea6226f792` 和该头，文件树相同。[Windows 工作区检查](https://github.com/zemeng5208/winstrata/actions/runs/46067012974/job/140734668243) 运行 `cargo test --locked --workspace`，通过了 `health_stays_absent_until_the_binary_is_beside_the_runtime`、`live_faults_queues_latency_and_loss_match_the_runtime` 和 3 个健康单元测试。Windows 10 和 Windows 11 客户端仍是 [OI-001](#oi-001)。
- 负责 PR: 客户端运行属于计划 049。这次 Server 2022 结果不关闭 OI-001。
- 交接负责人: 仓库所有者 zemeng5208
- 验证环境: Windows Server 2022，作业 140734668243，`cargo test --locked --workspace`
- 验收标准: 该作业通过 `winstrata-strata-health` 测试，包括实时故障、队列、延迟和丢失测试
- 关闭条件: 该作业已满足。Linux 通过没有关闭它。这次 Server 2022 通过不关闭 OI-001。
