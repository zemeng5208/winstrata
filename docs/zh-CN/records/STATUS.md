# 项目进度与工程记录

[English](../../records/STATUS.md) · [文档索引](../INDEX.md)

<a id="baseline"></a>

## 仓库基线

2026-11-27，所有者创建公开仓库 [zemeng5208/winstrata](https://github.com/zemeng5208/winstrata)。Git 和 GitHub API 已核验默认分支 `main`、提交 `fafe461abadfdee477ffef14f8bae549bddd5872`、初始 README 和 Apache-2.0 许可证。

所有者保存了 [Protect main](https://github.com/zemeng5208/winstrata/rules/28217627)。GitHub 分支及生效规则接口确认已激活保护，要求经 PR、解决审查对话，阻止强推和删除分支。纯文档基线阶段尚无强制状态检查。个人维护阶段由所有者审核并合并。PR 002 验证并增设适用的 CI 和顺序检查。

## 进度

| 工作 | 状态 | 证据与下一步 |
|---|---|---|
| PR 000 | 已验收 | 所有者合并 [GitHub PR #1](https://github.com/zemeng5208/winstrata/pull/1)，提交为 `c3e8abf07895edbdd1f84792dbab5c48b00c25e8`。核验 main 与送审文件树一致，分支保护生效。 |
| PR 001 | 已验收 | 所有者合并 [GitHub PR #2](https://github.com/zemeng5208/winstrata/pull/2)，提交为 `e0a3bbf2ec8c6b3f322d3f0379e47618fd129c3c`。合并后的文件树与送审分支一致，Linux 的环境检查、构建和验证通过。 |
| PR 002 | 已验收 | 已核验生效规则及 PR 004 的结果。两项 GitHub Actions 检查和分支保持最新均已强制，见 ERR-004。 |
| PR 003 | 已合并，验收仍有待项 | 所有者将 [GitHub PR #4](https://github.com/zemeng5208/winstrata/pull/4) 合并为 `f8809cc68c97ea841ca02ebe5e8c7a17a6287994`。合并树与送审树一致，Linux 工作区回归和两个 x64 链接探针复验通过。打包、签名及实际驱动加载仍待验证。 |
| PR 004 | 平台可行性验证已验收 | 所有者将 [GitHub PR #5](https://github.com/zemeng5208/winstrata/pull/5) 合并为 `36c12ddf45b6f2eb3b0f3ec401e0942f3fce14ee`。文件树与送审提交一致，合并后 Linux、Windows 及策略检查均通过。 |
| PR 005 | 已验收 | 所有者已合并[公共协议 PR #6](https://github.com/zemeng5208/winstrata/pull/6)。主分支提交 `14a8ec6` 与送审提交 `e446453` 的文件树相同。 |
| PR 006 | 已验收 | 所有者将 [GitHub PR #7](https://github.com/zemeng5208/winstrata/pull/7) 合并为 `6e601ed6dc8bec5769b3f28cc6790be57a58b78d`。合并树 `9cd3c33c49919c7e82f73824f9e5581148d5ff62` 与送审树一致。主分支 CI 和合并策略通过。Windows 10/11 客户端、ReFS 和加密迁移仍未验证。 |
| PR 007 | 已验收 | 所有者将 [GitHub PR #9](https://github.com/zemeng5208/winstrata/pull/9) 合并为 `3e0097a44f54d552662b39ab3621002ebce59505`。父提交为 `6e601ed6dc8bec5769b3f28cc6790be57a58b78d` 与 `a95bb7005d014cc4872b56c78fc869a3e0705540`。文件树为 `faf3878360e09dd679e0f2ebe9aacb019ae37d72`。 |
| PR 008 | 已合并，验收仍有待项 | 所有者将 [GitHub PR #12](https://github.com/zemeng5208/winstrata/pull/12) 合并为 `9f70e62a01bbedcdcbb09af956945eef2e23dcd5`。没有进行已签名驱动的加载或卸载。 |
| PR 009 | 已合并，验收仍有待项 | 所有者将 [GitHub PR #15](https://github.com/zemeng5208/winstrata/pull/15) 合并为 `8fe3014846605a726249f64d58eaca9362eaacd4`。父提交为 `9f70e62a01bbedcdcbb09af956945eef2e23dcd5` 与送审头 `e3d6b609fc9c6b58cf30ebf33c4b3a51261abbf0`。提交 `565599ed4f6125fdfa737d4e83087027211879c2` 已通过 Linux、Windows Server 2022、工作区和合并策略。文件与注册表测试观察到了真实变化。没有运行 Windows 10/11、已签名驱动、故意缓冲区溢出和 E2E-02。 |
| PR 010 | 待审查 | [PR #13](https://github.com/zemeng5208/winstrata/pull/13) 已更新到 main `8fe3014846605a726249f64d58eaca9362eaacd4`。较早的头提交 `62159f88d59233d90e780aca6d8c33737462e705` 通过了 Linux、Windows 和 Workspace 检查。它的 `winstrata/merge-policy` 失败，说明为 “Plan 009 must merge before plan 010”。这次更新自己的检查尚未记录。 |
| PR 011–050 | 规划中 | 按已批准的路线图推进并核验前置条件。计划 011 与本次进程工作分开。 |
| Core 与 PL-01 至 PL-25 | 规划中 | 计划 007 已在 Linux 验证进程内调度和事件总线。计划 008 增加服务控制面。计划 009 增加用户态文件和注册表监视。计划 010 采样进程、线程和模块，不加载驱动。网络采集、存储、插件及其他指标仍按各自计划推进。 |

## PR 000 验证

在 Linux 开发环境中，Pandoc 以 GFM 渲染全部 45 份 Markdown 文档。本地引用检查解析了 1,184 个相对链接及锚点，核对了双语的 90 项原子需求编号及映射、25 个插件编号与名称、61 项 Agent 规范编号、51 个计划编号及直接前置关系和 16 个验收场景编号。Git 检查了暂存差异的空白和范围。Windows 构建、驱动加载、GUI 和性能验证按路线图推进。

## PR 001 验证

在 Linux x86_64 环境，用 Rust 与 Cargo 1.100.0、GCC 14.2、Node.js 24.22.3 和 pnpm 11.25.0 运行 `cargo dev doctor`、`cargo dev init`、`cargo dev build` 和 `cargo dev verify` 均成功。Cargo 实际编译了开发工具。本阶段测试框架报告 0 个单元测试，另已直接执行开发工具和包管理流程。

分别从命令搜索路径移除 Node.js 与 pnpm 时，预检以状态码 1 退出并指出缺失工具。将干净源码副本移动到名称包含空格的目录后，从 Rust crate 子目录运行命令，确认使用迁移后的 Cargo 工作区、构建目录和 pnpm 存储。复用了外部宿主工具链。Windows 宿主执行待验证。Git 忽略检查覆盖 Rust 构建产物、pnpm 安装目录与工作区本地缓存。

## PR 002 验证

在 Linux x86_64，CI 初始化验证了官方 Node 发行包的校验和，安装了固定版本的 pnpm，并加载已批准的 Rust 工具链。7 项 Node 测试覆盖合并顺序、缺陷阻断、所有者确认绑定精确提交，以及测试合并与当前基线校验。本地状态适配测试核对结果与 GitHub Actions 检查记录在同一头提交。工作区脚本执行了策略测试与已批准的开发命令。现有 Protect main 规则生效。托管运行结果和规则激活状态通过 PR 和有效规则核验。

## PR 003 验证

Linux x86_64 使用 Rust 1.100.0、附带的 LLD 23.1.5 和 Binutils 2.44，运行 `cargo dev cross-probe` 两次，生成 COFF 对象并链接控制台与内核 PE32+ 映像，两次产物逐字节一致。验证了真实导入、两个子系统及非零入口，并检查驱动校验和与空签名目录。核验两个官方包的 SHA-512 和提取库摘要。在路径含空格的迁移副本中，分别验证库缺失和摘要不符时失败，恢复获批库后从工具子目录复验通过。映像和微软输入均留在忽略目录。本阶段未运行任何 Windows 程序或驱动。

## PR 004 验证

Windows Server 2022 x64 实测覆盖 CIM、事件日志、Job Object、AppContainer 文件授权与撤销、本机 MCP 子进程和三个上游组件的离线 WebView2 渲染。Linux 复验覆盖工作区、两个链接探针、本机协议、依赖查找和上游组件。所选依赖仅用于临时研究。指标、控件、原始运行链接及限制见[平台报告](../architecture/PLATFORM_FEASIBILITY.md)。

Phase 0 验收范围为实测工具链、已生效的合并门禁和可行性探针。三个工作流均在 PR 004 的实际合并提交上通过。后续阶段的产品及客户端验证见 AS-001 和 AS-002。[Linux CI](https://github.com/zemeng5208/winstrata/actions/runs/45969082288) · [Windows 可行性](https://github.com/zemeng5208/winstrata/actions/runs/45969082284) · [合并策略](https://github.com/zemeng5208/winstrata/actions/runs/45969082364)

## PR 005 验证

在提交 `995d148` 的候选上，Linux 通过了 11 个库测试和 2 个集成测试；Windows Server 2022 x64 还通过了第 14 项真实系统日志往返测试。第二次查询按记录号核对了同一条记录的提供方和原生时间。真实 Node 子进程核验了 PID，Rust 和 Node 交换事件和错误数据，并保留超出 JavaScript 安全整数范围的精度。自审发现默认解码可接受位置数组或数字枚举标签；增加对象与标签检查后，原复现用例在两个系统均通过。两种语言的文档共 6 个 JSON 代码块均通过协议解码、编码和再解码。Linux 另对 Windows MSVC x64 目标执行了测试代码类型检查。[Linux 工作区](https://github.com/zemeng5208/winstrata/actions/runs/45971619572/job/140413162125) · [Windows 协议测试](https://github.com/zemeng5208/winstrata/actions/runs/45971619572/job/140413162795)

合并后复验了工作区、Windows 协议和平台探针。两项必需状态检查及最新基线要求仍生效。[主分支 CI](https://github.com/zemeng5208/winstrata/actions/runs/45972348949) · [Windows 平台](https://github.com/zemeng5208/winstrata/actions/runs/45972349084) · [合并策略](https://github.com/zemeng5208/winstrata/actions/runs/45972348858)

## PR 006 验证

在候选代码 `b3f56ab` 上，Linux 通过 21 项 Core 测试，Windows Server 2022 x64 通过 20 项 Core 测试。两个平台也通过原有协议测试。Windows 以实际受限令牌读取获准的对照文件，访问用户私有文件时被拒绝；测试还覆盖当前 SID、NTFS 权限、联接点、大小写别名和命名数据流。两端验证会话锁随进程退出释放、配置损坏或版本不支持时保留原文件，以及迁移后重读文件并保留原目录。[Linux 工作区](https://github.com/zemeng5208/winstrata/actions/runs/45976058358/job/140427947697) · [Windows 工作区](https://github.com/zemeng5208/winstrata/actions/runs/45976058358/job/140427947209)

Windows 早期候选在替换配置文件时遇到共享冲突，测试也发现长时间锁定公共祖先会影响同级目录更名。修复后，持久会话只持有定位锁文件。NTFS 回归确认私有目录保持固定、普通同级目录仍可改名。缺盘测试使用系统报告未挂载的盘符；空间不足通过真实可用容量与超额写入预算触发，未填满物理磁盘。开发环境还通过两个 Linux 交叉链接探针及 Core 的 rustfmt 检查。整个工作区的格式检查仍报告早于此 PR 的开发工具源码排版差异。Windows 10/11 与 ReFS 的行为尚未实测。

合并后，本地 Git 确认父提交 `14a8ec6b9fda65c8bea3fcaf9d8af1093c830d98` 与 `4fbf9d64bd15aa3236f4ffad0a987a035304b3d7`，文件树为 `9cd3c33c49919c7e82f73824f9e5581148d5ff62`。该提交上的[主分支 CI](https://github.com/zemeng5208/winstrata/actions/runs/45978807885)和[合并策略](https://github.com/zemeng5208/winstrata/actions/runs/45978807889)均成功。加密迁移仍属于后续计划。

## PR 007 验证

在 Linux x86_64、Rust 1.100.0 和 Intel Xeon 主机上，`cargo test --workspace --locked` 通过 27 项 Core 测试、11 项协议库测试和 2 项协议集成测试。新增测试经由调度器与总线发布协议事件，覆盖连续投递、准入等待、满队列拒绝、合并后的 `lost` 与 `filtered` 缺口、跨批次空洞、重复序号、优先级、合作式取消、超时信号、panic 隔离、订阅上限和线程汇合。Linux 确认汇合后的线程 ID 不再带有该调度器的线程名。该检查记录线程启动时间，因此被复用的线程号不算泄漏，并最多等待两秒让内核删除任务项。Release 在 39.925 毫秒内投递 20,000 条观测，约每秒 501,000 条，且无丢失。该测试的 Debug 运行处于 10 秒界限内。`cargo build --locked -p winstrata-core` 没有警告。锁文件只把既有的 `winstrata-protocols` 加入 Core，没有新增外部包。

GitHub Actions 在 `58452bd` 的 Linux 工作区检查中，`unsubscribe_and_shutdown_release_queues_without_keeping_threads` 失败。当时 `live_threads` 已是 0，但汇合后立刻仍有一个线程号带有该调度器的名称。同一提交的 Windows 工作区检查通过，合并策略评估也通过。[Linux 失败记录](https://github.com/zemeng5208/winstrata/actions/runs/45982561210/job/140450010980) 修正后，本机连续八次 `cargo test --locked -p winstrata-core --lib -- --test-threads=16` 通过，其中包括线程释放测试。提交 `0dceae00c8693cd84c6c061538206233622a27da` 上，GitHub 的 Workspace、Linux 工作区和 Windows 工作区检查成功，`winstrata/merge-policy` 的说明为 “Merge order, blockers, and policy files checked”。[工作区运行](https://github.com/zemeng5208/winstrata/actions/runs/45983311774) · [Linux](https://github.com/zemeng5208/winstrata/actions/runs/45983311774/job/140452443114) · [Windows](https://github.com/zemeng5208/winstrata/actions/runs/45983311774/job/140452444036) · [合并策略](https://github.com/zemeng5208/winstrata/actions/runs/45983326195) 这里的 Windows 检查是 GitHub 工作区作业，不是 Windows 10/11 客户端、ReFS、加密迁移或已签名驱动测试。

当前 Node.js 为 v22.23.4，固定版本为 24.22.3，因此未运行 `cargo dev doctor`。verify 所调用的工作区测试命令已直接执行。本机尚未运行计划 007 的 Windows 测试。Windows 10/11 客户端、GUI 延迟、持续压力、动态预算、带认证的 IPC、ReFS 和加密迁移均未执行。E2E-08 与 E2E-15 仍待完成。

所有者将 PR #9 合并为 `3e0097a44f54d552662b39ab3621002ebce59505`，文件树为 `faf3878360e09dd679e0f2ebe9aacb019ae37d72`。该提交上的[主分支 CI](https://github.com/zemeng5208/winstrata/actions/runs/45994424551)和[合并策略](https://github.com/zemeng5208/winstrata/actions/runs/45994424565)均成功。

## PR 008 验证

在 Linux x86_64、Rust 1.100.0 和 Intel Xeon 主机上，`cargo test --workspace --locked` 通过 28 项 Core 测试、6 项内核管理器测试、11 项协议库测试、2 项协议集成测试和 7 项服务测试。服务测试使用真实 Unix 套接字。它们拒绝 `gui`、`plugin` 和 `script` 的订阅，并在 Linux 进程尝试订阅时保持每个模块为 `idle`，同时检查身份、版本、重复键、未知驱动、多余字段、损坏的 JSON 和超过 16 KiB 的帧。服务库测试在 0.01 秒内结束，其中包括 64 次状态往返和随后的审计调用，处于该测试 5 秒界限内。审计页报告丢弃了 1 条记录，且不再保留最旧的 id。同一组内核和服务测试在 `RUSTFLAGS="-Dwarnings"` 下也通过。`cargo check --locked --target x86_64-pc-windows-msvc --tests -p winstrata-kernel -p winstrata-service` 对 Windows 管道、令牌和服务控制管理器测试做了类型检查。这些测试没有执行。`CatalogHost` 返回 `image_absent`，不调用服务控制管理器。没有加载或卸载已签名驱动。锁文件只加入这两个路径包，没有新增外部包版本。

当前 Node.js 为 v22.23.4，固定版本为 24.22.3，因此未运行 `cargo dev doctor`。工作区测试命令已直接执行。

在提交 `4f7dbf1254b647e92d1b06110cc3f4dfe86c8a8d` 上，Linux 工作区检查通过，Windows 工作区检查失败。Windows 通过了 27 项 Core 测试、9 项内核测试和协议测试。这 9 项内核测试包括枚举现有驱动服务、拒绝启动指向不存在文件 `C:\Windows\Temp\WinStrata008-missing.sys` 的 `WinStrata008MissingImage`、删除该探针，以及拒绝受限令牌的 SCM 创建权限。四项服务管道测试失败。三个客户端在 `listen` 返回时管道尚不存在，因而得到 Win32 错误 2。第四项测试中，受限令牌被拒绝访问，随后的普通客户端在 200 毫秒等待关闭实例后得到 Win32 错误 232。[Windows 失败](https://github.com/zemeng5208/winstrata/actions/runs/45998537771/job/140503975068) · [Linux 通过](https://github.com/zemeng5208/winstrata/actions/runs/45998537771/job/140503975617) · [工作区失败](https://github.com/zemeng5208/winstrata/actions/runs/45998537771/job/140504351262) · [合并策略](https://github.com/zemeng5208/winstrata/actions/runs/45998537937) 监听器现在会在返回前创建实例，并在轮询期间保持它打开。

在提交 `c056e32389f0e2a75ec0e1407d0e253a5cdde8ab` 上，Linux 工作区检查再次通过。Windows 管道测试已经连上，但在交换数据时失败。三个客户端看到流结束，或看到没有原生代码的传输错误。受限令牌被拒绝后，普通客户端再次得到 Win32 错误 232。客户端没有请求模拟权限，服务因此关闭管道而不是应答。客户端现在以 `SECURITY_IMPERSONATION` 打开管道。[第二次 Windows 失败](https://github.com/zemeng5208/winstrata/actions/runs/45999133804/job/140506021647) · [第二次 Linux 通过](https://github.com/zemeng5208/winstrata/actions/runs/45999133804/job/140506021252) · [第二次工作区失败](https://github.com/zemeng5208/winstrata/actions/runs/45999133804/job/140506340654)

在提交 `fe4b59117f7b3948f2e1a6ecdcc181ab11ea9795` 上，Linux、Windows 和合并策略均成功。Linux 通过 28 项 Core 测试、6 项内核测试、11 项协议库测试、2 项协议集成测试和 7 项服务测试。Windows 通过 27 项 Core 测试、9 项内核测试、11 项协议库测试、2 项协议集成测试、1 项真实系统事件测试和 7 项服务测试。内核测试再次拒绝启动缺失映像，并拒绝受限令牌的 SCM 创建权限。服务测试在命名管道上交换了帧，受限令牌被拒绝访问该管道。[Linux](https://github.com/zemeng5208/winstrata/actions/runs/45999706617/job/140507919189) · [Windows](https://github.com/zemeng5208/winstrata/actions/runs/45999706617/job/140507919585) · [工作区](https://github.com/zemeng5208/winstrata/actions/runs/45999706617/job/140508248861) · [合并策略](https://github.com/zemeng5208/winstrata/actions/runs/45999704275) 该 Windows 作业是 Windows Server 2022。它不是 Windows 10/11 客户端、交互式未提升 GUI，也不是在 Secure Boot 与 HVCI 启用时加载已签名驱动。`winstrata-service` 没有注册到服务控制管理器。没有加载或卸载已签名驱动。E2E-14 与 E2E-15 仍待完成。所有者将本计划作为 [GitHub PR #12](https://github.com/zemeng5208/winstrata/pull/12) 合并为 `9f70e62a01bbedcdcbb09af956945eef2e23dcd5`。文件和注册表采集属于计划 009，已经在 main 上。进程用户态采样属于计划 010。网络采集仍属于 PR 011。已签名驱动加载的验收尚未完成。

## 决策与技术假设

| ID | 状态 | 依据与后续 |
|---|---|---|
| ADR-000 | 所有者已确认 | 以 Apache-2.0 开放 `zemeng5208/winstrata`。初始提交包含许可证并予保留。后续材料带来实际 NOTICE 义务时增设。 |
| ADR-001 | 随 PR 000 生效 | 采用 `docs/` 和 `docs/zh-CN/` 双语关联文档、稳定计划及规范编号、统一工程记录和候选目录布局。 |
| ADR-002 | 所有者已确认 | 固定 Rust 与 Cargo 1.100.0、Node.js 24.22.3 LTS、pnpm 11.25.0。采用 Cargo 和 pnpm 根工作区，本阶段无第三方 Rust 或 JavaScript 软件包。 |
| ADR-003 | 所有者已确认 | 按各自工具许可，在本地开发环境使用官方 Microsoft WDK 与 Windows SDK x64 10.0.26100.6584 导入库验证 Linux 探针。 |
| ADR-004 | 所有者已确认 | 以初版事件、对象、错误及版本契约启动 Phase 1。使用启用 `derive` 的 `serde` 1.0.229、`serde_json` 1.0.154 及其锁定的间接依赖进行 JSON 编解码。[协议契约](../architecture/PROTOCOLS.md) |
| ADR-005 | 所有者已确认 | Windows 以当前令牌 SID 隔离数据，默认使用 D 盘。Local AppData 保存位置指针和会话锁。固定 windows-sys 0.61.2、windows-link 0.2.1 与 Linux 开发依赖 libc 0.2.191。三者均为 MIT OR Apache-2.0；2026-11-27 的 OSV 查询未返回已知公告。[存储接口](../architecture/CORE-STORAGE.md) |
| AS-001 | 部分已验证 | Linux 已链接两个 x64 探针。计划 008 没有增加 INF/CAT 包，也没有加载已签名驱动。在启用 Secure Boot 与 HVCI 的 Windows 上做代表性加载和卸载仍待完成。PR 050 需验证完整 Linux 交叉构建。 |
| AS-002 | 部分已验证 | [平台报告](../architecture/PLATFORM_FEASIBILITY.md)记录 Server 2022 探针结果。计划 008 检查了 Linux 对端凭据，并实现了 Windows 令牌、管道 ACL 和 SCM 拒绝；这些 Windows 检查只完成类型检查，本机没有执行。普通用户 GUI 仍属于 PR 016，混合运行时隔离属于 PR 015，持续性能属于 PR 048，Windows 10/11 客户端属于 PR 049。 |
| AS-003 | 待决策 | 确认计量与清理语义、无标记元数据留存、WebView2 主题、Windows 支持和性能方法。见[待决事项](../roadmap/PLAN.md#section-19)。 |
| AS-004 | 已记录的约束 | 计划 007 的工作者池使用 Rust 标准库，因为尚未批准异步运行时包。更换运行时须单独决定依赖。[Core 运行时](../architecture/CORE-RUNTIME.md) |
| AS-005 | 已记录的限制 | 服务认证的是操作系统用户，不是进程角色。以服务所有者身份运行的进程可以发送角色 `core`。`GuiClient` 只提供状态查询，服务端拒绝 `gui`、`plugin` 和 `script` 的订阅、释放和审计。插件能力隔离仍属于 PR 014 和 PR 015。[服务 IPC](../architecture/SERVICE-KERNEL.md) |
| AS-006 | 已记录的限制 | 计划 009 在用户进程中使用 `ReadDirectoryChangesW` 和 `RegNotifyChangeKeyValue`。这些 API 不报告变更时刻的精度，也不报告操作进程，因此省略 `precision_ns` 和进程字段。没有加载已签名的 minifilter 或注册表回调。该限制不禁止以后经批准的内核回调。[文件与注册表采集](../architecture/FILE-REGISTRY.md) |
| AS-007 | 已记录的限制 | 计划 010 对进程采样。只在两次采样之间出现并退出的进程不是事件，跟踪器也不为它编造序号缺口。退出时间是采样时间。ETW、内核回调、命令行和服务停止记录不属于本计划。计划 010 分支最初把这项限制记为 AS-006；main 上的 AS-006 已经属于计划 009。[进程来源](../architecture/PROCESS.md) |

## 错误与交接

| ID | 状态 | 发现、影响与复验 |
|---|---|---|
| ERR-000 | 已解决 | 2026-11-27，GitHub HTTPS 开发分支推送返回 403，个人账号下无覆盖该仓库的安装。所有者配置后，开发分支推送试运行成功。 |
| ERR-001 | 所有者配置后已解决 | 集成创建规则集返回 403。所有者保存保护规则，随后查询分支、规则集及有效规则已确认启用。集成仍缺设置权限。 |
| ERR-002 | Linux 工作区已解决 | PR 001 预检发现 PATH 缺少 Rust 与 Cargo。验证 Rustup 安装器校验和后，将已批准的官方工具链安装至项目的忽略目录。工具版本与真实构建均通过验证。 |
| ERR-003 | 已解决 | 所选 x64 WDK 导入库未导出 `KeQueryInterruptTime`，第一次内核映像链接失败。检查归档后改用文档所列的真实导出 `KeQueryTimeIncrement`，重跑两项探针和 PE 检查。 |
| ERR-004 | 已在 PR 004 复验并关闭 | PR 003 合并时策略检查失败，当时仅强制 Workspace checks，最新基线选项关闭。PR 004 的送审提交 `1788410` 通过两项检查。生效规则已要求 `Workspace checks`、`winstrata/merge-policy` 以及分支保持最新，两项检查的来源均为 GitHub Actions，绕过名单为空。[PR 003 策略失败](https://github.com/zemeng5208/winstrata/actions/runs/45964752411) · [PR 003 主分支 CI](https://github.com/zemeng5208/winstrata/actions/runs/45965182726) · [PR 004 策略通过](https://github.com/zemeng5208/winstrata/actions/runs/45968732577) · [生效规则](https://api.github.com/repos/zemeng5208/winstrata/rules/branches/main) |

## PR 009 验证

在 Linux x86_64、Rust 1.100.0 和 Intel Xeon 主机上，`cargo test --workspace --locked` 通过 36 项 Core 测试、13 项内核测试、11 项协议库测试、2 项协议集成测试和 7 项服务测试。新增的 Core 测试解码文件通知、配对重命名、拒绝截断名称、比较注册表值、仅在 SDDL 能放入标识符时保留它、省略 `precision_ns`，并确认 Linux 订阅返回 `unsupported` 且不发布事件。新增的内核测试覆盖一次启动、归零停止、故障隔离、停止失败、64 个订阅上限、不支持的启动、丢弃会话和不安全细节文本。同一组 Core 和内核库测试在 `RUSTFLAGS="-Dwarnings"` 下通过。`RUSTFLAGS="-Dwarnings" cargo check --locked --tests --target x86_64-pc-windows-msvc -p winstrata-core -p winstrata-kernel -p winstrata-service` 完成了 Windows 监视器的类型检查。这些测试没有在本机执行。锁文件只把已有的 `winstrata-kernel` 路径包加入 Core，没有新增外部 crate 版本。Core 启用了已固定的 `windows-sys` 0.61.2 的 `Win32_System_Registry` 特性。

由于当前 Node.js 为 v22.23.4，固定引擎为 24.22.3，没有运行 `cargo dev doctor`。工作区测试命令是直接运行的。

本机 Linux 没有执行的 Windows 测试是 `windows_file_watch_reports_real_changes_without_content`、`windows_registry_watch_reports_exact_values` 和 `windows_registry_failure_leaves_the_file_watch_running`。它们创建、重命名和删除真实文件，拒绝子目录变化，在最后一次释放时停止，写入并删除易失的 HKCU DWORD，并在注册表键缺失时保持文件监视继续运行。没有故意撑满 16 KiB 文件缓冲区。没有加载已签名驱动。没有运行 Windows 10 和 Windows 11 客户端、Secure Boot、HVCI、E2E-02、进程归因和插件呈现。

在提交 `83cd44377fd4588c7706a6f227ca6e819a7e177e` 上，Linux 工作区检查通过，`winstrata/merge-policy` 报告“Merge order, blockers, and policy files checked”。Windows Server 2022 运行 `cargo test --locked --workspace`，`winstrata-core` 为 34 项通过、4 项失败。[Linux](https://github.com/zemeng5208/winstrata/actions/runs/46007410073/job/140533973394) · [Windows 失败](https://github.com/zemeng5208/winstrata/actions/runs/46007410073/job/140533972873) · [工作区失败](https://github.com/zemeng5208/winstrata/actions/runs/46007410073/job/140534358882) · [合并策略](https://github.com/zemeng5208/winstrata/actions/runs/46007410135) `linux_and_invalid_scope_publish_nothing` 用 `/tmp/winstrata-009-unused` 调用 `open`。在 Windows 上该字符串不是绝对路径，因此 `open` 在订阅前返回 `collector_fault` / `invalid collection scope`。注册表断言同时要求 `registry.value_added`、证据 `t=4;a=11000000`，以及 JSON 中未转义的主体 `HKCU\...`。JSON 会转义反斜杠，所以即使字段正确，这个主体检查也不会成功。该失败本身不能证明 DWORD 事件不存在。两项文件测试都等到 8 秒截止，仍没有主体包含新建文件名的事件。`ReadDirectoryChangesW` 把字节数指针放在栈上。重叠完成可能在调用返回后写入该地址。现在改为传入空指针，并由 `GetOverlappedResult` 读取字节数。范围测试已拆开：每个宿主都拒绝相对目录和 `..\Software`，Linux 仍检查订阅返回 `unsupported` 且不发布事件。注册表断言改为比较事件类型、主体标识和证据字段。拆分之后，本机 Linux 用 `cargo test --workspace --locked` 通过 37 项 Core 测试、13 项内核测试、11 项协议库测试、2 项协议集成测试和 7 项服务测试。该修正已用 `RUSTFLAGS="-Dwarnings"` 针对 `x86_64-pc-windows-msvc` 做类型检查，尚未在 Windows 上执行。

在提交 `798b91ff5a394d6fec2820fe0142ecac873fcb35` 上，Linux 工作区检查通过，`winstrata/merge-policy` 再次报告“Merge order, blockers, and policy files checked”。Windows Server 2022 通过 36 项 Core 测试，包括 `invalid_scope_is_rejected` 和 `windows_registry_watch_reports_exact_values`。两项文件测试失败。它们只看到 `gap:SourceFailure`，没有看到新建文件。[Linux](https://github.com/zemeng5208/winstrata/actions/runs/46008317408/job/140537031986) · [Windows 失败](https://github.com/zemeng5208/winstrata/actions/runs/46008317408/job/140537032504) · [工作区失败](https://github.com/zemeng5208/winstrata/actions/runs/46008317408/job/140537559877) · [合并策略](https://github.com/zemeng5208/winstrata/actions/runs/46008333018) 第一次 `ReadDirectoryChangesW` 把完成状态写进随后被移动的 `OVERLAPPED`。`GetOverlappedResult` 读到的是移动后的副本，因此真实目录变化被报成 `source_failure`，监视随之停止。重叠结构、字节数和缓冲区现在留在第一次调用使用的地址上。文件监视停止并不是因为注册表键缺失。

在提交 `565599ed4f6125fdfa737d4e83087027211879c2` 上，Linux 工作区检查、Windows 工作区检查和工作区检查均通过。`winstrata/merge-policy` 报告“Merge order, blockers, and policy files checked”。Windows Server 2022 运行 `cargo test --locked --workspace`，通过 38 项 Core 测试、16 项内核测试、11 项协议库测试、2 项协议集成测试、1 项真实系统事件测试和 7 项服务测试。`windows_file_watch_reports_real_changes_without_content` 创建、重命名并删除了真实文件。`windows_registry_watch_reports_exact_values` 新增、修改并删除了易失的 HKCU DWORD，证据包含 `t=4;a=11000000`。`windows_registry_failure_leaves_the_file_watch_running` 在注册表键缺失时保持文件监视继续运行。内核测试再次拒绝启动缺失映像。[Linux](https://github.com/zemeng5208/winstrata/actions/runs/46009118783/job/140539785857) · [Windows](https://github.com/zemeng5208/winstrata/actions/runs/46009118783/job/140539785271) · [工作区](https://github.com/zemeng5208/winstrata/actions/runs/46009118783/job/140540125932) · [合并策略](https://github.com/zemeng5208/winstrata/actions/runs/46009138740) 该 Windows 作业是 Windows Server 2022。它不是 Windows 10/11 客户端，也不是在 Secure Boot 与 HVCI 下加载已签名驱动，也没有故意撑满 16 KiB 缓冲区。`precision_ns` 仍然省略。进程归因仍属于 PR 010。插件呈现仍属于 PR 021。E2E-02 仍待完成。没有加载已签名驱动。

所有者将本计划作为 [GitHub PR #15](https://github.com/zemeng5208/winstrata/pull/15) 合并为 `8fe3014846605a726249f64d58eaca9362eaacd4`，父提交为 `9f70e62a01bbedcdcbb09af956945eef2e23dcd5` 与 `e3d6b609fc9c6b58cf30ebf33c4b3a51261abbf0`。[主分支 CI](https://github.com/zemeng5208/winstrata/actions/runs/46012223713) 在该提交上成功。合并策略工作流也成功：https://github.com/zemeng5208/winstrata/actions/runs/46012223697。上面的 Windows 证据属于 `565599ed4f6125fdfa737d4e83087027211879c2`。

## PR 010 验证

在 Linux x86_64、Rust 1.100.0 和 Intel Xeon 主机上，`RUSTFLAGS="-Dwarnings" cargo test --workspace` 通过 32 项 Core 测试、9 项内核测试、11 项协议库测试、2 项协议集成测试和 7 项服务测试。新增测试读取真实进程表。一个真实子进程是本次测试进程的子进程，只有一条启动事件；第二次采样仍保持该身份；回收后只有一条退出事件。进程表中存在至少三个进程的链。这些事件发布到 Core 总线后序号连续，且没有记录丢失。内核采样包含当前进程映像、至少一条线程，以及名称含测试可执行文件的模块；两次采样的设备和启动标识相同。本机这次采样有 33 个进程、0 条服务行和 0 条服务事件，服务枚举可用。cgroup 树没有 `.service` 单元，因此服务测试确认没有创建 `process.service` 事件。PID 复用规则把两个真实启动时间先后放到同一个 PID 上。这是跟踪器输入，测试期间操作系统没有回收 PID。

`cargo check --target x86_64-pc-windows-msvc --tests -p winstrata-kernel -p winstrata-core -p winstrata-service` 在拒绝警告的条件下对 Toolhelp、注册表、服务控制管理器、管道和令牌测试做了类型检查。这次本机检查没有执行 Windows 测试。没有加载已签名驱动。由于当前 Node.js 是 v22.23.4，锁定版本是 24.22.3，没有运行 `cargo dev doctor`。工作区测试命令是直接运行的。锁文件把 `winstrata-kernel` 加到 Core，并把 `libc` 加到 kernel 包。这两个 crate 原本已锁定。没有新增外部包版本。

在头提交 `5bff80632ccdf9567d249fcdf8d526bd8f4065f9` 上，GitHub 记录的 Linux 工作区、Windows 工作区和 Workspace 检查均成功。这些作业执行的是测试合并 `a21f6c7964744bcf01642ca82edb64be5ab256e6`。[Linux](https://github.com/zemeng5208/winstrata/actions/runs/46006824539/job/140531964285) 通过 32 项 Core 测试、9 项内核测试、11 项协议库测试、2 项协议集成测试和 7 项服务测试。[Windows](https://github.com/zemeng5208/winstrata/actions/runs/46006824539/job/140531964652) 是 Windows Server 2022，版本 10.0.20348。它通过 31 项 Core 测试、11 项内核测试、11 项协议库测试、2 项协议集成测试、1 项实时系统事件测试和 7 项服务测试。`real_process_tree_pid_lifecycle_and_dedup` 与 `service_associations_are_indirect_and_deduplicated` 在该主机上通过。Windows 服务测试要求服务控制管理器枚举可用、至少有一个 Win32 服务进程、一条推断的 `service:` 事件，并且下一次采样不重复该事件。Cargo 把采样计数留在通过的测试输出里，因此这里不写服务行总数。[Workspace](https://github.com/zemeng5208/winstrata/actions/runs/46006824539/job/140532328842) 在两个平台作业成功后通过。提交状态 `winstrata/merge-policy` 为失败。其发布说明是 “Plan 009 must merge before plan 010”。工作流检查 “Evaluate merge policy from main” 成功完成，并写入了该状态。[策略运行](https://github.com/zemeng5208/winstrata/actions/runs/46006824031)。该说明是路线图顺序门禁，不是工作区测试失败，本 PR 不绕过它。已签名 `strata-proc` 加载、ETW、命令行、服务停止记录、Windows 10/11 客户端运行，以及 E2E-02 和 E2E-03 仍待完成。

## 交接

当前交接点为计划 010，分支 `pr/010-process-sources`，已更新到 main `8fe3014846605a726249f64d58eaca9362eaacd4`。较早的头提交 `62159f88d59233d90e780aca6d8c33737462e705` 通过了 Linux、Windows 和 Workspace 检查，其 `winstrata/merge-policy` 失败，发布说明为 “Plan 009 must merge before plan 010”。这次合并自己的 Linux、Windows、Workspace 和合并策略结果尚未记录。本机没有服务单元。Windows Server 2022 已执行进程树、PID 生命周期、服务间接事件和去重测试。已签名 `strata-proc` 加载、ETW、命令行、服务停止记录、Windows 10/11 客户端运行，以及 E2E-02 和 E2E-03 仍待完成。计划 011 是分开的工作。测试宿主返回的 `ready` 不是已加载的驱动。
