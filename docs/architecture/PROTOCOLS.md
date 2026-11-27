# Public observation protocol v1

[简体中文](../zh-CN/architecture/PROTOCOLS.md) · [Rust library](../../protocols/src/lib.rs) · [Plan 005](../roadmap/PLAN.md#pr-005)

Core, collectors and plugins can exchange the same event, object reference and diagnostic types. The JSON representation is the reference for cross-language compatibility. PR 008 owns service framing, authenticated identity, authorization and auditing. A source ID or an observed user in a record confers no permissions.

## Reading and writing

Call `Event::from_json` and `Event::to_json`, or the matching methods on `ProtocolError`. Version 1.0 writers emit JSON objects. Readers accept later 1.x minor versions with optional additive fields, ignore fields they do not recognize and reject unknown record kinds or gap reasons. Breaking required semantics need a new major version. The JSON entry point reads the version before interpreting the record so an incompatible major consistently yields `unsupported_version`.

A 1.0 writer rejects a future minor version. An intermediary that needs lossless forwarding must retain the original message. Explicitly creating a new 1.0 projection keeps only the fields understood by 1.0. Consumers should branch on stable error codes and retain the domain with any native code. Available decoder codes are `invalid_json` (syntax or truncation), `invalid_record` (shape, scalar or invariant), and `unsupported_version`. The diagnostic excludes input values. Additional application-defined codes, such as `source.access_denied`, can be read even when newer than the consumer.

```json
{"version":{"major":1,"minor":0},"code":"source.access_denied","detail":"Cannot open this event channel","native":{"domain":"win32","code":"5"}}
```

## Identity, time and incomplete observations

| Field | Producer and consumer contract |
|---|---|
| `source` and `sequence` | An event key is (device, provider, epoch, sequence). The producer supplies stable device and provider IDs, starts a new unique epoch on sequence reset, and issues increasing positive sequence numbers within that stream. Consumers order by sequence within the same source instance. Separate streams have independent sequence spaces. |
| `time` | `unix_ns` is signed nanoseconds since the UTC Unix epoch. A source can report positive `precision_ns` when known. Omit an unknown precision. Clock adjustments can move an observation's time backwards. Units alone give no accuracy guarantee. |
| `subject`, `user`, `process` | Object references use (device, object kind, stable ID). Source-specific record IDs must include any channel or generation needed for uniqueness. User identities are scoped to their device. A process instance uses device, boot identity, PID and process start time. When only a PID is available, attach the PID observation as evidence and omit the unresolved process identity. |
| Attribution and completeness | `observed` and `inferred` carry an evidence reference. Use `unknown` when there is no attribution. Completeness is `complete` for the declared observation scope, or `partial` with a reason. Optional IDs may be absent. Producers must preserve the distinction between unavailable and observed values. |
| Scalars | Sequences, gap bounds and known precision are positive unsigned 64-bit values. Times are signed 64-bit values. JSON carries these as canonical decimal strings for JavaScript precision. Use `BigInt` for arithmetic. Zero is represented by `"0"` for signed time. Numeric literals, leading zeroes, a plus sign, negative zero and overflow are invalid. Identifiers contain 1–256 UTF-8 bytes, with no surrounding whitespace or control characters. Interior spaces and original Unicode spelling are preserved. |

In this illustrative observation the PID was available and the process start time was unknown. The observation preserves the PID evidence and records the missing identity information.

```json
{
  "version": {"major": 1, "minor": 0},
  "source": {"device": "example-device", "provider": "example.events", "epoch": "run-a"},
  "sequence": "11",
  "time": {"unix_ns": "1799999999000000000"},
  "record": {
    "kind": "observation", "event_type": "process.observed",
    "subject": {"kind": "pid_observation", "id": "sample-11"},
    "attribution": {"kind": "observed", "evidence": "sample-11:pid=321"},
    "completeness": {"kind": "partial", "reason": "process_start_unavailable"}
  }
}
```

## Source gaps

The following illustrative marker reports that sequences 12 through 14 were lost, inclusive. Both endpoints refer to the marker's source instance and precede the marker itself.

```json
{
  "version": {"major": 1, "minor": 0},
  "source": {"device": "example-device", "provider": "example.events", "epoch": "run-a"},
  "sequence": "16",
  "time": {"unix_ns": "1800000000000000000"},
  "record": {"kind": "gap", "reason": "lost", "range": {"first": "12", "last": "14"}}
}
```

Use `filtered`, `sampled`, `paused`, `source_failure` or `lost` according to the observed cause. Omit `range` if its bounds are unknown. A single marker cannot establish the coverage of another source, and the decoder does not invent missing records from a clock change. Cross-batch reconciliation and backpressure belong to PR 007.

## Verification and dependencies

Run `cargo test --locked -p winstrata-protocols` from the workspace with the pinned Rust and Node.js tools. A Rust observer launches a real Node peer and verifies its actual PID across the JSON round trip. Deterministic replay covers PID reuse, clock rollback and a new source epoch. The Windows-only test reads one live System log record and independently queries its record ID to check its provider and timestamp after serialization. The test uses a temporary device alias and logs no event message or user identity. [Interop tests](../../protocols/tests/interop.rs) · [Windows source test](../../protocols/tests/windows_event.rs)

The workspace pins serde 1.0.229 with derive and serde_json 1.0.154. Cargo locks 11 registry packages including build-time macros. On 2026-11-27 their crates.io versions, checksums and withdrawal status were verified, and an OSV query for each exact version returned no listed vulnerability. Both direct crates offer MIT or Apache-2.0. The locked closure also includes memchr (Unlicense or MIT), unicode-ident (MIT or Apache-2.0, together with Unicode-3.0) and zmij (MIT). Retain applicable notices when packaging. [Cargo lockfile](../../Cargo.lock) · [serde](https://crates.io/crates/serde/1.0.229) · [serde_json](https://crates.io/crates/serde_json/1.0.154) · [OSV query](https://google.github.io/osv.dev/post-v1-querybatch/)
