# 平台实现记录与构建隔离规范

[English](../../standards/PLATFORM_ISOLATION.md) · [文档索引](../INDEX.md) · [AG-65](../AGENTS.md#ag-65)

- **文档地位**：[AG-65](../AGENTS.md#ag-65) 对应的权威强制规范，经所有者批准并合并计划 055 后生效。
- **当前代码核对基线**：最初按 `4f257aee948d10af41e7fbddb96d8cfaa46697af`（PR 030 合并后的 `main`）记录；计划 054 在合并计划 055 后同步补充了 Windows 11 AppContainer 运行时隔离、WebView2/托盘修复与实测状态。后续任何修改平台代码、`cfg` 条件编译、平台依赖、构建/打包脚本或平台测试范围的 PR，必须在本 PR 内同步更新本文档。
- **非运行资源边界**：本文档及所有 `docs/` 规范均属于**源码仓库工程规范**，**严禁作为运行资源打入任何最终软件安装包、便携包或运行时交付目录**。
- **当前产品范围**：WinStrata V1.0 产品目标仍为 **Windows 10 与 Windows 11 x64**。仓库中现有的 Linux 实现属于**开发阶段的技术储备**（用于落实 [AG-35](DEPENDENCIES.md#ag-35) 与 [AG-49](GIT_AND_PR.md#ag-49) 要求的双环境构建与验证能力），**不代表正式支持 Linux 产品**；在当前阶段不需要删除，也不需要为此启动额外重构或扩大范围。

---

<a id="section-1"></a>

## 1. 当前开发阶段的平台实现记录

本节依据基线提交 `4f257ae` 与计划 054 的实测修复结果，完整记录目前已经存在的 Windows 与 Linux 专属实现。

### 1.1 公共基础设施、内核、受控服务与协议层

| 模块 / Crate | 核对路径 | Windows 实现 | Linux 技术储备 | 未来 Linux 复用价值 | 已验证事项与未完成事项 |
|---|---|---|---|---|---|
| Core 平台抽象与权限 (`winstrata-core`) | `core/src/platform/mod.rs`、`linux.rs`、`windows.rs`、`windows/security.rs`、`windows/tests.rs` | `NtCreateFile`（`OBJ_DONT_REPARSE`）、`CreateFileW`、私有 DACL/SDDL 属主校验、`SHGetKnownFolderPath`（`LocalAppData`）、`CoCreateGuid`、`GetDiskFreeSpaceExW`、`GetVolumeInformationByHandleW`。 | `openat`（`O_CLOEXEC \| O_DIRECTORY \| O_NOFOLLOW \| O_PATH`）、`mkdirat`（`0o700`/`0o755`）、`fstatvfs`、`/dev/urandom`、`geteuid`、`XDG_STATE_HOME` / `$HOME/.local/state`。 | **高**：路径锚定与防符号链接/重定向语义与 Windows 对齐，可直接审计复用。 | Linux 与 Windows Server 2022 测试已通过；Windows 10/11 客户端（[OI-001](../records/OPEN_ITEMS.md#oi-001)）与 ReFS（[OI-002](../records/OPEN_ITEMS.md#oi-002)）未验证。 |
| Core 加密 (`winstrata-core`) | `core/src/crypto/mod.rs`、`openssl.rs`、`cng.rs` | Windows CNG（`BCrypt*`）实现 AES-256-GCM、PBKDF2-HMAC-SHA256 与系统随机数。 | 动态链接 `libcrypto.so.3`（OpenSSL 3 `EVP_*`、`PKCS5_PBKDF2_HMAC`、`HMAC`）。 | **高**：双端已通过同一组公开密码学测试向量。 | 双端向量测试通过；密钥轮换崩溃安全（[OI-008](../records/OPEN_ITEMS.md#oi-008)）及 Linux 发行版 OpenSSL ABI 打包约束未验证。 |
| Core 文件、存储、范围、迁移与混合存储 (`winstrata-core`) | `core/src/files.rs`、`storage.rs`、`scope.rs`、`migration.rs`、`hybrid/mod.rs`、`hybrid/tests.rs` | `GetFileInformationByHandle`、`FILE_FLAG_OPEN_REPARSE_POINT`、`CompareStringOrdinal`、`GetShortPathNameW`、NTFS 备用流校验。 | `O_NOFOLLOW \| O_NONBLOCK`、`0o600` 私有文件、`dev`/`ino` 校验、POSIX 路径比较。 | **高**：混合存储分段、索引、加密包格式与迁移校验可在 Linux 复用。 | Linux 存储与迁移测试通过；ReFS（[OI-002](../records/OPEN_ITEMS.md#oi-002)）、跨设备迁移及 [E2E-10](../roadmap/ACCEPTANCE.md#e2e-10) 未完成。 |
| Core 文件与注册表监视 (`winstrata-core`、`winstrata-kernel`) | `core/src/file_registry/mod.rs`、`windows.rs`、`kernel/src/file_registry.rs` | 用户态 `ReadDirectoryChangesW` 与 `RegNotifyChangeKeyValue` 按需监视（不加载 `strata-fs.sys`/`strata-reg.sys`）。 | `CollectHost::start` 返回 `CollectError::Unsupported`，`stop` 返回 `Ok(())`。 | **中**：`kernel/src/file_registry.rs` 引用计数与健康状态机可复用；Linux `inotify`/`fanotify` 监视尚未实现。 | Server 2022 监视测试通过；已签名驱动加载与 [E2E-02](../roadmap/ACCEPTANCE.md#e2e-02) 仍未完成（[OI-007](../records/OPEN_ITEMS.md#oi-007)）。 |
| Core 与内核进程/系统采样 (`winstrata-core`、`winstrata-kernel`) | `core/src/process.rs`、`schedule.rs`、`kernel/src/system.rs`、`system/linux.rs`、`system/windows.rs` | `Toolhelp` 快照与 SCM 服务 PID 关联。 | 读取 `/proc`、`/etc/machine-id`、`/proc/sys/kernel/random/boot_id`、`/proc/stat` `btime` 及 `/proc/<pid>/cgroup` 中的 `.service` 单元。 | **高**：进程树构建、PID 复用启动时间去重及 cgroup 服务映射已在 Linux 实际跑通。 | `ERR-005` 进程退出采样已在 Server 2022 修复重测；已签名 `strata-proc.sys`、ETW 与命令行采集仍开放（[OI-015](../records/OPEN_ITEMS.md#oi-015)）。 |
| 内核网络来源 (`winstrata-kernel`、`winstrata-core`) | `kernel/src/net/mod.rs`、`linux.rs`、`windows.rs`、`parse.rs`、`core/src/network.rs` | IP Helper 连接表与 WinSock 原始套接字（未标记包仅留内存）。 | `AF_PACKET` `SOCK_RAW` + `/proc/net` + sysfs 接口枚举（无 `CAP_NET_RAW` 时记录 `capture_inaccessible` 缺口）。 | **高**：公共报文解析与内存保留策略共享；Linux 抓包链路已具备基础框架。 | 回环与接口测试通过；`strata.net`（[PR 031](../roadmap/PLAN.md#pr-031)）、`strata-net.sys` 与 [E2E-04](../roadmap/ACCEPTANCE.md#e2e-04) 待完成。 |
| 服务控制与受控 IPC (`winstrata-kernel`、`winstrata-service`) | `kernel/src/scm.rs`、`services/src/transport.rs`、`transport/linux.rs`、`transport/windows.rs`、`services/src/tests.rs` | Windows SCM 驱动枚举/探测与命名管道（`SECURITY_IMPERSONATION` 与受限令牌拒绝）。 | `$XDG_RUNTIME_DIR/winstrata/service.sock` Unix 域套接字（`0o700`），`Privilege::NotWindows`（无驱动控制权限）。 | **中**：帧协议、审计页与订阅引用计数可复用；Linux 提权守护进程（如 systemd/polkit/capabilities）尚未实现。 | Linux 套接字与 Server 2022 管道测试通过；真实 SCM 服务注册、驱动加载（[OI-007](../records/OPEN_ITEMS.md#oi-007)）与 [E2E-14](../roadmap/ACCEPTANCE.md#e2e-14) 未完成。 |
| 协议契约 (`winstrata-protocols`) | `protocols/Cargo.toml`、`protocols/src/*`、`protocols/tests/windows_event.rs`、`interop.rs` | 跨平台 JSON/帧协议契约 + 真实 Windows 事件日志读取测试（`windows_event.rs`）。 | Rust 与 Node 跨语言互操作测试在 Linux 通过。 | **高**：协议定义完全跨平台通用。 | Linux 与 Server 2022 均已验证。 |

### 1.2 官方插件与宿主集成 (`plugins/strata-*`、`core/src/plugins/`)

| 插件 / 模块 | 核对路径 | Windows 实现 | Linux 技术储备 | 未来 Linux 复用价值 | 已验证事项与未完成事项 |
|---|---|---|---|---|---|
| `strata.process` (`winstrata-strata-process`) | `plugins/strata-process/src/*`、`tests/observe.rs`、`core/src/plugins/process.rs` | 进程外采集器调用 `winstrata_kernel::system::capture()`。 | 同一二进制通过 `kernel/src/system/linux.rs` 读取 `procfs`；`#[cfg(unix)]` 清空环境与会话管道辅助。 | **高**：真实 Linux 子进程生命周期、线程/模块与进程树追踪已实现。 | Linux 与 Server 2022 测试通过；ETW、命令行及 Windows 10/11 客户端仍开放（[OI-015](../records/OPEN_ITEMS.md#oi-015)、[OI-001](../records/OPEN_ITEMS.md#oi-001)）。 |
| `strata.registry` (`winstrata-strata-registry`) | `plugins/strata-registry/src/lib.rs`、`windows.rs`、`core/src/plugins/registry.rs` | 进程内 `RegNotifyChangeKeyValue` 监视最多 4 个 HKCU/HKLM 键。 | 启动返回 `RegistryError::Unsupported` / `unsupported`，不生成事件。 | **低（OS 专属）**：仅跟踪/事件模型可复用；Linux 无 Windows 注册表（未来需另行设计 dconf/gsettings 等对应范围）。 | Server 2022 测试通过（本机普通权限写 HKLM 会报错误码 5）；[E2E-02](../roadmap/ACCEPTANCE.md#e2e-02) 未完成。 |
| `strata.env` (`winstrata-strata-env`) | `plugins/strata-env/src/lib.rs`、`windows.rs`、`core/src/plugins/env.rs` | 进程内 HKCU/HKLM 环境存储与 PATH 变更监视（`windows.env_notify`）。 | 启动返回 `EnvError::Unsupported` / `unsupported`，不生成事件。 | **中**：变量与 PATH 分段差异模型可复用；Linux `/etc/environment`、shell 与 systemd 用户环境监视未实现。 | Server 2022 测试通过（本机普通权限写 HKLM 会报错误码 5）；[OI-039](../records/OPEN_ITEMS.md#oi-039)、[E2E-02](../roadmap/ACCEPTANCE.md#e2e-02) 未完成。 |
| `strata.trace` (`winstrata-strata-trace`) | `plugins/strata-trace/src/lib.rs`、`windows.rs`、`core/src/plugins/trace.rs` | 进程内 `ReadDirectoryChangesW` 元数据/SDDL 监视与基线关联投影（`winstrata.trace`）。 | 启动返回 `TraceError::Unsupported` / `unsupported`，不生成事件。 | **中**：关联投影与历史查询逻辑可复用；Linux `fanotify`/`inotify` 采集器尚未实现。 | Server 2022（`ERR-019` 已解决）与 Linux 投影测试通过；`strata-fs.sys` 与 [E2E-02](../roadmap/ACCEPTANCE.md#e2e-02) 未完成。 |
| `strata.services` (`winstrata-strata-services`) | `plugins/strata-services/src/lib.rs`、`windows.rs`、`linux.rs`、`session.rs`、`tests/scm.rs` | `EnumServicesStatusExW` 与 `QueryServiceConfigW` 只读快照及受限令牌测试。 | `linux.rs` 读取 `/proc/*/cgroup` `.service` 与 `/proc/modules`（`Coverage::ActiveOnly`，`evidence = "procfs"`）。 | **中高**：状态差分追踪器可复用；未启动的单元文件及非 systemd 初始化系统尚未覆盖。 | `ERR-020` 已解决；**`ERR-021` 仍未解决**（Windows `service.start-type` 计数为 0）；[OI-043](../records/OPEN_ITEMS.md#oi-043) 开放。 |
| `strata.startup` (`winstrata-strata-startup`) | `plugins/strata-startup/src/lib.rs`、`windows.rs`、`linux.rs`、`parse.rs`、`tests/probes.rs` | Run/RunOnce/PolicyRun、启动文件夹、StartupApproved 与任务计划程序 XML。 | `linux.rs` 与 `parse.rs` 解析 XDG autostart `.desktop`、`/etc/crontab`、`/etc/cron.d`、`/etc/cron.*`、用户 crontab 及 systemd 系统/用户 `.timer` 文件。 | **高**：Linux 自启动/定时任务解析器已完成并有独立测试。 | Linux 测试通过；Server 2022 探测已记录（[OI-049](../records/OPEN_ITEMS.md#oi-049)），Windows 10/11 客户端仍待验证。 |
| `strata.pulse` (`winstrata-strata-pulse`) | `plugins/strata-pulse/src/platform.rs`、`linux.rs`、`windows.rs`、`sample.rs` | `GetSystemTimes`、`GlobalMemoryStatusEx`、`CallNtPowerInformation`、`GetSystemPowerStatus`、`ToolHelp`、`GetProcessMemoryInfo`（GPU 标为 `reader_absent`）。 | `linux.rs` 读取 `/proc/stat`、`/proc/meminfo`、`/proc/loadavg`、sysfs CPU/GPU/电池/传感器、RAPL 与 `perf_event_open`。 | **极高**：已具备完整的 Linux 原生硬件与资源计数器采集后端。 | Linux 与 Server 2022 单元/观测测试通过；Windows 负载测试（[OI-046](../records/OPEN_ITEMS.md#oi-046)）及 GPU/磁盘未完成。 |
| `strata.budget` (`winstrata-strata-budget`) | `plugins/strata-budget/src/regulate.rs`、`tests/regulate.rs`、`core/src/plugins/budget.rs` | Core 内使用 `GetProcessTimes` 与 `GetProcessMemoryInfo`；插件调控主循环跨平台。 | `/proc/self/stat`、`/proc/self/status`（`VmRSS`）、`sysconf(_SC_CLK_TCK)`、`0o600` 私有文件（`#[cfg(unix)]`）。 | **高**：限额调控与自我测量逻辑双端共用。 | Linux 测试通过；Windows 限额测试（[OI-050](../records/OPEN_ITEMS.md#oi-050)）与磁盘/网络/GUI 刷新限额（[OI-051](../records/OPEN_ITEMS.md#oi-051)）未完成。 |
| `strata.script` (`winstrata-strata-script`) | `plugins/strata-script/src/contain.rs`、`execute.rs`、`workbench.rs`、`tests/workbench.rs` | 启动 Python/JS/TS 脚本子进程；**脚本子进程的 Windows AppContainer 隔离尚未在此实现**（[OI-045](../records/OPEN_ITEMS.md#oi-045)）。 | `contain.rs` 构建 Landlock 规则集（ABI 1–5）并注入 `LD_PRELOAD` `winstrata_shim`；`#[cfg(unix)]` 清空继承环境。 | **高**：Linux 脚本隔离链路已真实执行。 | Linux 与 Server 2022 工作台测试通过（`ERR-022`、`ERR-024` 已解决）；[OI-044](../records/OPEN_ITEMS.md#oi-044) 与 [OI-045](../records/OPEN_ITEMS.md#oi-045) 开放。 |
| `strata.storage` (`winstrata-strata-storage`) | `plugins/strata-storage/src/lib.rs`、`windows.rs`、`tests/cleanup.rs` | `windows.rs` 使用显式属主与受保护 DACL 创建私有文件。 | `#[cfg(unix)]` 以 `mode(0o600)` 写入私有文件。 | **高**：阈值阶梯与索引清理逻辑完全跨平台。 | `ERR-025` 与 `OI-058` 仅在指定 Server 2022 提交关闭；[OI-024](../records/OPEN_ITEMS.md#oi-024)、[OI-057](../records/OPEN_ITEMS.md#oi-057)、[OI-059](../records/OPEN_ITEMS.md#oi-059)、[OI-060](../records/OPEN_ITEMS.md#oi-060) 开放。 |
| `strata.filter`、`strata.tag`、`strata.health` | `plugins/strata-filter/`、`plugins/strata-tag/`、`plugins/strata-health/` | 核心协议与状态逻辑跨平台；Windows CI 验证编译与集成测试。 | `session.rs` / `report.rs` / `tests/` 含 `#[cfg(unix)]` 测试与管道/进程辅助。 | **高**：业务逻辑已平台无关。 | Server 2022 对应送审头测试通过；界面页与 MCP 联动仍开放（[OI-053](../records/OPEN_ITEMS.md#oi-053)、[OI-054](../records/OPEN_ITEMS.md#oi-054)、[OI-056](../records/OPEN_ITEMS.md#oi-056)、[OI-047](../records/OPEN_ITEMS.md#oi-047)）。 |

### 1.3 语言运行时、桌面界面与工具链

| 领域 | 核对路径 | Windows 实现 | Linux 技术储备 | 未来 Linux 复用价值 | 已验证事项与未完成事项 |
|---|---|---|---|---|---|
| 语言运行时隔离与垫片 (`winstrata-core`、`winstrata-runtime`、`winstrata-shim`) | `core/src/plugins/language.rs`、`sandbox.rs`、`runtimes/Cargo.toml`、`runtimes/shim/src/lib.rs`、`runtimes/tests/languages*.rs` | AppContainer 配置文件 + Job Object（`kill-on-close`、单活动进程）；`Toolchain::bind` 解析 `sys._base_executable`；非继承顶层/标准库前缀 DACL 授权（`grant_python_prefix`、`grant_node_prefix`）、`SE_DACL_PROTECTED` 节点单独补授及 `revoke_all` 撤回；`winstrata-shim` 仅在 Linux 有实现（`#[cfg(target_os = "linux")]`），Windows 不加载。 | Landlock 规则集 + `LD_PRELOAD` `libwinstrata_shim.so` + seccomp-bpf（`AUDIT_ARCH_X86_64`，拒绝套接字/`execve`/`fork`/挂载/命名空间）+ `close_range`。 | **高**：x86_64 Linux 隔离已通过 `languages.rs` 真实限制验证。 | Linux 隔离已验证；非 x86_64 Linux 架构未实现（`fail(b'x')`）；计划 054 修复 [ERR-026](../records/modules/core.md#err-026) 并在 Windows 11 通过 `fixture.pywin` 与固定版本校验（`languages_windows` 2/2）；`node.exe` 在受限桌面 DACL 下报 `0xC0000142`（[ERR-028](../records/modules/core.md#err-028)、[OI-019](../records/OPEN_ITEMS.md#oi-019)、[OI-062](../records/OPEN_ITEMS.md#oi-062)）；[OI-020](../records/OPEN_ITEMS.md#oi-020) 与 `runtimes/Cargo.toml` 无条件 `dev-dependencies` `libc` 已记账。 |
| 官方默认桌面宿主 (`winstrata-desktop`、`packages/ui`) | `apps/desktop/src/main.rs`、`lib.rs`、`host.rs`、`bridge.rs`、`prefs.rs`、`tests/official_gui.rs`、`packages/ui/*` | Win32 窗口 + 系统托盘 + WebView2（`1.0.2903.40` ABI、AMD64 PE 头校验 + `vswhere.exe` `CREATE_NO_WINDOW` 查找、`ICoreWebView2Controller` COM 生命周期 + `WM_SIZE` + `WM_PAINT` 回退地址绘制 + 待确认存储托盘提示）+ `BCryptGenRandom` + `LocalAppData` 偏好。 | 仅本地 HTTP 桥接 + `XDG_STATE_HOME` + `/dev/urandom`；`main.rs` 在标准输入阻塞，**无 Linux 原生窗口或 WebView 宿主**。 | **中**：HTTP 桥接、偏好配置与 `packages/ui/dist` 前端包可复用；Linux 桌面窗口与托盘宿主未实现。 | Linux 无头 Chrome 桥接检查通过；计划 054 修复 [ERR-027](../records/modules/desktop.md#err-027) 并在 Windows 11 普通用户下关闭 [OI-013](../records/OPEN_ITEMS.md#oi-013) 与 [OI-057](../records/OPEN_ITEMS.md#oi-057)（`winstrata-desktop` 11/11 与 Edge WebView2 `158.0.4370.45`）；Windows 10（[OI-001](../records/OPEN_ITEMS.md#oi-001)）、[OI-009](../records/OPEN_ITEMS.md#oi-009) 与 [E2E-11](../roadmap/ACCEPTANCE.md#e2e-11) 仍开放。 |
| 开发工具与交叉构建 (`winstrata-dev`、`.github/`) | `toolchains/dev/src/main.rs`、`cross_probe.rs`、`toolchains/windows/*`、`.github/scripts/*`、`.github/workflows/*` | Windows CI 运行 `cargo test --locked --workspace`；Windows 上执行 `cargo dev build` 会因正在运行的 `winstrata-dev.exe` 自锁报 `os error 5`。 | `cargo dev cross-probe` 在 Linux 用 `rust-lld` + `objdump` 验证 PE32+ 控制台与 `.sys` 驱动链接；Linux CI 运行 `check-workspace.sh`。 | **高**：已统一工具链版本锁定（`Rust 1.100.0`、`Node 24.22.3`、`pnpm 11.25.0`）。 | Linux `cross-probe` 与 CI 通过；Linux 完整构建 Windows 应用/驱动及 Windows 宿主执行 `cargo dev` 仍待完成（[OI-010](../records/OPEN_ITEMS.md#oi-010)、[OI-011](../records/OPEN_ITEMS.md#oi-011)、[OI-012](../records/OPEN_ITEMS.md#oi-012)）。 |

### 1.4 平台门控依赖汇总 (`Cargo.toml`)

- **仅 Windows 依赖（`[target.'cfg(windows)'.dependencies]` / `dev-dependencies`）**：`windows-sys = "=0.61.2"`，分布于 `apps/desktop`、`core`、`kernel`、`services`、`plugins/strata-env`、`plugins/strata-pulse`、`plugins/strata-registry`、`plugins/strata-services`、`plugins/strata-startup`、`plugins/strata-storage`、`plugins/strata-trace`。
- **仅 Linux / Unix 依赖（`[target.'cfg(target_os = "linux")'.dependencies]` 或 `cfg(unix)`）**：`libc = "=0.2.191"`，分布于 `core`（`linux`）、`kernel`（`linux`）、`services`（`linux`）、`plugins/strata-pulse`（`linux`）、`apps/desktop`（`unix`）、`plugins/strata-script`（`unix`）、`plugins/strata-startup`（`unix`）；以及 `core/src/crypto/openssl.rs` 动态链接 `libcrypto.so.3`（`linux`）。
- **已记录的依赖卫生待项**：`runtimes/Cargo.toml` 目前在 `[dev-dependencies]` 下无条件声明 `libc.workspace = true`（仅供 `#![cfg(unix)]` 测试 `runtimes/tests/languages.rs` 使用）。它不影响发布二进制依赖树，后续清理运行时或打包依赖时应移至 `[target.'cfg(unix)'.dev-dependencies]`。

---

<a id="section-2"></a>

## 2. 未来构建与打包隔离要求

后续所有编译、链接、依赖解析、打包及最终交付验证，必须严格执行以下目标平台隔离约束：

1. **按目标平台分别编译、链接、管理依赖及打包**：
   - Windows 与 Linux 必须依据各自显式目标三元组（例如 `x86_64-pc-windows-msvc` 与未来 Linux 目标）独立执行依赖解析、编译、链接与打包。
2. **最终交付产物零跨平台污染**：
   - 任何平台的最终交付产物均不得包含其他平台专属的无用代码、二进制文件、运行库、依赖及资源：
     - **Windows 交付产物**不得包含 `libwinstrata_shim.so`、ELF 目标/二进制、`libc` 或 `libcrypto.so.3` 运行依赖、Linux 专属脚本/单元文件，以及源码仓库工程规范文档（`docs/`、`AGENTS.md`、`PLATFORM_ISOLATION.md` 等不得打入运行时安装包）。
     - **未来 Linux 交付产物**不得包含 `.exe`、`.dll`、`.sys`、`.inf`、`.cat` 文件、`windows-sys` / WebView2 / WDK 产物，或无用 Windows 专属资源。
3. **公共逻辑共享与零额外开销边界**：
   - 公共能力（`winstrata-protocols`、有界调度、事件总线、混合存储格式、共享追踪状态机）允许复用，但不得因跨平台共享而引入冗余运行时开销、不必要抽象层或额外后台服务/进程。
4. **最终交付必须实测验证隔离效果**：
   - 最终交付时（[PR 050](../roadmap/PLAN.md#pr-050) 及未来 Linux 发布前）必须提供真实产物级的隔离验证证据：
     - **实际产物清单核对**：逐文件核验打包输出清单与平台白名单（确认非运行文档、测试与异构平台文件被排除）。
     - **依赖树分析**：运行 `cargo tree --target <目标三元组> -e normal`，确认生产依赖树不存在异构平台 crate（Windows 不含 `libc`/OpenSSL；Linux 不含 `windows-sys`）。
     - **二进制与导入表检查**：Windows 产物检查 PE32+ 头与导入表（`objdump`/`dumpbin`），Linux 产物检查 ELF 动态依赖（`ldd`/`readelf`）。
     - **运行性能与体积实测**：对照 [ACCEPTANCE.md §10](../roadmap/ACCEPTANCE.md#section-10) 指标实测（完整运行产物 <= 250 MB、后台内存 <= 150 MB、CPU <= 1%、GUI 内存 <= 350 MB、冷启动 <= 3 秒）。
5. **机器可验证的自动化检查**：
   - 构建及交付工具（`toolchains/dev`、`.github/scripts/` 及发布打包脚本）必须把可机器验证的平台隔离要求落实为自动检查（目标依赖树断言、产物白名单与异构二进制扫描、非运行文档排除校验），不得仅依赖 Agent 阅读文档后的自觉执行。该自动化门禁建设已在 [OPEN_ITEMS.md](../records/OPEN_ITEMS.md#oi-061) 中记为 `OI-061`，归口计划 050 验收。

---

<a id="section-3"></a>

## 3. 强制主动读取与追溯机制

依据 [AG-65](../AGENTS.md#ag-65)，根 [AGENTS.md](../../../AGENTS.md) 与[中文根规范](../AGENTS.md)已设立本规范的**强制主动阅读入口**（不是普通参考链接）。

### 3.1 必须主动读取的 6 类触发任务

所有主 Agent、开发 Agent 和子 Agent 在执行以下任一任务**开始前**，必须主动读取最新版本的本文档：

1. 涉及平台专属功能的开发或修改；
2. 修改平台接口、公共接口及平台依赖；
3. 编译、交叉编译及构建；
4. 打包、发布及最终交付；
5. 正式启动未来 Linux 版本的开发；
6. 平台兼容性检查及性能优化。

### 3.2 强制执行纪律

- **不得依赖提醒**：不得等待或依赖所有者主动提醒才读取本文档。
- **不得依赖旧记忆**：不得依赖历史对话、上下文摘要或 Agent 临时记忆代替重新读取最新已提交文档。
- **每次任务前重读并注明版本**：每次相关任务开始前读取仓库最新版本，并在工程记录（[STATUS.md](../records/STATUS.md)、模块记录、会话交接或 PR 说明）中明确注明所依据的文档版本或基线提交 SHA。
- **自动化门禁兜底**：可机器验证的隔离约束必须进入构建与交付工具链（见 [§2.5](#section-2)），不得只靠文档约束。

---

<a id="section-4"></a>

## 4. 长期维护与未来 Linux 接续机制

1. **随源码同步更新**：后续只要有 PR 新增、删除、重构或变更平台专属实现、`cfg` 条件编译、平台依赖、构建/打包脚本或平台测试状态，必须在同一 PR 内同步更新中英文版 `PLATFORM_ISOLATION.md`（[AG-06](DOCUMENTATION.md#ag-06)、[AG-65](../AGENTS.md#ag-65)）。
2. **未来启动 Linux 版本的接续流程**：未来正式启动 Linux 版本开发时，必须：
   - 首先读取本文档；
   - 对照届时最新的 `main` 分支逐项审计 [§1](#section-1) 所列 Linux 技术储备；
   - 优先复用经过核验的有效代码（如 `core/src/platform/linux.rs`、`core/src/crypto/openssl.rs`、`kernel/src/system/linux.rs`、`kernel/src/net/linux.rs`、`runtimes/shim`、`plugins/strata-pulse/src/linux.rs`、`plugins/strata-startup/src/linux.rs`），避免重复开发；
   - 禁止未经真实环境重新验证就直接沿用过时或不完整的储备实现。
3. **云端主分支持续生效**：本文档与根 `AGENTS.md` 中的强制阅读入口必须通过正式 PR 审核并合并至远程仓库 `main` 分支（[AG-12](GIT_AND_PR.md#ag-12)、[AG-60](../AGENTS.md#ag-60)），确保所有本地、云端及后续新会话无需所有者重复提醒即可直接读取并持续生效。

---

<a id="section-5"></a>

## 5. 本次任务边界

- 本次变更（计划 055）仅建立独立的双语平台规范文档及长期强制读取机制，并遵循现有规范审批流程；
- 不删除任何已有 Linux 代码，不启动大规模重构，不要求立即完成 Linux 产品版本，不改变当前 Windows 10/11 x64 产品开发目标，也不干扰并行推进中的其他开发分支；
- 最终交付阶段（计划 050）必须落实本文档规定的全部平台隔离验证及自动化门禁验收要求。
