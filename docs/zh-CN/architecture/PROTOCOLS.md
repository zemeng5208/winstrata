# 公共观测协议 v1

[English](../../architecture/PROTOCOLS.md) · [Rust 库](../../../protocols/src/lib.rs) · [计划 005](../roadmap/PLAN.md#pr-005)

Core、采集器和插件可使用同一组事件、对象引用和诊断类型。JSON 是此次跨语言兼容验证使用的参考表示。服务消息分帧、身份认证、授权和审计由 PR 008 实现。记录中的来源标识和观测到的用户不授予任何权限。

## 读写与兼容

使用 `Event::from_json`、`Event::to_json`，或 `ProtocolError` 的同名方法。1.0 写入端输出 JSON 对象。读取端可读后续 1.x 次版本新增的可选字段，跳过无法识别的字段，并拒绝未知的记录类型或缺口原因。改变必需语义须升级主版本。JSON 入口先读取版本，再解释记录，保证收到不兼容主版本时返回 `unsupported_version`。

1.0 写入端拒绝按未来次版本重写数据。需要无损转发时须保留原始消息。显式生成 1.0 投影时，仅保留 1.0 能理解的字段。调用方按稳定错误码处理，并保留原生错误码所属的域。解码器使用 `invalid_json`（语法或截断）、`invalid_record`（结构、取值或约束）、`unsupported_version`。诊断中不回显输入值。应用可定义新错误码，例如 `source.access_denied`，旧调用方仍能读取。

```json
{"version":{"major":1,"minor":0},"code":"source.access_denied","detail":"Cannot open this event channel","native":{"domain":"win32","code":"5"}}
```

## 身份、时间和不完整观测

| 字段 | 生产者与消费者约定 |
|---|---|
| `source` 与 `sequence` | 事件键由设备、提供方、来源实例和序号组成。生产者提供稳定的设备和提供方标识，序号重置时启用新的唯一实例，并在各实例内分配递增正序号。消费者在同一来源实例内按序号排序。不同来源独立计数。 |
| `time` | `unix_ns` 为距 UTC Unix 纪元的有符号纳秒值。已知观测精度时，来源可提供正数 `precision_ns`；未知则省略。校时允许观测时间回退。计量单位本身无法证明时钟精度。 |
| `subject`、`user`、`process` | 对象引用由设备、对象类型和稳定标识组成。来源记录号需包含保证唯一性所需的通道或代次。用户标识在设备范围内解释。进程实例由设备、系统启动实例、PID 和进程启动时间确定。仅能观测 PID 时可记录这项证据，并省略无法确定的进程实例。 |
| 归因与完整性 | 直接观测 `observed` 和推断 `inferred` 均携带证据引用，无法归因使用 `unknown`。声明范围内完整时使用 `complete`，部分观测使用 `partial` 并给出原因。可选身份字段允许缺失。生产者须区分无法获得的值和已观测值。 |
| 标量 | 序号、区间边界及已知精度为正的无符号 64 位整数，时间为有符号 64 位整数。JSON 使用规范十进制字符串以保留 JavaScript 精度，计算可用 `BigInt`。有符号时间的零写为 `"0"`。数字字面量、前导零、正号、负零及溢出均无效。标识符占 1–256 个 UTF-8 字节，不允许首尾空白或控制字符，保留内部空格与原始 Unicode 拼写。 |

下面是说明性观测。PID 已知，启动时间未知，因此保留 PID 证据并说明不完整原因。

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

## 来源缺口

下面的说明性标记表示序号 12 至 14 丢失，含两个端点。区间与报告它的标记属于同一来源实例，且两个端点均早于标记序号。

```json
{
  "version": {"major": 1, "minor": 0},
  "source": {"device": "example-device", "provider": "example.events", "epoch": "run-a"},
  "sequence": "16",
  "time": {"unix_ns": "1800000000000000000"},
  "record": {"kind": "gap", "reason": "lost", "range": {"first": "12", "last": "14"}}
}
```

根据实际原因选择 `filtered`（过滤）、`sampled`（采样）、`paused`（暂停）、`source_failure`（来源故障）或 `lost`（丢失）。区间未知则省略 `range`。一个标记只能说明本来源实例的覆盖情况，解码器不会因时钟回退而补造丢失记录。跨批次核对与背压由 PR 007 处理。

## 验证与依赖

在工作区根目录用固定的 Rust 和 Node.js 执行 `cargo test --locked -p winstrata-protocols`。Rust 观测器启动真实 Node 子进程，双方通过 JSON 往返核对其实际 PID。确定性回放覆盖 PID 复用、时钟回退与来源重启。Windows 专用测试读取一条真实 System 日志，再按记录编号独立查询，核对序列化后的提供方和时间戳。测试使用临时设备别名，日志不输出事件正文或用户身份。[互操作测试](../../../protocols/tests/interop.rs) · [Windows 来源测试](../../../protocols/tests/windows_event.rs)

工作区固定启用 derive 的 serde 1.0.229 与 serde_json 1.0.154，Cargo 锁定含构建期宏在内的 11 个外部包。2026-11-27 已核对各包在 crates.io 的版本、校验和与撤回状态，逐一按精确版本查询 OSV，未查到已列出的漏洞。两项直接依赖均提供 MIT 或 Apache-2.0。锁定依赖还包括 memchr（Unlicense 或 MIT）、unicode-ident（MIT 或 Apache-2.0，并须满足 Unicode-3.0）及 zmij（MIT）。打包时保留适用声明。[Cargo 锁文件](../../../Cargo.lock) · [serde](https://crates.io/crates/serde/1.0.229) · [serde_json](https://crates.io/crates/serde_json/1.0.154) · [OSV 查询](https://google.github.io/osv.dev/post-v1-querybatch/)
