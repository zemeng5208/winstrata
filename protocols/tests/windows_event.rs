#![cfg(windows)]

use std::io::Read;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use serde::Deserialize;
use serde_json::Value;
use winstrata_protocols::Event;

const CAPTURE: &str = r#"
$ErrorActionPreference = 'Stop'
$ProgressPreference = 'SilentlyContinue'
[Console]::OutputEncoding = [Text.UTF8Encoding]::new($false)
$culture = [Globalization.CultureInfo]::InvariantCulture
$stage = 'read'
try {
    $query = [Diagnostics.Eventing.Reader.EventLogQuery]::new('System', [Diagnostics.Eventing.Reader.PathType]::LogName)
    $query.ReverseDirection = $true
    $reader = [Diagnostics.Eventing.Reader.EventLogReader]::new($query)
    try {
        $entry = $reader.ReadEvent([TimeSpan]::FromSeconds(2))
        if ($null -eq $entry) { throw 'No System event available.' }
        try {
            if ($null -eq $entry.RecordId -or $null -eq $entry.TimeCreated -or -not $entry.ProviderName) {
                throw 'Record lacks required System metadata.'
            }
            $id = $entry.RecordId.ToString($culture)
            $utc = $entry.TimeCreated.ToUniversalTime()
            $unixNs = ([decimal]$utc.Ticks - 621355968000000000L) * 100
            $epoch = 'read-' + [guid]::NewGuid().ToString('N')
            $event = @{
                version = @{ major = 1; minor = 0 }
                source = @{ device = 'test-device-' + [guid]::NewGuid().ToString('N')
                            epoch = $epoch; provider = $entry.ProviderName }
                sequence = '1'
                time = @{ unix_ns = $unixNs.ToString('0', $culture) }
                record = @{ kind = 'observation'; event_type = 'windows.system_event'
                            subject = @{ kind = 'event_log_sample'; id = ('System/{0}/{1}' -f $epoch, $id) }
                            attribution = @{ kind = 'observed'; evidence = 'windows.system_log' }
                            completeness = @{ kind = 'complete' } }
            }
        } finally { $entry.Dispose() }
    } finally { $reader.Dispose() }

    $stage = 'read-back'
    $lookup = '*[System[EventRecordID={0}]]' -f $id
    $control = Get-WinEvent -LogName System -FilterXPath $lookup -MaxEvents 1
    try {
        $metadata = @{ provider = $control.ProviderName; record_id = $control.RecordId.ToString($culture)
                       filetime = $control.TimeCreated.ToFileTimeUtc().ToString($culture) }
    } finally { $control.Dispose() }
    $stage = 'encode'
    [Console]::Write((@{ event = $event; metadata = $metadata } | ConvertTo-Json -Depth 8 -Compress))
} catch {
    [Console]::Error.WriteLine(('System log metadata {0} failed (HRESULT {1}).' -f $stage, $_.Exception.HResult))
    exit 1
}
"#;

#[derive(Deserialize)]
struct Capture {
    event: Value,
    metadata: Metadata,
}

#[derive(Deserialize)]
struct Metadata {
    provider: String,
    record_id: String,
    filetime: String,
}

fn capture() -> Capture {
    let system_root = std::env::var_os("SystemRoot").expect("Windows system root");
    let powershell =
        PathBuf::from(system_root).join(r"System32\WindowsPowerShell\v1.0\powershell.exe");
    let mut child = Command::new(powershell)
        .args([
            "-NoLogo",
            "-NoProfile",
            "-NonInteractive",
            "-Command",
            CAPTURE,
        ])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .spawn()
        .expect("start Windows PowerShell");
    let stdout = child.stdout.take().expect("capture stdout");
    let output = thread::spawn(move || {
        let mut bytes = Vec::new();
        stdout
            .take(65_537)
            .read_to_end(&mut bytes)
            .expect("read metadata");
        bytes
    });
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        if let Some(status) = child.try_wait().expect("poll event reader") {
            assert!(status.success(), "Windows event capture failed");
            break;
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            panic!("Windows event capture exceeded 30 seconds");
        }
        thread::sleep(Duration::from_millis(50));
    }
    let bytes = output.join().expect("join metadata reader");
    assert!(bytes.len() <= 65_536, "unexpected System metadata size");
    serde_json::from_slice(&bytes).expect("parse captured metadata")
}

#[test]
fn windows_system_event_survives_protocol_round_trip() {
    let Capture { event, metadata } = capture();
    let original = Event::from_json(&event.to_string()).expect("decode Windows observation");
    let encoded = original.to_json().expect("encode Windows observation");
    let decoded = Event::from_json(&encoded).expect("decode serialized observation");
    assert!(
        original == decoded,
        "round-trip changed the typed observation"
    );

    // 参考记录使用 1601 年起的 FILETIME，与采集端按公元元年起算的换算独立。
    let ticks: i128 = metadata.filetime.parse().expect("native FILETIME ticks");
    let expected_ns = ((ticks - 116_444_736_000_000_000_i128) * 100).to_string();
    let wire: Value = serde_json::from_str(&encoded).expect("inspect serialized fields");
    let record = &wire["record"];
    assert!(!metadata.provider.is_empty());
    assert!(wire["source"]["provider"].as_str() == Some(metadata.provider.as_str()));
    assert!(metadata.record_id.parse::<u64>().is_ok_and(|id| id > 0));
    let epoch = wire["source"]["epoch"].as_str().expect("read session");
    let subject = format!("System/{epoch}/{}", metadata.record_id);
    assert!(record["subject"]["id"].as_str() == Some(subject.as_str()));
    assert!(wire["time"]["unix_ns"].as_str() == Some(expected_ns.as_str()));
    // FILETIME 的计量单位为 100 ns。来源未报告其时钟精度。
    assert!(wire["time"].get("precision_ns").is_none());
    assert_eq!(wire["version"], serde_json::json!({"major": 1, "minor": 0}));
    assert_eq!(wire["sequence"], "1");
    assert_eq!(record["kind"], "observation");
    assert_eq!(record["event_type"], "windows.system_event");
    assert_eq!(record["subject"]["kind"], "event_log_sample");
    assert_eq!(record["attribution"]["kind"], "observed");
    assert_eq!(record["attribution"]["evidence"], "windows.system_log");
    assert_eq!(record["completeness"]["kind"], "complete");
    for (key, prefix) in [("device", "test-device-"), ("epoch", "read-")] {
        let nonce = wire["source"][key]
            .as_str()
            .and_then(|id| id.strip_prefix(prefix));
        assert!(nonce.is_some_and(|n| n.len() == 32 && n.bytes().all(|b| b.is_ascii_hexdigit())));
    }
}
