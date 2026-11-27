# Core 错误登记

[English](../../../records/modules/core.md) · [全局登记](../ERRORS.md) · [未验收事项](../OPEN_ITEMS.md)

`core` 的权威错误记录。[AG-63](../../AGENTS.md#ag-63) 要求这些字段。STATUS.md 不能代替本文件。

<a id="err-016"></a>

## ERR-016

- Error ID: ERR-016
- 发现时间: 2026-11-27，计划 007 期间 `58452bd` 的 Linux 工作区作业
- 关联 PR: 计划 007，合并前
- 环境: GitHub Actions Linux 工作区
- 复现方法: `cargo test` 进入 `unsubscribe_and_shutdown_release_queues_without_keeping_threads`
- 证据: [Linux 失败](https://github.com/zemeng5208/winstrata/actions/runs/45982561210/job/140450010980)。`live_threads` 已经是 0，汇合后立刻仍有一个线程号带有该调度器的名称。同一提交的 Windows 工作区检查通过。
- 根因: 检查把汇合后立刻仍带调度器名称的线程号当成存活线程。后来的修复记录每个线程的启动时间，并最多等待两秒让内核删除任务项。没有证据表明存在更深的调度器泄漏。
- 影响: Linux 工作区检查失败。该提交的合并策略评估通过。
- 修复计划: 计划 007 合并前已实现。本记录不改那段代码。
- 重测结果: 本机连续八次 `cargo test --locked -p winstrata-core --lib -- --test-threads=16` 通过，其中包括该测试。`0dceae00c8693cd84c6c061538206233622a27da` 上 GitHub 的 Workspace、Linux 和 Windows 检查成功。计划 007 合并提交 `3e0097a44f54d552662b39ab3621002ebce59505` 的主分支 CI 成功。
- 当前状态: 已解决

<a id="err-009"></a>

## ERR-009

- Error ID: ERR-009
- 发现时间: 2026-11-27，计划 012 期间 `53408d7e2ba450b5cacc15923c513f9fc9e1881b` 的 Windows 工作区作业
- 关联 PR: 计划 012，合并前
- 环境: GitHub Actions `windows-2022`
- 复现方法: `cargo test --locked --workspace` 进入 `writes_query_segments_index_and_capacity` 和 `Hybrid::create`
- 证据: Windows 错误 1305，“The revision level is unknown.”。[Windows 作业](https://github.com/zemeng5208/winstrata/actions/runs/45998553679/job/140504032301)。同一份 Rust 代码在 `dfe59001d1647d8093fe8a90881efdc9a397f045` 上通过。失败进程里其余混合存储创建都通过了。
- 根因: 记录到的原因是安全描述符修订无法读取。它不是 DACL 比较失败。那次读取为何失败一次，除此之外未知。
- 影响: Windows 工作区检查失败，因此 Workspace 检查失败
- 修复计划: 创建私有目录和重新读取描述符时，对错误 1305 最多重试四次。所有者和 ACE 拒绝检查保持不变。计划 012 合并前已实现。
- 重测结果: `5243b91d50948f89d92635adb7a02f169f89c54f` 通过全部 33 项 Windows Core 测试，包括原先失败的那一项
- 当前状态: 已解决

<a id="err-010"></a>

## ERR-010

- Error ID: ERR-010
- 发现时间: 2026-11-27，计划 012 期间 `b6481384b0796c1dccd526202613036cba346c69` 的 Windows 工作区作业
- 关联 PR: 计划 012，合并前
- 环境: GitHub Actions `windows-2022`
- 复现方法: `cargo test --locked --workspace` 进入 `hybrid::tests::measured_event_batch_round_trips` 和 `Hybrid::create`
- 证据: Windows 错误 1336，“The access control list (ACL) structure is invalid.”。[Windows 作业](https://github.com/zemeng5208/winstrata/actions/runs/46021115653/job/140580685441)。同一进程里其余混合存储创建都通过了。Core 在这次失败处停止，内核和服务测试没有运行。
- 根因: 安全描述符读取在所有者和 ACE 比较之前返回 1336。所有者和 ACE 不符仍返回拒绝访问，并且不会重试。那次读取为何失败一次，除此之外未知。
- 影响: Windows 工作区检查失败。该头不是审查头。
- 修复计划: 同一次读取对 1305 和 1336 最多重试四次；错误如果持续存在，仍然返回。计划 012 合并前已实现。
- 重测结果: `31f15a72cc2ff4d63136caa8517dd64a448a0001` 通过，包括 `measured_event_batch_round_trips`
- 当前状态: 已解决

<a id="err-011"></a>

## ERR-011

- Error ID: ERR-011
- 发现时间: 2026-11-27，计划 009 期间 `83cd44377fd4588c7706a6f227ca6e819a7e177e` 和 `798b91ff5a394d6fec2820fe0142ecac873fcb35` 的 Windows 作业
- 关联 PR: 计划 009，合并前
- 环境: GitHub Actions `windows-2022`
- 复现方法: `cargo test --locked --workspace` 运行文件和注册表监视测试
- 证据: `83cd443` 上 Core 为 34 项通过、4 项失败。[Windows 作业](https://github.com/zemeng5208/winstrata/actions/runs/46007410073/job/140533972873)。`798b91f` 上两项文件测试只看到 `gap:SourceFailure`。[Windows 作业](https://github.com/zemeng5208/winstrata/actions/runs/46008317408/job/140537032504)。
- 根因: 第一次失败把 `ReadDirectoryChangesW` 的字节数放在栈地址上，并用无法匹配转义反斜杠的 JSON 主体检查。第二次失败把内核完成状态写进随后被移动的 `OVERLAPPED`。记录的修复把重叠结构、字节数和缓冲区留在第一次调用使用的地址上。
- 影响: 两个头的 Windows 工作区检查都失败
- 修复计划: 计划 009 合并前已实现
- 重测结果: `565599ed4f6125fdfa737d4e83087027211879c2` 通过三项 Windows 文件和注册表测试
- 当前状态: 已解决

<a id="err-012"></a>

## ERR-012

- Error ID: ERR-012
- 发现时间: 2026-11-27，计划 013 期间 `62b7bc1dde69cd793373e48abcdd8891e2a5c022` 的 Windows 作业
- 关联 PR: GitHub PR #16，计划 013，审查头之前
- 环境: GitHub Actions `windows-2022`，版本 10.0.20348
- 复现方法: `cargo test --locked --workspace` 运行 `backup_and_restore_keep_ciphertext_integrity`
- 证据: 58 项 Core 测试通过，该测试失败。篡改包用默认 ACL 写出，私有文件检查返回 `PermissionDenied`，校验值还没有机会返回 `InvalidData`。[Windows 作业](https://github.com/zemeng5208/winstrata/actions/runs/46026990013/job/140600824049)。
- 根因: 篡改包没有使用真实备份所用的私有文件 ACL
- 影响: Windows 工作区检查失败，因此 Workspace 检查失败
- 修复计划: 用与真实备份相同的私有文件调用创建篡改包，测试仍然要求 `InvalidData`。合并前已实现。
- 重测结果: 审查头 `36b5cd688afa2f9690f4a634d0a4c57ec6eba7cd` 通过 59 项 Core 测试，包括该测试。[Windows 作业](https://github.com/zemeng5208/winstrata/actions/runs/46027463216/job/140602443474)。
- 当前状态: 已解决

<a id="err-005"></a>
<a id="err-013"></a>

## ERR-005

- Error ID: ERR-005。ERR-013 是同一次缺陷，不是第二个错误。
- 发现时间: 2026-11-27T15:49:51Z
- 关联 PR: GitHub PR #16 的合并，计划 013，main `a2471d5fc13f0a307ee39d2629c454aea4537809`
- 环境: GitHub Actions `windows-2022`。运行器标签是 Windows Server 2022。它不是 Windows 10 或 Windows 11 客户端。
- 复现方法: 该 main 提交上的 WinStrata CI 在步骤 “Test the workspace on Windows” 中运行 `cargo test --locked --workspace`。测试套件在 `process::tests::real_process_tree_pid_lifecycle_and_dedup` 的 `core/src/process.rs:905` 恐慌，断言是 `has_event(&third, PROCESS_EXITED, pid, start)`。
- 证据: [Windows 作业](https://github.com/zemeng5208/winstrata/actions/runs/46028223956/job/140605027762) 于 2026-11-27T15:49:51Z 完成，结论为失败。[Linux 作业](https://github.com/zemeng5208/winstrata/actions/runs/46028223956/job/140605027316) 于 2026-11-27T15:49:30Z 成功。[Workspace 作业](https://github.com/zemeng5208/winstrata/actions/runs/46028223956/job/140605466118) 因 Windows 失败而失败。合并父提交为 `be5eb0d699ad0a7e6aab46df6f6bbbf3d912f8af` 与 `31f22d3f0f9d5ef0e9120f9ba6b75af868ce32a7`。第二父提交与 main 的文件树相同。它与审查头 `36b5cd688afa2f9690f4a634d0a4c57ec6eba7cd` 的唯一差别是 STATUS 记录，因此 Rust 测试代码与审查头相同。审查头的 Windows 作业通过了 59 项 Core 测试，所以该断言在那里没有失败。[审查头 Windows 作业](https://github.com/zemeng5208/winstrata/actions/runs/46027463216/job/140602443474)。同一文件树在 `31f22d3` 上也通过了。[稍后的审查头 Windows 作业](https://github.com/zemeng5208/winstrata/actions/runs/46027962309/job/140604132924)。失败日志没有包含那次采样。
- 根因: 未知。断言失败了。同一测试代码在几分钟前的审查头上通过，因此失败可能是间歇性的。这种可能不是根因，也不关闭本错误。采样被截断，或 PID 仍然被列出但没有正的启动时间，都会抑制 `process.exited`。这次运行属于哪一种，没有被记录下来。
- 影响: main `a2471d5` 上已确认的 CI 失败。Workspace 检查失败。那次运行不能把 [REQ-06.02](../../roadmap/REQUIREMENTS.md) 标成已验收。计划 052 是指定的产品修复。计划 051 不修改该测试。计划 007 的线程释放失败是 [ERR-016](#err-016)。
- 修复计划: 计划 052 保留退出断言，并等到完整采样不再包含该实例，或该实例被另一个启动时间替换。它不把缺失事件当成成功。所有者将该计划合并为 `004974215e239f96160b47d3eef40a84c316e802`。计划 051 不修改 `core/src/process.rs`，也不修改该测试。
- 重测结果: 合并提交 `004974215e239f96160b47d3eef40a84c316e802` 的 Windows 工作区检查通过。[Windows 作业](https://github.com/zemeng5208/winstrata/actions/runs/46036724064/job/140633882007)。该提交的 Linux 和 Workspace 检查也通过。先前失败的根因仍然未知。
- 当前状态: 已解决

<a id="err-014"></a>

## ERR-014

- Error ID: ERR-014
- 发现时间: 2026-11-27，计划 006 期间，候选 `b3f56ab` 之前。失败提交的 SHA 未知。
- 关联 PR: GitHub PR #7，计划 006
- 环境: Windows，替换配置期间。作业 URL 未知。
- 复现方法: 在共享祖先上持有长期锁时替换配置。失败候选的精确命令未知。
- 证据: 计划 006 的记录说，较早候选在替换配置时遇到 Windows 共享冲突，共享祖先上的长期锁还影响同级目录更名。没有保存失败 SHA 或作业 URL。
- 根因: 会话持有的锁挡住了替换和同级更名。会话现在只在操作之间持有定位锁文件。
- 影响: 那些较早的 Windows 候选失败。后来记录的候选通过。
- 修复计划: 计划 006 合并前已实现
- 重测结果: 候选 `b3f56ab` 在 Windows Server 2022 上通过 20 项 Core 测试。[Windows 作业](https://github.com/zemeng5208/winstrata/actions/runs/45976058358/job/140427947209)。本机 NTFS 回归确认私有目录保持固定，普通同级目录仍可改名。
- 当前状态: 已解决

<a id="err-018"></a>

## ERR-018

- Error ID: ERR-018
- 发现时间: 2026-11-27，计划 017 期间 `df34c741e80d02e711dc97b8b8eeec34eae05f24` 的 Windows 工作区作业
- 关联 PR: GitHub PR #28，计划 017，合并前
- 环境: GitHub Actions `windows-2022`。运行器标签是 Windows Server 2022。它不是 Windows 10 或 Windows 11 客户端。
- 复现方法: `cargo test --locked --workspace` 进入 `scope::tests::foreign_profiles_and_unselected_types_stay_out`、`scope::tests::registry_process_and_interface_choices_are_explicit` 和 `onboarding::tests::baseline_scope_pause_history_and_loss_stay_distinct`
- 证据: [Windows 作业](https://github.com/zemeng5208/winstrata/actions/runs/46058310468/job/140706127654)。`admits_file` 和 `admits_process` 拒绝了 `std::env::temp_dir()` 下的路径。`build_baseline` 随后返回 `a full snapshot needs at least one entry`。同一提交的 Linux 工作区检查通过。[Linux 作业](https://github.com/zemeng5208/winstrata/actions/runs/46058310468/job/140706126963)。
- 根因: 运行器的临时目录位于当前用户的配置目录内，Windows 用与 `USERPROFILE` 解析出的所有者不一致的 8.3 短路径写出该目录。范围过滤把当前用户自己的目录当成了其他用户的私有配置目录。
- 影响: Windows 工作区检查失败，因此 Workspace 检查失败。该配置目录下的选定基线无法保存。
- 修复计划: 比较当前配置目录的长路径和 8.3 短路径，并展开候选路径上已经存在的祖先。其他用户的私有配置目录仍然排除。所选范围没有匹配文件时保存基线对象，不写空快照。测试没有改弱。
- 重测结果: 等待修正提交上的下一次 Windows 工作区作业
- 当前状态: 未解决
