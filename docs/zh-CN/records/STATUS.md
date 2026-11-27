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
| PR 002 | 部分生效 | 已强制 Workspace checks。首次后续候选的策略拒绝未能阻止合并，见 ERR-004。 |
| PR 003 | 已合并，验收仍有待项 | 所有者将 [GitHub PR #4](https://github.com/zemeng5208/winstrata/pull/4) 合并为 `f8809cc68c97ea841ca02ebe5e8c7a17a6287994`。合并树与送审树一致，Linux 工作区回归和两个 x64 链接探针复验通过。打包、签名及实际驱动加载仍待验证。 |
| PR 004 | 待所有者审核 | [平台可行性与选型](../architecture/PLATFORM_FEASIBILITY.md)。实测证据、适用环境和待验项见报告。需补齐合并保护。 |
| PR 005–050 | 规划中 | Phase 0 验收后按路线图推进。 |
| Core 与 PL-01 至 PL-25 | 规划中 | 运行及性能均待验证；相关工作提交时更新需求及模块证据。 |

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

## 决策与技术假设

| ID | 状态 | 依据与后续 |
|---|---|---|
| ADR-000 | 所有者已确认 | 以 Apache-2.0 开放 `zemeng5208/winstrata`。初始提交包含许可证并予保留。后续材料带来实际 NOTICE 义务时增设。 |
| ADR-001 | 随 PR 000 生效 | 采用 `docs/` 和 `docs/zh-CN/` 双语关联文档、稳定计划及规范编号、统一工程记录和候选目录布局。 |
| ADR-002 | 所有者已确认 | 固定 Rust 与 Cargo 1.100.0、Node.js 24.22.3 LTS、pnpm 11.25.0。采用 Cargo 和 pnpm 根工作区，本阶段无第三方 Rust 或 JavaScript 软件包。 |
| ADR-003 | 所有者已确认 | 按各自工具许可，在本地开发环境使用官方 Microsoft WDK 与 Windows SDK x64 10.0.26100.6584 导入库验证 Linux 探针。 |
| AS-001 | 部分已验证 | 两个探针的纯 Linux 链接通过。原生 Linux INF/CAT 流程、微软签名、实际 Windows 加载及完整交叉构建待验证。 |
| AS-002 | 部分已验证 | [PR 004 证据](../architecture/PLATFORM_FEASIBILITY.md)涵盖当前系统、隔离、协议与组件。产品授权、混合运行时的完整隔离和客户端版本验收仍待验证。 |
| AS-003 | 待决策 | 确认计量与清理语义、无标记元数据留存、WebView2 主题、Windows 支持和性能方法。见[待决事项](../roadmap/PLAN.md#section-19)。 |

## 错误与交接

| ID | 状态 | 发现、影响与复验 |
|---|---|---|
| ERR-000 | 已解决 | 2026-11-27，GitHub HTTPS 开发分支推送返回 403，个人账号下无覆盖该仓库的安装。所有者配置后，开发分支推送试运行成功。 |
| ERR-001 | 所有者配置后已解决 | 集成创建规则集返回 403。所有者保存保护规则，随后查询分支、规则集及有效规则已确认启用。集成仍缺设置权限。 |
| ERR-002 | Linux 工作区已解决 | PR 001 预检发现 PATH 缺少 Rust 与 Cargo。验证 Rustup 安装器校验和后，将已批准的官方工具链安装至项目的忽略目录。工具版本与真实构建均通过验证。 |
| ERR-003 | 已解决 | 所选 x64 WDK 导入库未导出 `KeQueryInterruptTime`，第一次内核映像链接失败。检查归档后改用文档所列的真实导出 `KeQueryTimeIncrement`，重跑两项探针和 PE 检查。 |
| ERR-004 | 待所有者配置，阻断下一次合并 | PR 003 候选因缺少所有者对精确头提交的确认，策略状态为失败。有效规则当时仅强制 Workspace checks，最新基线选项关闭。两个合并后工作流成功。下一候选必须保持打开，直至策略检查成功、两项检查均已强制且最新基线选项生效。[策略状态所属运行](https://github.com/zemeng5208/winstrata/actions/runs/45964752411) · [合并后 CI](https://github.com/zemeng5208/winstrata/actions/runs/45965182726) |

在 `pr/004-platform-feasibility` 上继续验证。合并前读取实际生效规则和当前候选检查。在 Phase 0 复核之前，PR 005 保持规划状态。
