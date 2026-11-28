# WinStrata Agent 规范

[English](../../AGENTS.md) · [文档索引](INDEX.md)

PR 000 审核通过并合并后，以下规范随项目基线生效。开始工作前，从规范索引阅读所有适用规则和对应的产品、架构、接口与验收文档。

- 保留所有者的架构、范围、供应链和合并决策权。仅修改已批准范围。
- 使用开发分支及 PR。严禁直接推送 main、强推、绕过保护和由 Agent 合并。
- 验证实际环境、完整流程和当前合并候选。确认缺陷须修复并重验，剩余未验证事项必须标明。
- 保持最小权限和本地数据边界。前端执行[指定组件来源、三套主题及排版规范](architecture/FRONTEND.md)，见 [AG-30](standards/ARCHITECTURE.md#ag-30) 与可替换 GUI 规则 [AG-64](#ag-64)。
- **强制主动阅读（[AG-65](#ag-65)）**：所有主 Agent、开发 Agent 和子 Agent 在执行以下任一任务前，必须主动读取最新版[平台实现记录与构建隔离规范](standards/PLATFORM_ISOLATION.md)：（1）平台专属功能开发或修改；（2）修改平台接口、公共接口及平台依赖；（3）编译、交叉编译及构建；（4）打包、发布及最终交付；（5）正式启动未来 Linux 版本开发；（6）平台兼容性检查及性能优化。严禁等待所有者提醒，严禁以历史对话或临时记忆代替最新文档，并在工程记录中注明所依据的文档版本。
- 执行[开发前检查、变更同步、申请 PR、自审与交接清单](standards/AGENT_WORKFLOW.md#execution-checklist)。

<a id="ag-24"></a>

### AG-24 规范优先级与防篡改

项目内优先级从高到低：P0 所有者最新明确批准决策；P1 根 AGENTS.md；P2 全局架构/技术规范；P3 模块 AGENTS/设计；P4 已批准 PR 与验收；P5 现有代码。低级不得覆盖高级，冲突报告。不得反改规范、测试、验收或模块规则使错误代码通过。规范变更记录依据、影响和批准，独立审批（AG-60）。

<a id="ag-58"></a>

### AG-58 分层规范

根 AGENTS.md 保留最高红线/索引，详细规则按开发、架构、Git/PR、测试、安全、文档、依赖、Agent 工作流集中；模块只补充专属，读取所有适用规范，不选择忽略。65 项全部保留，同项权威定义一次、引用关联，修改递归检查，避免重复膨胀/Token 浪费。

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

<a id="ag-64"></a>

### AG-64 可替换 GUI 与官方来源追踪

WinStrata 必须允许官方及第三方插件扩展、修改甚至完整替换 GUI。注册独立页面和仪表盘组件不是上限。插件可以修改全局主题、颜色、材质、字体、动效和交互方式；修改全局布局、导航栏、侧边栏、顶部区域和仪表盘；替换共享 UI 组件及其他插件的界面呈现；并提供完整的第三方 GUI，替换整个应用界面。前端插件的安装、启用、禁用、更新、卸载以及恢复默认界面，都经过[扩展规范](architecture/EXTENSIONS.md#section-5)中的现有插件市场。不另建市场。不同时运行多套完整 GUI 和渲染环境。

官方默认 GUI 必须完整内置、不可卸载，并同时作为默认界面和故障恢复界面。官方 GUI 与官方插件遵守 [AG-30](standards/ARCHITECTURE.md#ag-30) 所指向的[前端规范](architecture/FRONTEND.md#section-9)：四个已批准来源、三套共享主题和排版边界。它们复用上游源码和已有实现。禁止仅参考视觉风格后重写同类组件，禁止另造一套同类组件，禁止擅自设计新的视觉体系。允许组合已有组件、绑定真实业务数据、做必要的功能适配和主题配置。允许对 shadcn-admin 的整体布局做小幅排版、间距和内容组织调整。确实找不到满足要求的现成组件时，提交缺口、候选方案和证据，经所有者批准后才能自行实现或引入其他来源。第三方 GUI 插件可以使用自己的设计体系，同时必须遵守安全、兼容性和性能契约。

GUI 与 Core、业务插件、存储、采集服务及内核驱动彻底解耦。更换 GUI 不得中断后台监视、重新启动无关服务或丢失数据。第三方 GUI 不得绕过权限、安全隔离、救援授权和数据删除确认。这些关键安全交互由可信宿主执行。第三方 GUI 不得伪造授权。经过授权的业务能力、数据和事件只通过统一的前端 SDK/API 获取。

本规则不选定技术栈。后续计划先调研、验证并提交方案，经所有者批准后再实现。官方 GUI 或官方插件复用的每个组件和布局都写入[GUI 来源登记](records/GUI_SOURCES.md)，包括上游仓库、版本、源码路径、许可证、复用方式和修改内容。登记只描述同一提交里真实存在的文件。不得以进度为由降低要求。空壳接口、假插件、假主题切换或静态演示都不算完成。新能力进入需求矩阵和真实验收场景。

<a id="ag-65"></a>

### AG-65 平台实现记录、构建隔离与强制阅读

将双语[平台实现记录与构建隔离规范](standards/PLATFORM_ISOLATION.md)（[English](../standards/PLATFORM_ISOLATION.md)）作为平台技术储备记录、目标平台构建与打包隔离以及未来 Linux 接续开发的权威标准。现有 Linux 实现属于开发阶段技术储备，不代表正式支持 Linux 产品：当前阶段不删除、不为此额外重构，也不改变 Windows 10/11 x64 的 V1.0 产品目标。

Windows 与 Linux 必须按目标平台分别编译、链接、管理依赖与打包。任何平台的最终交付产物均不得包含其他平台专属的无用代码、二进制、运行库、依赖及资源，也不得将 `PLATFORM_ISOLATION.md` 或其他 `docs/` 工程规范作为运行资源打入安装包。公共逻辑允许共享，但不得引入冗余运行开销、不必要抽象层或额外后台服务。最终交付必须通过实际产物清单、依赖分析、二进制检查与运行性能测试验证隔离效果，并且可机器验证的规则必须落实为构建与交付工具中的自动检查，不得仅依赖 Agent 自觉执行。

涉及平台专属功能、修改平台/公共/依赖接口、编译/交叉编译/构建、打包/发布/交付、启动未来 Linux 版本开发、平台兼容性检查及性能优化的任务，所有主 Agent、开发 Agent 和子 Agent 开始前必须主动读取最新 `PLATFORM_ISOLATION.md`（不依赖所有者提醒、不依赖历史对话或临时记忆），在工程记录中注明所依据的文档版本，并在平台代码或依赖变更的同一 PR 内同步更新该文档。

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
| AG-64 | [AGENTS.md](#ag-64) |
| AG-65 | [AGENTS.md](#ag-65) |
