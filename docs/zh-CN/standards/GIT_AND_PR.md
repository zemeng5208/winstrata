# Git 与 PR 规范

[English](../../standards/GIT_AND_PR.md) · [文档索引](../INDEX.md)

<a id="ag-12"></a>

### AG-12 Git 与 PR 权限

可建分支、Worktree、Commit、推开发分支、开/更新 PR、查看 CI。禁止直推 main、Force Push、擅自重写已提交历史、绕过保护、自行合并。删分支/Tag/Release、关闭其他 PR、改顺序须批准；仅项目所有者合并。后续 PR 基于已合并前序重新验证。

<a id="ag-13"></a>

### AG-13 PR 计划及合并顺序

正式开发前审核细粒度完整路线图，明确计划号、前置、范围/目录、目标、接口、排除项、测试/量化验收、文档、安全和性能。创建 PR 申请合并位置、基线、依赖与冲突影响；可并行，不强制开发串行，实际合并严格按批准顺序。未经批准不新增/删除/拆合/重排。基于最新目标分支验证真实合并结果和语义冲突，禁止已删除功能/依赖/接口/文档回流及修复被覆盖；必要回归检查，不能仅凭 Git 无冲突通过。

<a id="ag-14"></a>

### AG-14 代码审查与缺陷阻断

个人维护阶段允许自审，不强制独立 Agent，可按复杂度辅助；不降审查标准。检查架构、逻辑、安全、性能、测试、回流、文档、范围；确认问题必须修复并重新验证。即使合并后发现，也阻断下一顺序合并；不受影响开发可继续，不能把确认缺陷推到后续。记录修复及真实重验证据。

<a id="ag-27"></a>

### AG-27 合并后确认

获取最新 main，检查实际合并代码、CI/整体回归、前序功能/修复及无回流；核对错误、文档、PR 与模块进度，确认下一顺序前置。异常阻断下一合并。记录更新走批准 PR，不直推主分支。

<a id="ag-48"></a>

### AG-48 强制规范自动化

能程序验证的规范逐步落实 CI/仓库门禁：顺序、依赖、架构、回流、引用、洁净、安全、质量。优先复用成熟工具和统一框架，不为每条造大量脚本；不能自动的人工/Agent 审查。检查本身真实验证，规则变更审批，不绕过。

<a id="ag-49"></a>

### AG-49 阶段验收与构建责任（最新覆盖规则）

阶段结束完整验收 PR、功能、核心/插件进度、安全、性能、稳定性、架构/依赖/配置/文件树/文档及错误债务、下一阶段前置；未达标不进入下一阶段。跨平台是项目能力，不是每次双环境验证义务：谁写谁负责当前可用环境构建/测试/排错，允许本地或云端，不强制 VM 或双端；未执行环境如实记录，已确认跨平台缺陷必须修复。不得把 Linux 成功冒充真实 Windows，环境缺失不制造无关开发阻塞。覆盖早期所有强制双端构建描述。

<a id="section-13"></a>

## Git 与 PR 顺序治理

### 13.1 计划顺序与并行

计划序号（`PR 000` 等）独立于 GitHub 分配的 PR 号。PR 000 审核完整路线图后才建立正式开发序列。创建每个 PR 时申请合并位置，列出任务范围、计划号、前置依赖、影响的后续 PR、共同修改的文件/接口、基线 commit、冲突风险及验证方案。可并行开发无依赖冲突的任务，合并顺序可以为经批准的依赖顺序/批次，不强制开发串行；实际合并必须符合批准位置及最新基线。调整、新增、拆分、合并、重排须批准。

### 13.2 强制门禁

- 仅所有者执行合并。Agent 可建分支/Worktree、Commit、推送开发分支、开/更新 PR、查看 CI；禁止直推 main、Force Push、重写已提交历史、绕过保护；删分支/Tag/Release、关闭其他 PR 须批准。
- 通过 CI/受保护检查验证批准顺序及前置合并、当前目标基线、缺陷阻断及检查状态；具体实现需要真实验证，不能只依赖 PR 标签或人工承诺。自动机制不足时由所有者显式阻断，不假称强制门禁已生效。
- 后续 PR 基于最新 main 重新集成并验证实际合并候选，遵循禁止重写历史；检查语义冲突而非仅文本冲突。禁止旧分支带回前序已删除的功能/文件/依赖/配置/接口、已修复缺陷或过期文档；建立必要防回流检查，实际检查合并 diff 及回归。
- PR 审查在个人维护阶段允许自审，独立 Agent 非强制。必须检查业务逻辑、架构、安全、性能、测试、文档、范围和回流；确认问题必须全部修复且重验。即使合并后发现也阻断下一顺序合并；不受影响开发可并行继续。
- 合并后重新获取 main，检查真实代码/CI/回归、删除决策、错误记录与进度、下一 PR 前置条件。必要的文档更新通过获批的 PR/路线图位置完成，不直推主分支，不以文档为由绕序。
- 阶段完成前整体验收：全部该阶段 PR、真实用户流程、核心和插件进度、安全、性能、稳定性、当前环境构建、跨平台能力未退化、架构及文档全局检查、错误及技术债务。未通过不宣布阶段完成。双环境不强制每次实际构建（见 [§3.4](../architecture/SYSTEM.md#section-3)）。

### 13.3 PR 申请模板

```markdown
# PR <plan-id>: <English title>
- Plan ID / Requirement IDs:
- Requested merge position / Dependencies:
- Owner-approved scope / Exclusions:
- Base commit / Concurrent PR impact:
- Implementation and interface changes:
- Allowed files / Actual changed files:
- Acceptance scenarios / Actual commands and environment:
- Test and build evidence / Not verified:
- Performance comparison / Security and privacy impact:
- Self-review findings / Fixes and re-verification:
- Anti-regression and removed-function check:
- Documentation impact / Updated references:
- Errors / Blockers / Pending decisions:
- Workspace cleanup:
- Owner approval / Merge readiness:
```

实际 PR 内容用英文，测试证据可引用双语项目文档；模板中的条目为待填写字段，不是已经执行的检查。禁止伪造检查结果。
