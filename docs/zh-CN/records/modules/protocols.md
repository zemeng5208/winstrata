# 协议错误登记

[English](../../../records/modules/protocols.md) · [全局登记](../ERRORS.md)

`protocols` 的权威错误记录。

<a id="err-015"></a>

## ERR-015

- Error ID: ERR-015
- 发现时间: 2026-11-27，计划 005 审查期间，`995d148` 的绿色候选之前。审查评论 URL 未知。
- 关联 PR: GitHub PR #6，计划 005
- 环境: Linux 和 Windows Server 2022 协议测试
- 复现方法: 用默认解码器解码位置数组或数字枚举标签
- 证据: 计划 005 的记录说，审查发现默认解码可接受位置数组或数字枚举标签。具名对象和字符串标签检查在两个系统上都拒绝了原来的用例。没有单独保存失败作业 URL。
- 根因: 默认解码表示接受了这些形式。替代检查要求具名对象和字符串标签。
- 影响: 原来的用例不是有效的协议编码
- 修复计划: 计划 005 合并前已实现
- 重测结果: `995d148` 的候选在 Linux 上通过 11 项库测试和 2 项集成测试，Windows 还通过了真实系统事件测试。[Linux](https://github.com/zemeng5208/winstrata/actions/runs/45971619572/job/140413162125)。[Windows](https://github.com/zemeng5208/winstrata/actions/runs/45971619572/job/140413162795)。两种语言文档中的 6 个 JSON 代码块都通过了类型化解码、编码和再解码。
- 当前状态: 已解决
