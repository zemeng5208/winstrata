# 全局错误登记

[English](../../records/ERRORS.md) · [未验收事项](OPEN_ITEMS.md) · [进度](STATUS.md) · [索引](../INDEX.md)

这是 [AG-25](../standards/DOCUMENTATION.md#ag-25) 和 [AG-63](../AGENTS.md#ag-63) 要求的全局错误登记。[STATUS.md](STATUS.md) 只链接到这里，不能代替本登记。模块拥有的错误以模块文件为准。下面的仓库和 CI 错误以本文件为准。已解决的错误永久留在历史中。未知字段写明未知。本计划不改产品代码。

ADR-001 仍然用 STATUS.md 记录进度和决策。该决定不代替本登记。

## 索引

| ID | 状态 | 权威位置 |
|---|---|---|
| ERR-000 | 已解决 | 本文件 |
| ERR-001 | 已解决 | 本文件 |
| ERR-002 | 已解决 | [工具链](modules/toolchain.md#err-002) |
| ERR-003 | 已解决 | [内核](modules/kernel.md#err-003) |
| ERR-004 | 已解决 | 本文件 |
| ERR-005 | 已解决 | [Core](modules/core.md#err-005) |
| ERR-006 | 已解决 | 本文件 |
| ERR-007 | 已解决 | [内核](modules/kernel.md#err-007) |
| ERR-008 | 已解决 | [内核](modules/kernel.md#err-008) |
| ERR-009 | 已解决 | [Core](modules/core.md#err-009) |
| ERR-010 | 已解决 | [Core](modules/core.md#err-010) |
| ERR-011 | 已解决 | [Core](modules/core.md#err-011) |
| ERR-012 | 已解决 | [Core](modules/core.md#err-012) |
| ERR-013 | 已解决 | [Core](modules/core.md#err-013)。与 ERR-005 是同一次缺陷。 |
| ERR-014 | 已解决 | [Core](modules/core.md#err-014) |
| ERR-015 | 已解决 | [协议](modules/protocols.md#err-015) |
| ERR-016 | 已解决 | [Core](modules/core.md#err-016) |
| ERR-017 | 已解决 | [服务](modules/services.md#err-017) |
| ERR-018 | 已解决 | [Core](modules/core.md#err-018) |
| ERR-019 | 已解决 | 本文件 |
| ERR-020 | 已解决 | [服务插件](modules/strata-services.md#err-020) |
| ERR-021 | 未解决 | [服务插件](modules/strata-services.md#err-021) |
| ERR-022 | 已解决 | [StrataScript](modules/strata-script.md#err-022) |

`plugins/strata-script` 没有未解决错误。[ERR-022](modules/strata-script.md#err-022) 留在它的登记里。Windows AppContainer 缺口是 [OI-045](OPEN_ITEMS.md#oi-045)，不是错误。[ERR-020](modules/strata-services.md#err-020) 和 [ERR-021](modules/strata-services.md#err-021) 属于 `strata.services`。

计划 023 的模块登记在[启动插件](modules/strata-startup.md)。该文件还没有错误。[OI-049](OPEN_ITEMS.md#oi-049) 是 Windows 启动项缺口，不是错误编号。

ERR-005 是 main `a2471d5fc13f0a307ee39d2629c454aea4537809` 上的 Windows 进程退出失败。ERR-013 是同一次缺陷。计划 052 是指定的修复，已合并为 `004974215e239f96160b47d3eef40a84c316e802`。该提交的 Windows 工作区检查通过。计划 051 不实现这项修复。

<a id="err-000"></a>

## ERR-000

- Error ID: ERR-000
- 发现时间: 2026-11-27。没有记录时钟时间。
- 关联 PR: 仓库初始化，计划 000 合并之前
- 环境: 通过 GitHub HTTPS 推送开发分支
- 复现方法: 通过 HTTPS 推送开发分支
- 证据: GitHub 返回 HTTP 403。个人账号下没有覆盖该仓库的安装。所有者配置后，一次开发分支试推送成功。
- 根因: 集成没有覆盖该仓库的安装
- 影响: 开发分支推送被拒绝
- 修复计划: 所有者配置。已完成。
- 重测结果: 试推送成功
- 当前状态: 已解决

<a id="err-001"></a>

## ERR-001

- Error ID: ERR-001
- 发现时间: 2026-11-27。没有记录时钟时间。
- 关联 PR: 仓库保护，规则集生效之前
- 环境: GitHub 规则集 API
- 复现方法: 通过集成创建保护规则集
- 证据: GitHub 返回 HTTP 403。所有者保存了 [Protect main](https://github.com/zemeng5208/winstrata/rules/28217627)。查询分支、规则集和生效规则确认已启用。集成仍然缺少设置权限。
- 根因: 集成无权创建规则集
- 影响: 自动创建规则集失败。保护随后由所有者保存。
- 修复计划: 所有者已保存规则集。集成缺少设置权限仍是集成能力的记录限制，不是未解决的产品缺陷。
- 重测结果: 生效规则显示规则集已启用
- 当前状态: 已解决

<a id="err-004"></a>

## ERR-004

- Error ID: ERR-004
- 发现时间: 2026-11-27，计划 003 合并期间
- 关联 PR: GitHub PR #4，计划 003
- 环境: GitHub Actions 合并策略和 main 分支保护
- 复现方法: 在只强制 Workspace checks、且未开启严格最新基线时合并计划 003
- 证据: [PR 003 策略失败](https://github.com/zemeng5208/winstrata/actions/runs/45964752411)。[PR 003 主分支 CI](https://github.com/zemeng5208/winstrata/actions/runs/45965182726)。计划 004 头 `1788410` 通过两项检查。[PR 004 策略通过](https://github.com/zemeng5208/winstrata/actions/runs/45968732577)。[生效规则](https://api.github.com/repos/zemeng5208/winstrata/rules/branches/main)。
- 根因: 当时尚未采用后来会阻断失败策略状态的必需检查和最新基线规则
- 影响: 计划 003 在策略检查失败时被合并
- 修复计划: 所有者要求 `Workspace checks`、`winstrata/merge-policy` 和分支保持最新，来源为 GitHub Actions，绕过名单为空。计划 004 合并前已完成。
- 重测结果: 计划 004 通过两项必需检查
- 当前状态: 已解决

<a id="err-006"></a>

## ERR-006

- Error ID: ERR-006
- 发现时间: 2026-11-27，计划 052 的头 `d180ae58afdfff1ce826875abb577792159e98a7`
- 关联 PR: GitHub PR #17，计划 052，合并前
- 环境: GitHub Actions 合并策略工作流，对照 main `a2471d5fc13f0a307ee39d2629c454aea4537809` 评估该头
- 复现方法: 工作流检查 “Evaluate merge policy from main” 发布提交状态 `winstrata/merge-policy`
- 证据: 已发布状态失败，说明是 “PR title and branch must identify a plan ID in the roadmap”。`mergeable_state` 是 blocked。脚本检出的 main 路线图没有 052，因此 `order.indexOf("052")` 为负。分支和标题已经是 `pr/052-process-exit-sample` 和 `PR 052: ...`。Linux 作业 140620679852 于 2026-11-27T16:21:28Z 成功，Windows 作业 140620680173 于 2026-11-27T16:22:25Z 成功，Workspace 作业 140621425280 于 2026-11-27T16:22:30Z 成功。[工作区运行](https://github.com/zemeng5208/winstrata/actions/runs/46032831553) · [策略运行](https://github.com/zemeng5208/winstrata/actions/runs/46032831547)。计划 008 的管道失败是 [ERR-017](modules/services.md#err-017)。
- 根因: 策略脚本读取的是当时还不包含计划 052 的 main 路线图
- 影响: 那个头不是审查头。Linux、Windows 和 Workspace 检查已经成功。
- 修复计划: 当拉取请求修改 `docs/roadmap/PLAN.md` 时，从已验证的测试合并提交读取该文件，并且仍然拒绝删除或重排已有编号。实现于 `a6a22c200eca6d67f5cffff6f370d0537567850e`，并合并为 `004974215e239f96160b47d3eef40a84c316e802`。
- 重测结果: 该合并上的 “Evaluate merge policy from main” 检查成功。[策略运行](https://github.com/zemeng5208/winstrata/actions/runs/46036724065/job/140633881742)。写这份记录时，该合并提交的提交状态 API 没有返回状态上下文。
- 当前状态: 已解决

<a id="err-019"></a>

## ERR-019

- Error ID: ERR-019
- 发现时间: 2026-11-27T20:36:59Z
- 关联 PR: GitHub PR #29，计划 021，头 `0b46a2edba97a587d774d1b721f714dde840f6a6`
- 环境: GitHub Actions `windows-2022`，Windows Server 2022
- 复现方法: `cargo test --locked --workspace` 进入 `windows_trace_records_metadata_without_content`
- 证据: 测试创建了 `observed.txt`，只看到 `gap:SourceFailure`。[Windows 作业](https://github.com/zemeng5208/winstrata/actions/runs/46064663019/job/140726967687)。该头的 Linux 工作区检查通过。同一个 Windows 进程里 `windows_file_watch_overflow_records_a_lost_gap` 通过。随后 Workspace 检查失败，因为 Windows 失败。已发布的 `winstrata/merge-policy` 状态失败，说明是 “Plan 020 must merge before plan 021”。这个顺序结果是预期的，不是这项缺陷。
- 根因: `ReadDirectoryChangesW` 发出时，`OVERLAPPED` 还在栈上的值里。把该值推进监视向量会移动这个结构。内核把完成状态写到旧地址，`GetOverlappedResult` 读到的是空副本。
- 影响: 一次真实的文件创建被报成 `source_failure`。监视线程随后停止。Windows 工作区检查失败。
- 修复计划: 先把目录放进向量，再在原地址发出读取。完成字节数也留在同一结构里。生产缓冲区仍是 16 KiB。实现于 `dc9d9e378f9f585929f6acfc18944411f1bbcc75`。
- 重测结果: `dc9d9e378f9f585929f6acfc18944411f1bbcc75` 上的通过不能关闭换基之后的这项缺陷。换基后的头 `2b0ad8d871dad905ea7fae551a2f9f69bb3b8386` 上，Windows Server 2022 的 `cargo test --locked --workspace` 通过。`windows_trace_records_metadata_without_content` 与另外 7 个跟踪测试一起通过。[Windows 作业](https://github.com/zemeng5208/winstrata/actions/runs/46066366972/job/140732532200)。同一作业里 `windows_file_watch_overflow_records_a_lost_gap` 也通过，这不重新打开 OI-018。本文件的 ERR-019 是计划 021 的 `OVERLAPPED` 缺陷。计划 022 把另一项服务控制管理器身份读取被拒绝的缺陷记为 [ERR-020](modules/strata-services.md#err-020)。本记录不替换那项缺陷。
- 当前状态: 已解决
