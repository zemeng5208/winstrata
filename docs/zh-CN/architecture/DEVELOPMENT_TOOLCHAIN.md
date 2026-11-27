# 开发工具链

[English](../../architecture/DEVELOPMENT_TOOLCHAIN.md) · [文档索引](../INDEX.md)

PR 001 建立 Rust 与 pnpm 工作区，以及 Linux 与 Windows 共用的开发入口。

## 已批准的工具链

| 工具 | 版本 | 声明 | 用途 |
|---|---|---|---|
| Rust 与 Cargo | 1.100.0 | [rust-toolchain.toml](../../../rust-toolchain.toml) | 编译 Rust 工作区与开发工具。 |
| Node.js | 24.22.3 LTS | [package.json](../../../package.json) | 运行 JavaScript 工作区工具。 |
| pnpm | 11.25.0 | [package.json](../../../package.json) | 解析并锁定 JavaScript 工作区依赖。 |

使用官方发行版，将匹配的工具加入 PATH。Rustup 从当前项目读取工具链版本，Rust 编译还需可用的宿主链接器。Rust 和 Cargo 可采用 Apache-2.0 或 MIT，Node.js 为 MIT 并附带组件许可，pnpm 为 MIT。已批准的 Node 版本为安全发行版，满足 pnpm 最低 Node 22.13 的要求。来源见 [Rust 发布公告](https://blog.rust-lang.org/2026/11/12/Rust-1.100.0/)、[Node.js 发布公告](https://nodejs.org/en/blog/release/v24.22.3)、[pnpm 注册表元数据](https://registry.npmjs.org/pnpm/11.25.0)。

## 统一命令

在仓库根目录或项目子目录运行以下命令。

| 命令 | 行为 |
|---|---|
| `cargo dev doctor` | 定位项目根目录，核验当前 Rust、Cargo、Node.js 和 pnpm 的版本。 |
| `cargo dev init` | 执行环境预检，获取 Rust 锁定依赖并安装按锁文件解析的 pnpm 工作区。 |
| `cargo dev build` | 使用已提交的锁文件构建 Rust 工作区。 |
| `cargo dev verify` | 运行 Cargo 工作区测试并验证 pnpm 锁文件的冻结解析。 |
| `cargo dev cross-probe` | 在 Linux 上构建并检查 [Windows x64 控制台与驱动链接探针](WINDOWS_CROSS_BUILD.md)。 |

Rust 使用工作区根目录的 [Cargo.toml](../../../Cargo.toml) 与 [Cargo.lock](../../../Cargo.lock)。pnpm 使用 [pnpm-workspace.yaml](../../../pnpm-workspace.yaml) 与 [pnpm-lock.yaml](../../../pnpm-lock.yaml)，将随交付纳入应用包和共享前端包。两个生态各维护一个根锁文件。工具从项目根目录启动子命令并使用当前宿主的工具链。Cargo 默认使用工作区的构建目录，pnpm 的依赖仓库与缓存在 `.winstrata/` 下。生成产物、安装目录和本地缓存均被 Git 忽略。

## 验收范围

PR 001 在当前可用环境构建并执行开发工具。Rust crate 使用标准库，JavaScript 工作区根包没有依赖。Linux 的验证记录见[项目进度](../records/STATUS.md)。Windows 环境验证及 Windows x64 应用和驱动交叉构建按[路线图](../roadmap/PLAN.md)推进，PR 002 引入的工作区质量检查见 [CI 与合并门禁](MERGE_GATES.md)。
