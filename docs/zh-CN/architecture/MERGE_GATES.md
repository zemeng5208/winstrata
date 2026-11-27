# CI 与合并门禁

[English](../../architecture/MERGE_GATES.md) · [文档索引](../INDEX.md)

PR 002 实现 [Git 与 PR 规范](../standards/GIT_AND_PR.md#section-13)所述的检查。合并遵循[路线图](../roadmap/PLAN.md)的批准顺序。互不依赖的分支可以并行准备。所有者在证据和保护要求满足后审核及合并。

## 检查项

| 检查 | 来源及范围 |
|---|---|
| `Workspace checks` | `WinStrata CI` 获取 GitHub 为 PR 生成的精确测试合并提交。Linux 运行器按项目声明安装工具链，检查空白、纳入 Git 的生成产物、合并策略回归测试，并运行与本地相同的 `cargo dev init`、`build` 和 `verify`。两个锁文件须保持一致。任务没有 GitHub API 权限，也不接收项目密钥。 |
| `winstrata/merge-policy` | 策略流程只拉取受保护的 `main`，验证当前测试合并提交及两个父提交，确认 PR 的头提交包含当前 main 后，将策略状态写入头提交。GitHub Actions 工作区检查也记录在该头提交。流程读取仓库及 PR 元数据，权限仅包含必要的状态写入。候选代码不参与此流程。PR、所有者评论、阻断问题和 `main` 更新时重新检查相关候选。 |

本地复用时，使用[已批准的工具链](DEVELOPMENT_TOOLCHAIN.md)。在 Linux 的 Git 工作区检出候选提交并保留其第一父提交，然后运行 `bash .github/scripts/check-workspace.sh`。

## 合并判定

分支用 `pr/NNN-description`，标题用 `PR NNN: Title`，两处计划编号须一致。门禁读取 `main` 的有序路线图，核对 GitHub 上的前序合并和重复编号。草稿不能通过检查。

PR 本身有确认缺陷时加 `merge-blocked` 标签。相同标签的未关闭议题阻断所有待合并 PR，包括合并后发现的缺陷。修复并复验后，所有者可关闭议题或移除标签，并等待更新后的状态。审查对话仍由 GitHub 原生规则检查。

变更工作流、统一开发检查、路线图或治理文件，需要所有者确认当前提交。审核后由所有者在 PR 评论中填写 `Approve gate policy at <完整 head SHA>`。提交更新后需要重新确认。检查覆盖重命名前后的文件。

## 激活强制检查

1. 在 PR 002 上确认 `Workspace checks` 成功后，由所有者将其加入现有 `Protect main` 规则集。来源选项可用时指定为 GitHub Actions。保留现有 PR、解决审查对话、限制删除、禁止强推和空绕过列表。
2. PR 002 合并后，策略流程从 `main` 生效。第一个后续候选须在当前头提交上看到真实的 `winstrata/merge-policy` 状态和 GitHub Actions 检查。工作区任务仍在测试合并提交上运行。暂留该 PR 供验证，由所有者将状态加入第二项强制检查，来源选项可用时指定 GitHub Actions，同时启用分支须更新至最新基线。
3. 读取有效规则，确认两项检查生效后再合并候选。刚更新提交、阻断议题、标签或评论时，须等待策略重新评估。事件是异步的，仓库写入者和设置管理员仍是受信任角色，所有者仍须核对实际范围和检查。

GitHub 需要先观察到实际运行，才能选择为强制检查。过渡期间，由所有者手动执行待激活的门禁。托管运行或生效规则尚未核验时，应记录为待完成。
