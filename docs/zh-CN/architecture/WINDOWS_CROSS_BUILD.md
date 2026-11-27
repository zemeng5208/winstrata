# Windows x64 交叉构建探针

[English](../../architecture/WINDOWS_CROSS_BUILD.md) · [文档索引](../INDEX.md)

PR 003 验证纯 Linux 原生链接 Windows 控制台映像与内核映像。两个探针均用 Rust 编写。控制台探针调用 `ExitProcess`。内核探针通过 `KeQueryTimeIncrement` 读取时钟间隔，有意返回 `STATUS_NOT_SUPPORTED`，未注册任何资源。

## 构建输入

- [已批准的 Rust 工具链](DEVELOPMENT_TOOLCHAIN.md)，安装 `x86_64-pc-windows-msvc` 目标。开发命令从当前 Rust sysroot 查找 Linux 宿主附带的 `rust-lld`。
- [版本文件](../../../toolchains/windows/kit-version.txt)所固定的 Microsoft WDK 和 Windows SDK x64 导入库。从对应的官方 NuGet 包获得，使用前核对许可。[WDK 包及许可](https://www.nuget.org/packages/Microsoft.Windows.WDK.x64/10.0.26100.6584) · [Windows SDK 包及许可](https://www.nuget.org/packages/Microsoft.Windows.SDK.CPP.x64/10.0.26100.6584)
- 支持 PE/COFF 的 GNU `objdump` 和用于本地导入库校验的 GNU `sha256sum`。Linux x86_64 使用 GNU Binutils 2.44 实测。

从下载目录使用 `sha512sum --check` 按 [NuGet 目录公布的 SHA-512](../../../toolchains/windows/nuget-packages.sha512)验证原始包，再将下表归档成员提取到当前项目的 `.winstrata/toolchains/microsoft/<kit-version>/`。

| 归档成员 | 工具包目录内的路径 |
|---|---|
| WDK `c/Lib/10.0.26100.0/km/x64/ntoskrnl.lib` | `wdk/ntoskrnl.lib` |
| SDK `c/um/x64/kernel32.Lib` | `sdk/kernel32.lib` |

命令会校验[两份导入库摘要](../../../toolchains/windows/imports.sha256)，库文件位于 Git 忽略目录中。

```sh
rustup target add --toolchain 1.100.0 x86_64-pc-windows-msvc
cargo dev cross-probe
```

每次运行在 `target/windows-cross-probe/` 下新建输出目录，生成 COFF 对象、`console.exe` 和 `driver.sys`。命令在验证两个映像的 x64 PE32+ 格式、非零入口、预期子系统与导入项以及驱动校验和后报告目录。导入库缺失或摘要不匹配会在编译和链接前失败。

## 实测与待验证

两次 Linux 构建产出的两个映像逐字节一致，各占 2,560 字节。控制台采用子系统 3，仅导入 `KERNEL32.dll!ExitProcess`。驱动采用子系统 1，仅导入 `ntoskrnl.exe!KeQueryTimeIncrement`。GNU `objdump` 已核验入口、驱动校验和、动态基址和 NX 标志、只读代码段及空 Authenticode 目录。这些证据覆盖两个探针的编译、导入库与链接阶段。

完整应用、服务、WebView2 前端及其跨平台构建依赖随实际组件验证。当前 Linux 环境未执行 Windows 运行测试。原生 Linux 的 INF 校验和目录生成也待验证。微软 WDK 提供 Windows 下的 `InfVerif.exe` 与 `Inf2Cat.exe`。[微软 INF 工具](https://learn.microsoft.com/en-us/windows-hardware/drivers/devtest/tools-for-inf-files)

签名与成功加载需要可加载和卸载的代表性驱动、经验证的驱动包及适用的微软签名流程。应在启用 Secure Boot 和 HVCI 的 Windows x64 测试环境中验证。当前探针是未签名映像，且有意拒绝成功加载。PE 静态验证不构成签名或 HVCI 验收证据。[微软签名选项](https://learn.microsoft.com/en-us/windows-hardware/drivers/dashboard/driver-signing-offerings) · [内核模式完整性](https://learn.microsoft.com/en-us/windows/security/operating-system-security/driver-security/kernel-mode-code-signing-and-integrity)
