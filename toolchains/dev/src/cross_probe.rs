use std::ffi::{OsStr, OsString};
use std::fs;
use std::path::Path;

use super::{capture, command, output, run};

const TARGET: &str = "x86_64-pc-windows-msvc";

struct Probe {
    name: &'static str,
    entry_point: &'static str,
    subsystem: &'static str,
    subsystem_id: u32,
    library: &'static str,
    dll: &'static str,
    symbol: &'static str,
    driver: bool,
}

const CONSOLE: Probe = Probe {
    name: "console",
    entry_point: "WinStrataEntry",
    subsystem: "console",
    subsystem_id: 3,
    library: "sdk/kernel32.lib",
    dll: "kernel32.dll",
    symbol: "ExitProcess",
    driver: false,
};

const DRIVER: Probe = Probe {
    name: "driver",
    entry_point: "DriverEntry",
    subsystem: "native",
    subsystem_id: 1,
    library: "wdk/ntoskrnl.lib",
    dll: "ntoskrnl.exe",
    symbol: "KeQueryTimeIncrement",
    driver: true,
};

pub(super) fn run_probe(root: &Path) -> Result<(), String> {
    if !cfg!(target_os = "linux") {
        return Err("the Windows cross-build probe requires a Linux host".into());
    }
    let source = root.join("toolchains/windows");
    let kit = fs::read_to_string(source.join("kit-version.txt"))
        .map_err(|error| format!("read approved Microsoft kit version: {error}"))?;
    let version = kit.trim();
    let parts: Vec<_> = version.split('.').collect();
    if parts.len() != 4 || parts.iter().any(|part| part.parse::<u32>().is_err()) {
        return Err("kit-version.txt must contain a four-part numeric version".into());
    }
    let kits = root.join(".winstrata/toolchains/microsoft").join(version);
    for probe in [&CONSOLE, &DRIVER] {
        let library = kits.join(probe.library);
        if !library.is_file() {
            return Err(format!(
                "missing approved Microsoft import library: {}",
                library.display()
            ));
        }
    }
    let checksums = source.join("imports.sha256");
    run(
        &kits,
        "sha256sum",
        [OsStr::new("--check"), OsStr::new("--strict"), checksums.as_os_str()],
    )?;

    let target_libdir = output(root, "rustc", &["--target", TARGET, "--print", "target-libdir"])?;
    if !Path::new(&target_libdir).is_dir() {
        return Err(format!("missing Rust standard library component for {TARGET}"));
    }
    let sysroot = output(root, "rustc", &["--print", "sysroot"])?;
    let details = output(root, "rustc", &["-vV"])?;
    let host = details
        .lines()
        .find_map(|line| line.strip_prefix("host: "))
        .ok_or_else(|| "cannot determine the active Rust host target".to_string())?;
    let linker = Path::new(&sysroot)
        .join("lib/rustlib")
        .join(host)
        .join("bin/rust-lld");

    let output_root = root.join("target/windows-cross-probe");
    fs::create_dir_all(&output_root)
        .map_err(|error| format!("create probe output directory: {error}"))?;
    let build = output_root.join(std::process::id().to_string());
    fs::create_dir(&build).map_err(|error| format!("create this run's probe directory: {error}"))?;
    let result = [&CONSOLE, &DRIVER].into_iter().try_for_each(|probe| {
        link_probe(root, &source, &kits, &linker, &build, probe)
    });
    if let Err(error) = result {
        return match fs::remove_dir_all(&build) {
            Ok(()) => Err(error),
            Err(cleanup) => Err(format!("{error}. Clean up {}: {cleanup}", build.display())),
        };
    }
    println!("Windows x64 console and driver linkage verified in {}", build.display());
    Ok(())
}

fn link_probe(
    root: &Path,
    source: &Path,
    kits: &Path,
    linker: &Path,
    build: &Path,
    probe: &Probe,
) -> Result<(), String> {
    let source_file = source.join("probes").join(format!("{}.rs", probe.name));
    let object = build.join(format!("{}.obj", probe.name));
    let flags = [
        "--edition=2024",
        "--target",
        TARGET,
        "--crate-type=lib",
        "--emit=obj",
        "-Cpanic=abort",
        "-Copt-level=2",
        "-o",
    ];
    run(
        root,
        "rustc",
        flags.map(OsString::from).into_iter().chain([
            object.as_os_str().to_owned(),
            source_file.into_os_string(),
        ]),
    )?;

    let extension = if probe.driver { "sys" } else { "exe" };
    let binary = build.join(format!("{}.{extension}", probe.name));
    let mut output_arg = OsString::from("/out:");
    output_arg.push(&binary);
    let mut arguments = [
        "-flavor",
        "link",
        "/machine:x64",
        "/nodefaultlib",
        "/dynamicbase",
        "/nxcompat",
        "/release",
        "/brepro",
    ]
    .map(OsString::from)
    .to_vec();
    arguments.extend([
        format!("/subsystem:{},10.0", probe.subsystem).into(),
        format!("/entry:{}", probe.entry_point).into(),
        output_arg,
        object.into_os_string(),
        kits.join(probe.library).into_os_string(),
    ]);
    if probe.driver {
        arguments.push("/driver".into());
    }
    run(root, linker, arguments)?;
    inspect(root, &binary, probe)
}

fn inspect(root: &Path, binary: &Path, probe: &Probe) -> Result<(), String> {
    let mut inspect = command(root, OsStr::new("objdump"), ["-f", "-p"]);
    inspect.env("LC_ALL", "C").arg(binary);
    let headers = capture(inspect)?;
    let imports: Vec<_> = headers
        .lines()
        .filter_map(|line| line.trim().strip_prefix("DLL Name: "))
        .collect();
    let correct_import = matches!(imports.as_slice(), [dll] if dll.eq_ignore_ascii_case(probe.dll));
    if !headers.contains("pei-x86-64")
        || !headers.contains("PE32+")
        || hex_field(&headers, "Subsystem") != Some(probe.subsystem_id)
        || hex_field(&headers, "AddressOfEntryPoint").unwrap_or(0) == 0
        || (probe.driver && hex_field(&headers, "CheckSum").unwrap_or(0) == 0)
        || !correct_import
        || !headers.split_whitespace().any(|value| value == probe.symbol)
    {
        return Err(format!("unexpected PE headers or import table in {}", binary.display()));
    }
    println!(
        "{}: PE32+ / {} / {}!{}",
        binary.display(),
        probe.subsystem,
        probe.dll,
        probe.symbol
    );
    Ok(())
}

fn hex_field(headers: &str, key: &str) -> Option<u32> {
    headers.lines().find_map(|line| {
        let mut fields = line.split_whitespace();
        if fields.next() == Some(key) {
            u32::from_str_radix(fields.next()?, 16).ok()
        } else {
            None
        }
    })
}
