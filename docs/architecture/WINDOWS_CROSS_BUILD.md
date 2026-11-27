# Windows x64 cross-build probe

[简体中文](../zh-CN/architecture/WINDOWS_CROSS_BUILD.md) · [Documentation index](../INDEX.md)

PR 003 verifies native Linux linking for a Windows console image and a native kernel image. Both are small toolchain probes written in Rust. The console probe calls `ExitProcess`. The kernel probe reads the clock increment through `KeQueryTimeIncrement` and intentionally returns `STATUS_NOT_SUPPORTED` with no registered resources.

## Inputs

- The [approved Rust toolchain](DEVELOPMENT_TOOLCHAIN.md) with `x86_64-pc-windows-msvc` installed. The tool discovers the Linux host's bundled `rust-lld` through the active Rust sysroot.
- Microsoft WDK and Windows SDK x64 import libraries pinned by the [kit version](../../toolchains/windows/kit-version.txt). Obtain the exact versions from the official NuGet packages and review their licenses. [WDK package and license](https://www.nuget.org/packages/Microsoft.Windows.WDK.x64/10.0.26100.6584) · [Windows SDK package and license](https://www.nuget.org/packages/Microsoft.Windows.SDK.CPP.x64/10.0.26100.6584)
- GNU `objdump` with PE/COFF support and GNU `sha256sum` for local inspection and import validation. Linux x86_64 verification used GNU Binutils 2.44.

Verify the downloaded packages with the [NuGet catalog SHA-512 digests](../../toolchains/windows/nuget-packages.sha512) using `sha512sum --check` from the package download directory. Extract the two members to `.winstrata/toolchains/microsoft/<kit-version>/` under the current checkout.

| Package member | Path under the kit directory |
|---|---|
| WDK `c/Lib/10.0.26100.0/km/x64/ntoskrnl.lib` | `wdk/ntoskrnl.lib` |
| SDK `c/um/x64/kernel32.Lib` | `sdk/kernel32.lib` |

The run verifies [both extracted library digests](../../toolchains/windows/imports.sha256) and retains them in the local Git-ignored directory.

```sh
rustup target add --toolchain 1.100.0 x86_64-pc-windows-msvc
cargo dev cross-probe
```

Each run writes COFF objects, `console.exe` and `driver.sys` in a new directory under `target/windows-cross-probe/`. The command reports the directory only after verifying both images as x64 PE32+, their nonzero entry points, each expected subsystem and import, and the driver checksum. A missing or different import library causes a failure before the compiler or linker runs.

## Observed and pending evidence

Two Linux runs produced byte-identical 2,560-byte images. The console uses subsystem 3 and imports only `KERNEL32.dll!ExitProcess`. The driver uses subsystem 1 and imports only `ntoskrnl.exe!KeQueryTimeIncrement`. GNU `objdump` verified the entry point, driver checksum, dynamic-base and NX flags, read-only code, and the empty Authenticode directory. This establishes the compile, archive and link stages for the probes.

The full application, service, WebView2 frontend, and their cross-build dependencies remain work for the actual components. Windows runtime remains unverified in this Linux environment. INF validation and catalog generation also remain open for the native Linux pipeline. Microsoft's WDK provides Windows `InfVerif.exe` and `Inf2Cat.exe` for these steps. [Microsoft INF tools](https://learn.microsoft.com/en-us/windows-hardware/drivers/devtest/tools-for-inf-files)

Signing and successful loading need a representative load/unload driver, a validated package, and the appropriate Microsoft signing workflow. Verification belongs on a configured Windows x64 test environment with Secure Boot and HVCI enabled. The current probe is unsigned and intentionally declines to load. Static PE checks give no Windows signing or HVCI acceptance evidence. [Microsoft signing options](https://learn.microsoft.com/en-us/windows-hardware/drivers/dashboard/driver-signing-offerings) · [Kernel-mode integrity](https://learn.microsoft.com/en-us/windows/security/operating-system-security/driver-security/kernel-mode-code-signing-and-integrity)
