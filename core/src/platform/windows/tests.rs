use super::*;
use std::fs;
use std::process::{Command, Output};

struct Fixture(PathBuf);

impl Fixture {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!("winstrata-anchor-{}", fresh_id().unwrap()));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir(self.0.join("junction"));
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn system_command(name: &str) -> Command {
    Command::new(
        PathBuf::from(std::env::var_os("SystemRoot").unwrap())
            .join("System32")
            .join(name),
    )
}

fn succeeds(output: Output) {
    assert!(output.status.success(), "Windows fixture setup failed");
}

#[test]
fn token_matches_operating_system_and_local_app_data_exists() {
    let sid = current_id().unwrap();
    let control = system_command("whoami.exe")
        .args(["/user", "/fo", "csv", "/nh"])
        .output()
        .unwrap();
    assert!(control.status.success());
    assert!(sid.starts_with("S-1-"));
    assert!(
        String::from_utf8_lossy(&control.stdout)
            .split('"')
            .any(|field| field == sid)
    );
    assert!(locator_parent().unwrap().is_dir());
}

#[test]
fn private_leaf_pins_ancestors_and_owns_new_files() {
    let fixture = Fixture::new();
    let leaf = {
        let base = Anchor::open(&fixture.0).unwrap();
        let leaf = base.child("private".as_ref(), true, true).unwrap();
        let reopened = base.child("private".as_ref(), false, true).unwrap();
        assert!(leaf.same_directory(&reopened).unwrap());
        assert!(!base.same_directory(&leaf).unwrap());
        leaf
    };
    assert!(fs::rename(&fixture.0, fixture.0.with_extension("moved")).is_err());
    let file = private_file_create(&leaf.io_path().join("owner.dat")).unwrap();
    verify_private_file(&file).unwrap();
    drop(file);
    let renamed = leaf.io_path().join("moved.dat");
    fs::rename(leaf.io_path().join("owner.dat"), &renamed).unwrap();
    fs::hard_link(&renamed, leaf.io_path().join("alias.dat")).unwrap();
    assert!(leaf.available().unwrap() > 0);
}

#[test]
fn existing_broad_acl_and_junction_are_rejected_without_modification() {
    let fixture = Fixture::new();
    fs::create_dir(fixture.0.join("ordinary")).unwrap();
    succeeds(
        system_command("icacls.exe")
            .current_dir(&fixture.0)
            .args(["ordinary", "/grant", "*S-1-1-0:(OI)(CI)RX"])
            .output()
            .unwrap(),
    );
    succeeds(
        system_command("cmd.exe")
            .current_dir(&fixture.0)
            .args(["/d", "/c", "mklink", "/J", "junction", "ordinary"])
            .output()
            .unwrap(),
    );
    let base = Anchor::open(&fixture.0).unwrap();
    assert_eq!(
        base.child("ordinary".as_ref(), true, true)
            .unwrap_err()
            .kind(),
        io::ErrorKind::PermissionDenied
    );
    assert!(base.child("junction".as_ref(), true, true).is_err());
    assert!(Anchor::open(&fixture.0.join("junction")).is_err());
    assert!(!fixture.0.join("ordinary").join("new").exists());
}

#[test]
fn invalid_path_components_are_never_opened() {
    for name in [
        "",
        "..",
        ".",
        "COM1.txt",
        "lpt².log",
        "dir\\child",
        "file:ads",
        "aux .txt",
        "dir.",
        "dir ",
        "A\0B",
    ] {
        assert!(valid_name(&OsStr::new(name).encode_wide().collect::<Vec<_>>()).is_err());
    }
    for path in [
        r"C:relative",
        r"\\server\share",
        r"\\?\C:\Windows",
        r"\Windows",
    ] {
        assert!(Anchor::open(Path::new(path)).is_err());
    }
}

#[test]
fn restricted_token_can_read_its_control_but_cannot_open_private_data() {
    use windows_sys::Win32::Security::Authorization::ConvertStringSidToSidW;
    use windows_sys::Win32::Security::{
        CreateRestrictedToken, DISABLE_MAX_PRIVILEGE, ImpersonateLoggedOnUser, RevertToSelf,
        SID_AND_ATTRIBUTES, TOKEN_DUPLICATE, TOKEN_QUERY,
    };
    use windows_sys::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};

    let fixture = Fixture::new();
    let base = Anchor::open(&fixture.0).unwrap();
    let leaf = base.child("private".as_ref(), true, true).unwrap();
    let private_path = leaf.io_path().join("marker");
    let mut marker = private_file_create(&private_path).unwrap();
    std::io::Write::write_all(&mut marker, b"private fixture").unwrap();
    drop(marker);
    assert_eq!(fs::read(&private_path).unwrap(), b"private fixture");

    // 使用资源管理器 SID 命名空间的临时限制标识；不创建系统账户或组。
    let random = fresh_id().unwrap();
    let parts: Vec<_> = random
        .as_bytes()
        .chunks_exact(8)
        .map(|chunk| {
            u32::from_str_radix(std::str::from_utf8(chunk).unwrap(), 16)
                .unwrap()
                .to_string()
        })
        .collect();
    let restricting_sid = format!("S-1-9-{}", parts.join("-"));
    let control_path = base.io_path().join("control");
    fs::write(&control_path, b"public fixture").unwrap();
    for (name, access) in [(".", "RX"), ("control", "R")] {
        succeeds(
            system_command("icacls.exe")
                .current_dir(&fixture.0)
                .args([name, "/grant", &format!("*{restricting_sid}:({access})")])
                .output()
                .unwrap(),
        );
    }

    let wide: Vec<u16> = restricting_sid.encode_utf16().chain([0]).collect();
    let mut raw_sid = null_mut();
    let mut unrestricted = null_mut();
    let mut restricted = null_mut();
    unsafe {
        check(ConvertStringSidToSidW(wide.as_ptr(), &mut raw_sid)).unwrap();
        let _sid = security::LocalBlock(raw_sid);
        check(OpenProcessToken(
            GetCurrentProcess(),
            TOKEN_QUERY | TOKEN_DUPLICATE,
            &mut unrestricted,
        ))
        .unwrap();
        let parent = OwnedHandle::from_raw_handle(unrestricted);
        let limiting = SID_AND_ATTRIBUTES {
            Sid: raw_sid,
            Attributes: 0,
        };
        check(CreateRestrictedToken(
            parent.as_raw_handle(),
            DISABLE_MAX_PRIVILEGE,
            0,
            null(),
            0,
            null(),
            1,
            &limiting,
            &mut restricted,
        ))
        .unwrap();
    }
    let token = unsafe { OwnedHandle::from_raw_handle(restricted) };
    // 仅在短命专用线程中模拟当前令牌的受限副本；恢复身份后再断言测试结果。
    let (allowed, denied) = std::thread::spawn(move || {
        unsafe { check(ImpersonateLoggedOnUser(token.as_raw_handle())).unwrap() };
        let result = (fs::read(control_path), fs::read(private_path));
        unsafe { check(RevertToSelf()).unwrap() };
        result
    })
    .join()
    .unwrap();
    assert_eq!(allowed.unwrap(), b"public fixture");
    assert_eq!(denied.unwrap_err().kind(), io::ErrorKind::PermissionDenied);
}
