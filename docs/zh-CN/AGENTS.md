# WinStrata Agent 规范

[English](../../AGENTS.md) · [文档索引](INDEX.md)

PR 000 审核通过并合并后，以下规范随项目基线生效。开始工作前，从规范索引阅读所有适用规则和对应的产品、架构、接口与验收文档。

- 保留所有者的架构、范围、供应链和合并决策权。仅修改已批准范围。
- 使用开发分支及 PR。严禁直接推送 main、强推、绕过保护和由 Agent 合并。
- 验证实际环境、完整流程和当前合并候选。确认缺陷须修复并重验，剩余未验证事项必须标明。
- 保持最小权限和本地数据边界。前端执行[指定组件来源、三套主题及排版规范](architecture/FRONTEND.md)，见 [AG-30](standards/ARCHITECTURE.md#ag-30)。
- 执行[开发前检查、变更同步、申请 PR、自审与交接清单](standards/AGENT_WORKFLOW.md#execution-checklist)。

<a id="ag-24"></a>

### AG-24 规范优先级与防篡改

项目内优先级从高到低：P0 所有者最新明确批准决策；P1 根 AGENTS.md；P2 全局架构/技术规范；P3 模块 AGENTS/设计；P4 已批准 PR 与验收；P5 现有代码。低级不得覆盖高级，冲突报告。不得反改规范、测试、验收或模块规则使错误代码通过。规范变更记录依据、影响和批准，独立审批（AG-60）。

<a id="ag-58"></a>

### AG-58 分层规范

根 AGENTS.md 保留最高红线/索引，详细规则按开发、架构、Git/PR、测试、安全、文档、依赖、Agent 工作流集中；模块只补充专属，读取所有适用规范，不选择忽略。63 项全部保留，同项权威定义一次、引用关联，修改递归检查，避免重复膨胀/Token 浪费。

<a id="ag-59"></a>

### AG-59 PR 000

创建仓库后先交付规范、架构、完整路线图/顺序、目录、项目依赖/配置、文档/错误/需求追踪、质量/验收规划及许可证，用户逐项审核合并；不含正式业务功能。未合并禁止开始正式功能开发。

<a id="ag-60"></a>

### AG-60 规范变更审批

强制规范实质变更独立申请及 PR，说明原条款、拟修改、原因/依据、必要性和影响；所有者批准并合并才生效。不混入普通功能 PR，不为错误代码通过反改。生效后检查全部关联文档、自动门禁及未合并 PR；保留历史。

<a id="ag-62"></a>

### AG-62 未验收事项的强制追踪与闭环

每一项未实现、未验收、部分验收、未测试、技术假设、已知限制和延期事项都必须有唯一编号，并写入[未验收事项登记](records/OPEN_ITEMS.md)。每条记录包含：关联需求、来源 PR、当前状态、未完成原因、负责 PR、交接负责人、验证环境、验收标准、关闭条件。只写“待验证”而不给出明确后续计划是禁止的。开发开始、提交 PR、合并后复核、上下文压缩和 Agent 交接时，检查每一项仍开放的记录，不得遗漏。确认缺陷不得伪装成技术限制或未验收事项。确认缺陷必须修复并重测，并阻断下一顺序 PR 合并。因环境限制无法验证的事项沿用既有规则，如实记录，并且仍须有明确的后续验收计划。

<a id="ag-63"></a>

### AG-63 独立错误管理与强制关闭

执行 [AG-25](standards/DOCUMENTATION.md#ag-25)。保持独立的[全局错误记录](records/ERRORS.md)和 [records/modules/](records/modules/core.md) 下的模块错误记录。错误不得只存在于 STATUS.md、PR 正文或聊天会话中。每条错误必须有唯一 Error ID、发现时间、关联 PR、环境、复现方法、证据、根因、影响、修复计划、重测结果和当前状态。未知项明确标出，不得编造事实。CI 失败、回归、安全问题、数据完整性风险和已确认的功能缺陷必须及时登记。未解决的错误不得在未经授权时删除、隐藏、降级或关闭。每次 PR 合并后复核实际 main。发生回归时，立即更新错误记录、需求矩阵和验收状态，并阻断下一顺序 PR 合并。全局记录与模块记录互相链接，保持双语，已解决的错误永久保留在历史中。

## 全部规范索引

| 编号 | 权威条款 |
|---|---|
| AG-01 | [standards/DEVELOPMENT.md](standards/DEVELOPMENT.md#ag-01) |
| AG-02 | [standards/DEVELOPMENT.md](standards/DEVELOPMENT.md#ag-02) |
| AG-03 | [standards/DEVELOPMENT.md](standards/DEVELOPMENT.md#ag-03) |
| AG-04 | [standards/DEVELOPMENT.md](standards/DEVELOPMENT.md#ag-04) |
| AG-05 | [standards/DOCUMENTATION.md](standards/DOCUMENTATION.md#ag-05) |
| AG-06 | [standards/DOCUMENTATION.md](standards/DOCUMENTATION.md#ag-06) |
| AG-07 | [standards/AGENT_WORKFLOW.md](standards/AGENT_WORKFLOW.md#ag-07) |
| AG-08 | [standards/DEPENDENCIES.md](standards/DEPENDENCIES.md#ag-08) |
| AG-09 | [standards/DEVELOPMENT.md](standards/DEVELOPMENT.md#ag-09) |
| AG-10 | [standards/DEVELOPMENT.md](standards/DEVELOPMENT.md#ag-10) |
| AG-11 | [standards/AGENT_WORKFLOW.md](standards/AGENT_WORKFLOW.md#ag-11) |
| AG-12 | [standards/GIT_AND_PR.md](standards/GIT_AND_PR.md#ag-12) |
| AG-13 | [standards/GIT_AND_PR.md](standards/GIT_AND_PR.md#ag-13) |
| AG-14 | [standards/GIT_AND_PR.md](standards/GIT_AND_PR.md#ag-14) |
| AG-15 | [standards/TESTING.md](standards/TESTING.md#ag-15) |
| AG-16 | [standards/TESTING.md](standards/TESTING.md#ag-16) |
| AG-17 | [standards/TESTING.md](standards/TESTING.md#ag-17) |
| AG-18 | [standards/TESTING.md](standards/TESTING.md#ag-18) |
| AG-19 | [standards/DEPENDENCIES.md](standards/DEPENDENCIES.md#ag-19) |
| AG-20 | [standards/AGENT_WORKFLOW.md](standards/AGENT_WORKFLOW.md#ag-20) |
| AG-21 | [standards/DEVELOPMENT.md](standards/DEVELOPMENT.md#ag-21) |
| AG-22 | [standards/DEVELOPMENT.md](standards/DEVELOPMENT.md#ag-22) |
| AG-23 | [standards/SECURITY.md](standards/SECURITY.md#ag-23) |
| AG-24 | [AGENTS.md](#ag-24) |
| AG-25 | [standards/DOCUMENTATION.md](standards/DOCUMENTATION.md#ag-25) |
| AG-26 | [standards/DEVELOPMENT.md](standards/DEVELOPMENT.md#ag-26) |
| AG-27 | [standards/GIT_AND_PR.md](standards/GIT_AND_PR.md#ag-27) |
| AG-28 | [standards/DOCUMENTATION.md](standards/DOCUMENTATION.md#ag-28) |
| AG-29 | [standards/DOCUMENTATION.md](standards/DOCUMENTATION.md#ag-29) |
| AG-30 | [standards/ARCHITECTURE.md](standards/ARCHITECTURE.md#ag-30) |
| AG-31 | [standards/DEVELOPMENT.md](standards/DEVELOPMENT.md#ag-31) |
| AG-32 | [standards/ARCHITECTURE.md](standards/ARCHITECTURE.md#ag-32) |
| AG-33 | [standards/DEVELOPMENT.md](standards/DEVELOPMENT.md#ag-33) |
| AG-34 | [standards/DEVELOPMENT.md](standards/DEVELOPMENT.md#ag-34) |
| AG-35 | [standards/DEPENDENCIES.md](standards/DEPENDENCIES.md#ag-35) |
| AG-36 | [standards/DOCUMENTATION.md](standards/DOCUMENTATION.md#ag-36) |
| AG-37 | [standards/TESTING.md](standards/TESTING.md#ag-37) |
| AG-38 | [standards/DEVELOPMENT.md](standards/DEVELOPMENT.md#ag-38) |
| AG-39 | [standards/DEPENDENCIES.md](standards/DEPENDENCIES.md#ag-39) |
| AG-40 | [standards/DEPENDENCIES.md](standards/DEPENDENCIES.md#ag-40) |
| AG-41 | [standards/DEVELOPMENT.md](standards/DEVELOPMENT.md#ag-41) |
| AG-42 | [standards/ARCHITECTURE.md](standards/ARCHITECTURE.md#ag-42) |
| AG-43 | [standards/ARCHITECTURE.md](standards/ARCHITECTURE.md#ag-43) |
| AG-44 | [standards/ARCHITECTURE.md](standards/ARCHITECTURE.md#ag-44) |
| AG-45 | [standards/ARCHITECTURE.md](standards/ARCHITECTURE.md#ag-45) |
| AG-46 | [standards/ARCHITECTURE.md](standards/ARCHITECTURE.md#ag-46) |
| AG-47 | [standards/SECURITY.md](standards/SECURITY.md#ag-47) |
| AG-48 | [standards/GIT_AND_PR.md](standards/GIT_AND_PR.md#ag-48) |
| AG-49 | [standards/GIT_AND_PR.md](standards/GIT_AND_PR.md#ag-49) |
| AG-50 | [standards/AGENT_WORKFLOW.md](standards/AGENT_WORKFLOW.md#ag-50) |
| AG-51 | [standards/AGENT_WORKFLOW.md](standards/AGENT_WORKFLOW.md#ag-51) |
| AG-52 | [standards/TESTING.md](standards/TESTING.md#ag-52) |
| AG-53 | [standards/TESTING.md](standards/TESTING.md#ag-53) |
| AG-54 | [standards/DEPENDENCIES.md](standards/DEPENDENCIES.md#ag-54) |
| AG-55 | [standards/DEPENDENCIES.md](standards/DEPENDENCIES.md#ag-55) |
| AG-56 | [standards/DEPENDENCIES.md](standards/DEPENDENCIES.md#ag-56) |
| AG-57 | [standards/AGENT_WORKFLOW.md](standards/AGENT_WORKFLOW.md#ag-57) |
| AG-58 | [AGENTS.md](#ag-58) |
| AG-59 | [AGENTS.md](#ag-59) |
| AG-60 | [AGENTS.md](#ag-60) |
| AG-61 | [standards/DOCUMENTATION.md](standards/DOCUMENTATION.md#ag-61) |
| AG-62 | [AGENTS.md](#ag-62) |
| AG-63 | [AGENTS.md](#ag-63) |
