# 文件与注册表采集

[English](../../architecture/FILE-REGISTRY.md) · [会话](../../../kernel/src/file_registry.rs) · [发布](../../../core/src/file_registry/mod.rs) · [计划 009](../roadmap/PLAN.md#pr-009) · [跟踪插件](TRACE.md)

计划 009 观察一个选定目录和 `HKEY_CURRENT_USER` 下的一个子键。监视运行在普通用户进程中，使用 `windows-sys` 0.61.2 已经链接的 Windows API。`CatalogHost` 仍然返回 `image_absent`，不加载 `strata-fs.sys` 或 `strata-reg.sys`。进程与网络采集仍属于 PR 010 和 PR 011。PR 008 的服务 IPC 没有改变，也不是启动这些监视的路径。

## 按需生命周期

`FileRegistrySession` 只为 `files` 和 `registry` 计数。上限与驱动管理器相同，为 64。第一次订阅启动该通道。再次订阅不会重复启动。最后一次释放停止通道并等待线程结束。停止失败时通道保持驻留，下一次订阅不会再启动第二条线程。一个通道故障不影响另一个通道保持就绪。会话被丢弃时，会停止仍处于就绪或停止失败状态的通道。

`FileRegistry::open` 拒绝不是绝对路径或含有控制字符的目录，也拒绝空子键、超过 200 个字符、超过八个组成部分，或包含空段、`.`、`..`、`/`、`:` 或控制字符的子键。错误是 `collector_fault`，细节固定为 `invalid collection scope`。被拒绝的路径不会写入错误。计划 017 可以附上 `CollectionScope`。设置之后，范围外的路径或注册表键不会发布。发布端记录一条过滤缺口，并且不把被拒绝的路径写入其中。没有范围时，计划 009 的行为不变。Linux 的订阅仍然返回 `unsupported`。

在 Linux 上，启动返回 `unsupported`，并且不发布事件。另一通道保持空闲。

## Windows 来源

文件线程对目录句柄调用 `ReadDirectoryChangesW`。监视不递归。缓冲区为 4096 个 `u32`，即 16 KiB。过滤器包括文件名、目录名、属性、大小、最后写入时间和安全描述符。不请求最后访问时间。来源提供者为 `windows.read_directory_changes`。

注册表线程用 `RegOpenKeyExW` 打开一个子键，并以名称和最后写入变化武装 `RegNotifyChangeKeyValue`。监视不递归。每次收到信号后，线程先重新武装，再读取下一份快照并比较。快照在超过 64 个值、名称或数据超过 256 个 UTF-16 字符或 1024 字节、名称不是 UTF-16，或名称重复时记为不完整。不完整快照只发布一条 `registry.key_changed`，部分原因为 `snapshot_incomplete`，不编造键值差异。空差异不发布事件。来源提供者为 `windows.reg_notify`。

只有第一次读取或通知已经挂起之后，才会发出启动完成信号。调用方最多等待 2 秒。若注册表通知在基线快照和第一次武装之间已经触发，线程发布一条 `lost` 缺口并重新取样。这些值不能逐条归因。之后的 `lost` 缺口不会停止监视。读取、武装或快照的硬失败会发布 `source_failure`，记录健康错误并退出线程。`FileRegistry::view` 和 `subscribe` 在健康错误已设置时把就绪通道改为失败。`release` 不这样做，因此失败的线程仍可被停止。

Win32 `ERROR_NOTIFY_ENUM_DIR`（1022）和长度为 0 的成功读取都是没有区间的 `lost` 缺口。`ERROR_OPERATION_ABORTED`（995）是取消。单元测试覆盖了这一分类。本计划没有在真实监视中故意撑满 16 KiB 缓冲区。计划 019 在[注册表插件](REGISTRY.md)中记录当前用户和本机的变更前后归因。计划 009 仍然只监视一个 HKCU 子键。文件插件是计划 021，见[跟踪插件](TRACE.md)。计划 009 的行为不变。计划 021 在原有文件监视测试旁增加了 `windows_file_watch_overflow_records_a_lost_gap`。生产缓冲区仍是 16 KiB。计划 021 的编写主机没有执行这项测试。

## 协议字段

事件使用现有事件总线上的协议 v1。设备标识是 `open` 传入的标识。纪元为 `fs-N` 或 `reg-N`，每次成功启动后递增。该纪元的序号从 1 开始，观察和缺口共用。省略 `precision_ns`。这些 API 不报告变更时刻的精度，时间戳是用户态观察到缓冲区时的时钟。超出 `i64` 纳秒范围的时钟会饱和，不用 0 代替。进程字段省略。这里没有操作进程标识。不读取文件字节。

元数据能区分时，文件或目录名称写成 `file.added`、`file.modified`、`directory.added` 或 `directory.modified`。完整路径能放入标识符时，主体使用完整路径。否则主体使用相对名称，部分原因为 `absolute_path_exceeds_identifier`。两者都放不下时没有主体，原因为 `path_exceeds_identifier`。用户是 `GetNamedSecurityInfoW` 读到的所有者 SID。SDDL 也能放入标识符时，它就是归因证据，完整性为完整。SDDL 放不下时，证据保持为提供者名称，部分原因为 `permissions_exceed_identifier`。缺少安全描述符时使用 `permissions_unavailable`。删除是 `path.removed`，原因为 `security_unavailable`。不为删除编造所有者。重命名是一对事件，先是 `path.renamed_from`，然后是 `file.renamed_to` 或 `directory.renamed_to`。不成对的重命名使用 `rename_unpaired`。不是 Unicode 的名称是 `path.unreadable`，原因为 `name_not_unicode_{action}`。逃出根目录的名称，包括 `..` 和绝对名称，不会被打开，原因为 `path_escapes_root`。重解析点使用 `reparse_unresolved`，不声称它是文件或目录。读不到元数据时使用 `metadata_unavailable`。

注册表主体是 `HKCU\{subkey}\{name}`。当前进程身份能放入标识符时，用户使用该身份。新增的证据是 `t={type};a={hex}`，删除是 `t={type};b={hex}`，类型不变的修改是 `t={type};b={before};a={after}`。类型变化使用 `tb`、`ta`、`b` 和 `a`。十六进制为小写。整段证据必须能放入标识符，不截断。放不下时，证据改为提供者名称，部分原因为 `value_exceeds_identifier`。订阅时已经存在的基线不会发布。

## 仍保留的限制

没有加载或卸载已签名驱动。没有在 Secure Boot 和 HVCI 下验证。本变更没有测试 Windows 10 和 Windows 11 客户端。Windows 测试针对 `x86_64-pc-windows-msvc` 编译，并在 Windows 工作区作业中运行。该作业是 Windows Server 2022。E2E-02 仍要求在选定磁盘、其他用户和更完整的文件与注册表场景上通过 GUI 查询。计划 019 记录了用户态的变更前后插件归因。文件插件是计划 021，见[跟踪插件](TRACE.md)。计划 009 的行为不变。计划 021 的编写主机没有执行 `windows_file_watch_overflow_records_a_lost_gap`。
