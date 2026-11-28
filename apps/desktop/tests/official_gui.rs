use std::io::{Read, Write};
use std::net::TcpStream;
use std::path::Path;

use winstrata_core::OFFICIAL;
use winstrata_desktop::{
    Bridge, ClosePreference, Language, Preferences, Theme, format_tray_tip, observe_at,
    uninstall_official_gui,
};
use winstrata_service::{Endpoint, Service};

fn scratch(name: &str) -> std::path::PathBuf {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or(0);
    std::env::temp_dir().join(format!(
        "winstrata-{name}-{}-{nanos}",
        std::process::id()
    ))
}

#[test]
fn preferences_round_trip_and_reject_corruption() {
    let directory = scratch("gui");
    let _ = std::fs::remove_dir_all(&directory);
    let mut preferences = Preferences::default();
    assert!(preferences.theme.is_none());
    preferences.theme = Some(Theme::LiquidGlass);
    preferences.language = Some(Language::ZhCn);
    preferences.close = Some(ClosePreference::Tray);
    preferences.save(&directory).unwrap();
    let loaded = Preferences::load(&directory).unwrap();
    assert_eq!(loaded.theme, Some(Theme::LiquidGlass));
    assert_eq!(loaded.language, Some(Language::ZhCn));
    assert_eq!(loaded.close, Some(ClosePreference::Tray));
    std::fs::write(directory.join("gui.json"), b"{").unwrap();
    assert!(Preferences::load(&directory).is_err());
    let _ = std::fs::remove_dir_all(&directory);
}

#[test]
fn bridge_persists_theme_and_rejects_driver_commands() {
    let root = scratch("bridge");
    let _ = std::fs::remove_dir_all(&root);
    let preferences = root.join("prefs");
    let locator = root.join("state");
    std::fs::create_dir_all(&locator).unwrap();
    let assets = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../packages/ui/dist");
    let bridge = Bridge::start(preferences.clone(), assets, locator.clone(), None).unwrap();
    let page = http(&bridge, "GET", &format!("/?token={}", bridge.token()), None, false);
    assert!(page.contains("200"), "{page}");
    let cookie = cookie(&page);
    let denied = http(&bridge, "POST", "/command", Some(r#"{"command":"subscribe"}"#), false);
    assert!(denied.contains("403"), "{denied}");
    let theme = http(
        &bridge,
        "POST",
        "/command",
        Some(r#"{"command":"set-theme","theme":"black-acrylic"}"#),
        true,
    );
    assert!(theme.contains("200"), "{theme}");
    let stored = Preferences::load(&preferences).unwrap();
    assert_eq!(stored.theme, Some(Theme::BlackAcrylic));
    let state = http(&bridge, "GET", "/state", None, true);
    assert!(state.contains("black-acrylic"), "{state}");
    assert!(state.contains("not_windows") || state.contains("ordinary") || state.contains("not_configured"), "{state}");
    assert!(!locator.join("WinStrata").join("plugins").exists());
    drop(bridge);
    let _ = cookie;
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn filter_page_stays_idle_and_does_not_create_plugins_before_a_location() {
    let root = scratch("filter");
    let _ = std::fs::remove_dir_all(&root);
    let preferences = root.join("prefs");
    let locator = root.join("state");
    std::fs::create_dir_all(&locator).unwrap();
    let assets = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../packages/ui/dist");
    let bridge = Bridge::start(preferences, assets, locator.clone(), None).unwrap();
    let idle = http(
        &bridge,
        "POST",
        "/command",
        Some(r#"{"command":"filter-apply","payload":"{\"op\":\"show\"}"}"#),
        true,
    );
    assert!(idle.contains("200"), "{idle}");
    assert!(idle.contains("\"enabled\":false"), "{idle}");
    assert!(idle.contains("\"kept\":0"), "{idle}");
    assert!(idle.contains("\"avoided\":0"), "{idle}");
    assert!(!locator.join("WinStrata").join("plugins").exists());
    let source = std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../packages/ui/compose/filter-page.tsx"),
    )
    .unwrap();
    assert!(source.contains("filter-apply"));
    assert!(!source.contains("\"enable\""));
    assert!(source.contains("Button"));
    assert!(source.contains("Input"));
    assert!(source.contains("Badge"));
    drop(bridge);
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn tag_page_stays_idle_and_does_not_create_plugins_before_a_location() {
    let root = scratch("tag");
    let _ = std::fs::remove_dir_all(&root);
    let preferences = root.join("prefs");
    let locator = root.join("state");
    std::fs::create_dir_all(&locator).unwrap();
    let assets = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../packages/ui/dist");
    let bridge = Bridge::start(preferences, assets, locator.clone(), None).unwrap();
    let idle = http(
        &bridge,
        "POST",
        "/command",
        Some(r#"{"command":"tag-apply","payload":"{\"kind\":\"event\",\"subject\":\"desk\"}"}"#),
        true,
    );
    assert!(idle.contains("200"), "{idle}");
    assert!(idle.contains("\"enabled\":false"), "{idle}");
    assert!(idle.contains("\"safety\":\"none\""), "{idle}");
    assert!(!locator.join("WinStrata").join("plugins").exists());
    let broken = http(
        &bridge,
        "POST",
        "/command",
        Some("{\"command\":\"tag-apply\",\"payload\":\"{\\\"kind\\\":\\\"event\\\"}\\n\"}"),
        true,
    );
    assert!(broken.contains("400"), "{broken}");
    let denied = http(&bridge, "POST", "/command", Some(r#"{"command":"enable"}"#), true);
    assert!(denied.contains("403"), "{denied}");
    drop(bridge);
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn storage_page_stays_idle_and_does_not_create_plugins_before_a_location() {
    let root = scratch("storage");
    let _ = std::fs::remove_dir_all(&root);
    let preferences = root.join("prefs");
    let locator = root.join("state");
    std::fs::create_dir_all(&locator).unwrap();
    let assets = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../packages/ui/dist");
    let bridge = Bridge::start(preferences, assets, locator.clone(), None).unwrap();
    let idle = http(
        &bridge,
        "POST",
        "/command",
        Some(r#"{"command":"storage-status"}"#),
        true,
    );
    assert!(idle.contains("200"), "{idle}");
    assert!(idle.contains("\"enabled\":false"), "{idle}");
    assert!(idle.contains("\"storage\":\"unavailable\""), "{idle}");
    assert!(!locator.join("WinStrata").join("plugins").exists());
    let source = std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../packages/ui/compose/storage-page.tsx"),
    )
    .unwrap();
    assert!(source.contains("storage-status"));
    assert!(source.contains("storage-configure"));
    assert!(source.contains("storage-decline"));
    assert!(source.contains("storage-propose"));
    assert!(source.contains("storage-confirm"));
    assert!(!source.contains("\"enable\""));
    assert!(source.contains("Button"));
    assert!(source.contains("Input"));
    assert!(source.contains("Badge"));
    drop(bridge);
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn script_page_stays_idle_and_does_not_create_plugins_before_a_location() {
    let root = scratch("script");
    let _ = std::fs::remove_dir_all(&root);
    let preferences = root.join("prefs");
    let locator = root.join("state");
    std::fs::create_dir_all(&locator).unwrap();
    let assets = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../packages/ui/dist");
    let bridge = Bridge::start(preferences, assets, locator.clone(), None).unwrap();
    let idle = http(
        &bridge,
        "POST",
        "/command",
        Some(r#"{"command":"script-workbench","payload":"{\"op\":\"empty\"}"}"#),
        true,
    );
    assert!(idle.contains("200"), "{idle}");
    assert!(idle.contains("\"enabled\":false"), "{idle}");
    assert!(idle.contains("\"presets\":0"), "{idle}");
    assert!(!locator.join("WinStrata").join("plugins").exists());
    let broken = http(
        &bridge,
        "POST",
        "/command",
        Some("{\"command\":\"script-workbench\",\"payload\":\"{\\\"op\\\":\\\"empty\\\"}\\n\"}"),
        true,
    );
    assert!(broken.contains("400"), "{broken}");
    let denied = http(&bridge, "POST", "/command", Some(r#"{"command":"enable"}"#), true);
    assert!(denied.contains("403"), "{denied}");
    drop(bridge);
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn gui_client_reads_status_without_taking_the_service() {
    let parent = scratch("observe");
    let _ = std::fs::remove_dir_all(&parent);
    std::fs::create_dir_all(&parent).unwrap();
    let endpoint = Endpoint::unique().unwrap();
    let mut service = Service::listen_catalog(endpoint).unwrap();
    let client = service.client_endpoint();
    let observed = observe_at(&parent, Some(&client));
    assert!(observed.service.contains("connected"), "{}", observed.service);
    assert!(observed.service.contains("idle"), "{}", observed.service);
    assert_eq!(observed.plugins.len(), OFFICIAL.len());
    assert!(observed.run.contains("stopped"), "{}", observed.run);
    service.shutdown();
    let after = observe_at(&parent, Some(&client));
    assert!(after.service.contains("unavailable"), "{}", after.service);
    assert!(uninstall_official_gui().is_err());
    let _ = std::fs::remove_dir_all(&parent);
}

#[test]
fn bridge_onboarding_keeps_scope_pause_history_and_loss_distinct() {
    let root = scratch("onboard");
    let _ = std::fs::remove_dir_all(&root);
    let preferences = root.join("prefs");
    let locator = root.join("state");
    let chosen = root.join("chosen");
    std::fs::create_dir_all(&locator).unwrap();
    std::fs::create_dir_all(&chosen).unwrap();
    let assets = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../packages/ui/dist");
    let bridge = Bridge::start(preferences, assets, locator, None).unwrap();
    let missing = http(
        &bridge,
        "POST",
        "/command",
        Some(&serde_json::json!({
            "command": "choose-location",
            "root": root.join("missing-parent").join("store"),
        }).to_string()),
        true,
    );
    assert!(missing.contains("400") || missing.contains("404"), "{missing}");
    let before = json_body(&http(&bridge, "GET", "/state", None, true));
    assert!(before["settings"]["monitoredRoots"].as_array().unwrap().is_empty(), "{before}");
    assert_eq!(before["onboarding"]["step"], "language");
    let located = http(
        &bridge,
        "POST",
        "/command",
        Some(&serde_json::json!({
            "command": "choose-location",
            "root": &chosen,
            "language": "en",
        }).to_string()),
        true,
    );
    assert!(located.contains("200"), "{located}");
    std::fs::write(chosen.join("keep.txt"), b"keep").unwrap();
    std::fs::write(root.join("secret.txt"), b"secret").unwrap();
    let rescue = http(
        &bridge,
        "POST",
        "/command",
        Some(r#"{"command":"opt-in","ids":["strata.rescue"]}"#),
        true,
    );
    assert!(rescue.contains("400"), "{rescue}");
    let opted = http(
        &bridge,
        "POST",
        "/command",
        Some(r#"{"command":"opt-in","ids":["strata.trace"]}"#),
        true,
    );
    assert!(opted.contains("200"), "{opted}");
    let marked = http(
        &bridge,
        "POST",
        "/command",
        Some(r#"{"command":"set-rescue","enabled":true}"#),
        true,
    );
    assert!(marked.contains("200"), "{marked}");
    let scoped = http(
        &bridge,
        "POST",
        "/command",
        Some(&serde_json::json!({
            "command": "set-scope",
            "roots": [&chosen],
            "fileTypes": ["txt"],
            "watchProcesses": false,
        }).to_string()),
        true,
    );
    assert!(scoped.contains("200"), "{scoped}");
    let retention = http(
        &bridge,
        "POST",
        "/command",
        Some(r#"{"command":"confirm-retention","days":null}"#),
        true,
    );
    assert!(retention.contains("200"), "{retention}");
    let boot = http(
        &bridge,
        "POST",
        "/command",
        Some(r#"{"command":"set-boot","startAtLogin":false}"#),
        true,
    );
    assert!(boot.contains("200"), "{boot}");
    let weak = http(
        &bridge,
        "POST",
        "/command",
        Some(r#"{"command":"enable-encryption","password":"short","autoLockSeconds":0}"#),
        true,
    );
    assert!(weak.contains("400"), "{weak}");
    assert!(!weak.contains("short"), "{weak}");
    let sealed = http(
        &bridge,
        "POST",
        "/command",
        Some(r#"{"command":"enable-encryption","password":"correct-horse","autoLockSeconds":0}"#),
        true,
    );
    assert!(sealed.contains("200"), "{sealed}");
    let mcp = http(
        &bridge,
        "POST",
        "/command",
        Some(r#"{"command":"set-mcp","enabled":false}"#),
        true,
    );
    assert!(mcp.contains("200"), "{mcp}");
    let baseline = http(&bridge, "POST", "/command", Some(r#"{"command":"build-baseline"}"#), true);
    assert!(baseline.contains("200"), "{baseline}");
    let state = json_body(&http(&bridge, "GET", "/state", None, true));
    assert_eq!(state["onboarding"]["complete"], true, "{state}");
    assert_eq!(state["onboarding"]["baselineLabel"], "reconstructed", "{state}");
    assert_eq!(state["onboarding"]["scripts"], false, "{state}");
    assert_eq!(state["onboarding"]["automaticSnapshots"], false, "{state}");
    assert_eq!(state["onboarding"]["rescue"], true, "{state}");
    assert_eq!(state["settings"]["startAtLogin"], false, "{state}");
    assert!(state["onboarding"]["optedIn"].as_array().unwrap().iter().any(|id| id == "strata.trace"));
    assert!(!state["onboarding"]["optedIn"].as_array().unwrap().iter().any(|id| id == "strata.rescue"));
    assert!(state["plugins"].as_array().unwrap().iter().all(|plugin| plugin["enabled"] == false), "{state}");
    assert!(!state.to_string().contains("secret"));
    assert!(!state.to_string().contains("correct-horse"));
    let live = http(&bridge, "POST", "/command", Some(r#"{"command":"start-live"}"#), true);
    assert!(live.contains("200"), "{live}");
    let running = json_body(&http(&bridge, "GET", "/state", None, true));
    assert_eq!(running["run"], "live", "{running}");
    assert_eq!(running["paused"], false, "{running}");
    let collectors = running["collectors"].as_str().unwrap();
    assert!(collectors == "unsupported" || collectors == "ready", "{running}");
    let paused = http(&bridge, "POST", "/command", Some(r#"{"command":"pause"}"#), true);
    assert!(paused.contains("200"), "{paused}");
    let held = json_body(&http(&bridge, "GET", "/state", None, true));
    assert_eq!(held["paused"], true, "{held}");
    assert!(held["markers"].as_array().unwrap().iter().any(|marker| {
        marker["activity"] == "pause" && marker["reason"] == "paused" && marker["releaseDrivers"] == false
    }), "{held}");
    let resumed = http(&bridge, "POST", "/command", Some(r#"{"command":"resume"}"#), true);
    assert!(resumed.contains("200"), "{resumed}");
    let history = http(&bridge, "POST", "/command", Some(r#"{"command":"enter-history"}"#), true);
    assert!(history.contains("200"), "{history}");
    let archived = json_body(&http(&bridge, "GET", "/state", None, true));
    assert_eq!(archived["run"], "history", "{archived}");
    assert_eq!(archived["drivers"], "not_configured", "{archived}");
    assert!(archived["markers"].as_array().unwrap().iter().any(|marker| {
        marker["activity"] == "history" && marker["releaseDrivers"] == true
    }), "{archived}");
    assert!(!archived["markers"].as_array().unwrap().iter().any(|marker| marker["activity"] == "loss"));
    let denied = http(&bridge, "POST", "/command", Some(r#"{"command":"enable"}"#), true);
    assert!(denied.contains("403"), "{denied}");
    let started = http(&bridge, "POST", "/command", Some(r#"{"command":"start-live"}"#), true);
    assert!(started.contains("200"), "{started}");
    drop(bridge);
    let _ = std::fs::remove_dir_all(&root);
}

fn json_body(response: &str) -> serde_json::Value {
    let body = response.split("\r\n\r\n").nth(1).unwrap_or("");
    serde_json::from_str(body).unwrap_or_else(|error| panic!("{error}: {response}"))
}

#[test]
fn vendored_shell_is_built_from_the_approved_sources() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../packages/ui");
    let source = std::fs::read_to_string(root.join("compose/shell.tsx")).unwrap();
    for required in [
        "white-acrylic",
        "black-acrylic",
        "liquid-glass",
        "AuthenticatedLayout",
        "liquidGlass",
        "FadeIn",
        "InkSwitch",
        "SignalReturn",
        "choose-location",
        "start-live",
        "enter-history",
        "strata.rescue",
    ] {
        assert!(source.contains(required), "{required}");
    }
    let stamp: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(root.join("dist/stamp.json")).unwrap())
            .unwrap();
    for item in stamp["files"].as_array().unwrap() {
        let path = root.join(item["path"].as_str().unwrap());
        let digest = sha256(&read_source(&path));
        assert_eq!(digest, item["sha256"].as_str().unwrap(), "{}", path.display());
    }
    let register: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(root.join("upstream.json")).unwrap()).unwrap();
    assert!(register["files"].as_array().unwrap().iter().any(|row| {
        row["repository"].as_str() == Some("satnaing/shadcn-admin")
    }));
    assert!(register["files"].as_array().unwrap().iter().any(|row| {
        row["repository"].as_str() == Some("kubai087/glasswork")
    }));
    assert!(register["files"].as_array().unwrap().iter().any(|row| {
        row["repository"].as_str() == Some("Subhan-code/Amicro--Micro-transitions-")
    }));
    for row in register["files"].as_array().unwrap() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .join(row["productPath"].as_str().unwrap());
        let digest = sha256(&read_source(&path));
        assert_eq!(digest, row["sha256"].as_str().unwrap(), "{}", path.display());
        if row["modifications"].as_str() == Some("none") {
            assert_eq!(row["sha256"], row["upstreamSha256"], "{}", path.display());
        } else {
            assert_ne!(row["sha256"], row["upstreamSha256"], "{}", path.display());
        }
    }
    let glass = std::fs::read(root.join("vendor/glasswork/glass.css")).unwrap();
    assert_eq!(glass, std::fs::read(root.join("dist/glass.css")).unwrap());
    let bundle = std::fs::read_to_string(root.join("dist/shell.js")).unwrap();
    assert_eq!(bundle.matches("react/cjs/react.production.js:").count(), 1);
    assert!(!root.join("static/shell.js").exists());
    assert!(!root.join("static/themes.css").exists());
    let workspace = std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("../../Cargo.toml")).unwrap();
    assert!(!workspace.to_ascii_lowercase().contains("fluent"));
    assert!(!workspace.to_ascii_lowercase().contains("astryx"));
    let moments = root.join("vendor/ui-moments");
    let moments_text = std::fs::read_to_string(moments.join("gallery/originals/index.html")).unwrap()
        + &std::fs::read_to_string(moments.join("gallery/loaders/originals.mjs")).unwrap()
        + &std::fs::read_to_string(moments.join("gallery/loaders/motifs.mjs")).unwrap();
    let lowered = moments_text.to_ascii_lowercase();
    assert!(!lowered.contains("fluentui"));
    assert!(!lowered.contains("astryx"));
    assert!(moments_text.contains("standalone:ink-switch"));
    assert!(moments_text.contains("signal-return"));
    let html = std::fs::read_to_string(moments.join("gallery/originals/index.html")).unwrap();
    let script = section(&html, "/* standalone:ink-switch-script */", "/* /standalone:ink-switch-script */");
    let mounted = std::fs::read_to_string(root.join("compose/ink-switch.js")).unwrap();
    assert!(
        normalize(&mounted).contains(&normalize(script.trim())),
        "ink switch script drifted"
    );
}

fn normalize(text: &str) -> String {
    text.lines().map(str::trim_end).collect::<Vec<_>>().join("\n")
}

fn read_source(path: &std::path::Path) -> Vec<u8> {
    // 登记记录的是 LF 字节。Windows 检出可能把文本改成 CRLF。
    let bytes = std::fs::read(path).unwrap();
    if bytes.contains(&b'\r') {
        bytes.into_iter().filter(|byte| *byte != b'\r').collect()
    } else {
        bytes
    }
}

fn section(source: &str, start: &str, end: &str) -> String {
    let from = source.find(start).expect(start) + start.len();
    let to = source[from..].find(end).expect(end) + from;
    source[from..to].to_owned()
}

fn sha256(bytes: &[u8]) -> String {
    // 测试只核对登记，不引入新的摘要 crate。
    fn encode(input: &[u8]) -> [u8; 32] {
        // SHA-256，FIPS 180-4。
        const K: [u32; 64] = [
            0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4,
            0xab1c5ed5, 0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe,
            0x9bdc06a7, 0xc19bf174, 0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f,
            0x4a7484aa, 0x5cb0a9dc, 0x76f988da, 0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7,
            0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967, 0x27b70a85, 0x2e1b2138, 0x4d2c6dfc,
            0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85, 0xa2bfe8a1, 0xa81a664b,
            0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070, 0x19a4c116,
            0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
            0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7,
            0xc67178f2,
        ];
        let mut state = [
            0x6a09e667u32, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab,
            0x5be0cd19,
        ];
        let bit_len = (input.len() as u64) * 8;
        let mut data = input.to_vec();
        data.push(0x80);
        while (data.len() % 64) != 56 {
            data.push(0);
        }
        data.extend_from_slice(&bit_len.to_be_bytes());
        for chunk in data.chunks(64) {
            let mut words = [0u32; 64];
            for (index, word) in chunk.chunks(4).enumerate() {
                words[index] = u32::from_be_bytes(word.try_into().unwrap());
            }
            for index in 16..64 {
                let s0 = words[index - 15].rotate_right(7)
                    ^ words[index - 15].rotate_right(18)
                    ^ (words[index - 15] >> 3);
                let s1 = words[index - 2].rotate_right(17)
                    ^ words[index - 2].rotate_right(19)
                    ^ (words[index - 2] >> 10);
                words[index] = words[index - 16]
                    .wrapping_add(s0)
                    .wrapping_add(words[index - 7])
                    .wrapping_add(s1);
            }
            let mut value = state;
            for index in 0..64 {
                let s1 = value[4].rotate_right(6) ^ value[4].rotate_right(11) ^ value[4].rotate_right(25);
                let ch = (value[4] & value[5]) ^ (!value[4] & value[6]);
                let temp1 = value[7]
                    .wrapping_add(s1)
                    .wrapping_add(ch)
                    .wrapping_add(K[index])
                    .wrapping_add(words[index]);
                let s0 = value[0].rotate_right(2) ^ value[0].rotate_right(13) ^ value[0].rotate_right(22);
                let maj = (value[0] & value[1]) ^ (value[0] & value[2]) ^ (value[1] & value[2]);
                let temp2 = s0.wrapping_add(maj);
                value[7] = value[6];
                value[6] = value[5];
                value[5] = value[4];
                value[4] = value[3].wrapping_add(temp1);
                value[3] = value[2];
                value[2] = value[1];
                value[1] = value[0];
                value[0] = temp1.wrapping_add(temp2);
            }
            for index in 0..8 {
                state[index] = state[index].wrapping_add(value[index]);
            }
        }
        let mut out = [0u8; 32];
        for (index, word) in state.iter().enumerate() {
            out[index * 4..index * 4 + 4].copy_from_slice(&word.to_be_bytes());
        }
        out
    }
    encode(bytes).iter().map(|byte| format!("{byte:02x}")).collect()
}

fn http(bridge: &Bridge, method: &str, path: &str, body: Option<&str>, cookie: bool) -> String {
    let body = body.unwrap_or("");
    let cookie = if cookie {
        let page = first_page(bridge);
        format!("Cookie: winstrata_gui={}\r\n", cookie_value(&page))
    } else {
        String::new()
    };
    let mut stream = TcpStream::connect(bridge.address()).unwrap();
    let request = format!(
        "{method} {path} HTTP/1.1\r\nHost: 127.0.0.1\r\nContent-Length: {}\r\n{cookie}Connection: close\r\n\r\n{body}",
        body.len()
    );
    stream.write_all(request.as_bytes()).unwrap();
    let mut response = String::new();
    stream.read_to_string(&mut response).unwrap();
    response
}

fn first_page(bridge: &Bridge) -> String {
    let mut stream = TcpStream::connect(bridge.address()).unwrap();
    let request = format!(
        "GET /?token={} HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\n\r\n",
        bridge.token()
    );
    stream.write_all(request.as_bytes()).unwrap();
    let mut response = String::new();
    stream.read_to_string(&mut response).unwrap();
    response
}

fn cookie(page: &str) -> String {
    cookie_value(page)
}

fn cookie_value(page: &str) -> String {
    let marker = "winstrata_gui=";
    let start = page.find(marker).expect("cookie");
    page[start + marker.len()..]
        .split([';', '\r'])
        .next()
        .unwrap()
        .to_owned()
}

#[test]
fn tray_tip_shows_pending_storage_categories_and_keeps_files() {
    assert_eq!(format_tray_tip("not-json"), "WinStrata");
    assert_eq!(
        format_tray_tip(r#"{"capture":{"pending":false},"logs":{"pending":false}}"#),
        "WinStrata"
    );
    assert_eq!(
        format_tray_tip(r#"{"capture":{"pending":true},"logs":{"pending":false}}"#),
        "WinStrata — storage pending: capture"
    );
    assert_eq!(
        format_tray_tip(r#"{"capture":{"pending":false},"logs":{"pending":true}}"#),
        "WinStrata — storage pending: logs"
    );

    let root = scratch("tray-tip");
    let _ = std::fs::remove_dir_all(&root);
    let preferences = root.join("prefs");
    let locator = root.join("locator");
    let storage = root.join("storage");
    std::fs::create_dir_all(&locator).unwrap();
    std::fs::create_dir_all(&storage).unwrap();
    let user = {
        let mut core = winstrata_core::Core::open_at(&locator, None).unwrap();
        core.select_directory(&storage).unwrap();
        core.append_saved_log("probe.log", b"retained-log-bytes\n")
            .unwrap();
        core.configure_storage(1, 1, 1, 1).unwrap();
        core.current_user().to_owned()
    };
    let assets = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../packages/ui/dist");
    let bridge = Bridge::start(preferences, assets, locator, None).unwrap();
    let status = http(
        &bridge,
        "POST",
        "/command",
        Some(r#"{"command":"storage-status"}"#),
        true,
    );
    let body = status.split("\r\n\r\n").nth(1).unwrap_or("");
    assert_eq!(format_tray_tip(body), "WinStrata — storage pending: logs");
    assert!(storage.join(user).join("logs/probe.log").is_file());
    drop(bridge);
    let _ = std::fs::remove_dir_all(&root);
}

#[cfg(windows)]
#[test]
fn windows_official_window_attaches_webview2_resizes_hides_to_tray_and_quits() {
    use winstrata_desktop::{
        close_current_window, current_window_visible, last_window_snapshot, loader_present,
        resize_current_window, restore_current_window, run_window,
    };

    assert!(loader_present(), "WebView2Loader.dll was not discovered");
    let root = scratch("win-host");
    let _ = std::fs::remove_dir_all(&root);
    let preferences = root.join("prefs");
    let locator = root.join("locator");
    let storage = root.join("storage");
    std::fs::create_dir_all(&locator).unwrap();
    std::fs::create_dir_all(&storage).unwrap();
    let user = {
        let mut core = winstrata_core::Core::open_at(&locator, None).unwrap();
        core.select_directory(&storage).unwrap();
        core.append_saved_log("probe.log", b"tray-keeps-this-log\n")
            .unwrap();
        core.configure_storage(1, 1, 1, 1).unwrap();
        core.current_user().to_owned()
    };
    let assets = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../packages/ui/dist");
    let bridge = Bridge::start(preferences, assets, locator, None).unwrap();
    let set_tray = http(
        &bridge,
        "POST",
        "/command",
        Some(r#"{"command":"set-close","close":"tray"}"#),
        true,
    );
    assert!(set_tray.contains("200"), "{set_tray}");

    let url = format!("http://{}/?token={}", bridge.address(), bridge.token());
    let token = bridge.token().to_owned();
    let action = bridge.action();
    let thread = std::thread::spawn(move || {
        run_window(&url, &token, action);
    });

    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    while std::time::Instant::now() < deadline {
        if last_window_snapshot().0 {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    assert!(
        last_window_snapshot().0,
        "WebView2 controller did not attach within 10s"
    );
    assert!(resize_current_window(480, 700));
    assert!(close_current_window());
    let hide_deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    while std::time::Instant::now() < hide_deadline {
        if current_window_visible() == Some(false) {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(25));
    }
    assert_eq!(current_window_visible(), Some(false));
    assert_eq!(
        last_window_snapshot().1,
        "WinStrata — storage pending: logs"
    );
    assert!(storage.join(user).join("logs/probe.log").is_file());

    assert!(restore_current_window());
    let show_deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    while std::time::Instant::now() < show_deadline {
        if current_window_visible() == Some(true) {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(25));
    }
    assert_eq!(current_window_visible(), Some(true));

    let set_exit = http(
        &bridge,
        "POST",
        "/command",
        Some(r#"{"command":"set-close","close":"exit"}"#),
        true,
    );
    assert!(set_exit.contains("200"), "{set_exit}");
    assert!(close_current_window());
    thread.join().unwrap();
    drop(bridge);
    let _ = std::fs::remove_dir_all(&root);
}
