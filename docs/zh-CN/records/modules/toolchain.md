# 工具链错误登记

[English](../../../records/modules/toolchain.md) · [全局登记](../ERRORS.md)

`toolchains/dev` 和固定宿主工具链的权威错误记录。

<a id="err-002"></a>

## ERR-002

- Error ID: ERR-002
- 发现时间: 2026-11-27，计划 001 预检。没有记录时钟时间。
- 关联 PR: GitHub PR #2，计划 001
- 环境: 已批准工具链安装之前的 Linux x86_64 开发宿主
- 复现方法: `PATH` 中没有 Rust 和 Cargo 时运行 `cargo dev doctor`
- 证据: 预检因为缺少 Rust 和 Cargo 而退出。核验 Rustup 安装器校验和后，把官方工具链安装到项目忽略的本地工具目录。版本检查和构建随后通过。
- 根因: 宿主的 `PATH` 上没有已批准的 Rust 工具链
- 影响: 在安装工具链之前，计划 001 的预检无法构建
- 修复计划: 在该环境本地安装固定工具链。已完成。
- 重测结果: Rust 与 Cargo 1.100.0 下，`cargo dev doctor`、`cargo dev init`、`cargo dev build` 和 `cargo dev verify` 通过
- 当前状态: 已解决

后来使用 Node.js v22.23.4、而不是固定版本 24.22.3 的宿主，没有在本机运行 `cargo dev doctor`。该缺口是 [OI-010](../OPEN_ITEMS.md#oi-010)。它不是这条已解决的错误。
