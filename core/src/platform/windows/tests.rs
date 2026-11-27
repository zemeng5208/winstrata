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
