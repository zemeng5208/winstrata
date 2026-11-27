# Core 的用户配置与存储

[English](../../architecture/CORE-STORAGE.md) · [Rust 库](../../../core/src/lib.rs) · [计划 006](../roadmap/PLAN.md#pr-006)

PR 006 提供本地 Core 会话、运行设置及重新加载设置所需的存储。调用方使用实际运行进程的操作系统账号身份。Windows 从进程令牌读取用户 SID，Linux 使用有效 UID。

## 打开会话

以当前用户身份调用 `Core::open()`。Windows 将定位文件 `WinStrata/location.json` 和会话锁保存在用户的 Local AppData 目录。定位文件记录上次选择的存储位置，会话存续期间，操作系统保持 `session.lock` 的独占锁。默认数据根目录为 `D:\WinStrata\`。Linux 将定位文件保存在用户的 XDG 状态目录，首次打开时需显式选择数据目录。

受管文件为 `<所选根目录>/<用户标识>/core.json`，保存格式版本 1、所有者、工作区标识及设置。用户目录须保持私有。Core 检查所有者和权限，并拒绝路径重定向。用 `storage_state()` 区分 `Ready` 和 `NeedsDirectory { kind, detail }`。操作检测到卷失联、空间不足或无法写入时，会暂停持久化并停止实时模式。定位文件无法读取、格式无效或已有会话占用会话锁时，打开会话将返回错误。

## 修改配置与位置

- 初始会话停止实时监视，监视范围为空。用 `update_settings` 保存 `Settings` 中显式选择的 `monitored_roots` 和 `start_at_login`。监视目录必须为不含上级跳转的绝对路径。随登录启动默认为关闭，实际向操作系统注册启动项由相应集成负责。
- 选定至少一个监视目录后调用 `start_live`。用 `run_state()` 读取 `Stopped`、`Live` 或 `History` 状态。可调用 `stop` 停止会话，或调用 `enter_history` 退出实时模式并进入历史模式。重新打开会话后，监视仍为停止状态。
- 停止实时监视后，用 `select_directory` 显式选择目录。迁移时，Core 复制当前用户的普通文件和目录，重新枚举两侧并校验名称、类型及字节内容，成功后才原子更新定位文件。目录树重叠、链接或目标内容冲突均会被拒绝。迁移保留原文件。
- 原工作区不可用时，可在它恢复后重新选择原根目录。Core 会核对工作区标记是否与定位文件一致。若要在其他位置重新开始，须显式调用 `select_empty_directory`，选择独立且用户子目录为空的位置。原卷保留先前数据。原工作区仍可访问时应使用经过验证的迁移流程。

设置和定位文件的 JSON 上限为 64 KiB。未知版本、损坏内容以及已有的宽泛权限会导致操作失败，已有文件可供恢复。选择空监视范围会停止实时状态。此次迁移验证普通文件的内容、目录结构及用户隔离，遇到 NTFS 命名数据流会停止。Windows 文件系统实测覆盖 NTFS；ReFS 尚未实测。空间检查比较当前账户可用字节数与本次写入的需求，实际写入错误也会暂停持久化。

## 验证范围

在 Linux 和 Windows 上运行 `cargo test --locked -p winstrata-core`，覆盖启动、设置、重启、存储失联与恢复、私有目录访问和 Core 配置迁移。工作区 CI 在 Linux 与 Windows Server 2022 上运行测试。Windows 10/11 客户端验收、数据库及事件格式迁移、加密备份与恢复以及端到端监视由后续计划覆盖。见[工程记录](../records/STATUS.md)和[数据架构](DATA.md#section-8)。
