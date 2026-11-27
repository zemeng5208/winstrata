# Security standard

[简体中文](../zh-CN/standards/SECURITY.md) · [Index](../INDEX.md)

<a id="ag-23"></a>

## AG-23 Security and least privilege

Default to least privilege and validate all external input, including plugins, MCP, networks, and imports. Limit Rust unsafe and document why it is necessary and the invariants it requires. Enforce kernel, user mode, service, and plugin trust boundaries. Prohibit bypasses and unauthorized data transfers. Review attack surface in each PR. Find related security defects, fix and retest them, and block until real validation passes.

<a id="ag-47"></a>

## AG-47 Kernel drivers

Limit drivers to collection and transport using supported interfaces. Keep signing, Secure Boot, and HVCI protections in place. Avoid undocumented or unverified interfaces and hooks. Bound IRQL, locks, memory, buffers, and callback time. Keep expensive analysis and unnecessary I/O out of callbacks. Verify real Windows load, unload, recovery, leaks, crash risks, sustained load, and stress. Prioritize kernel defects and synchronize findings. Report unverified Windows work as pending.
