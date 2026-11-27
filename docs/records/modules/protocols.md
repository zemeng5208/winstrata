# Protocols error register

[简体中文](../../zh-CN/records/modules/protocols.md) · [Global register](../ERRORS.md)

Authoritative error records for `protocols`.

<a id="err-015"></a>

## ERR-015

- Error ID: ERR-015
- Discovery time: 2026-11-27, during plan 005 review, before the green candidate for `995d148`. The review comment URL is unknown.
- Related PR: GitHub PR #6, plan 005
- Environment: Linux and Windows Server 2022 protocol tests
- Reproduction method: decode a positional array or a numeric enum tag with the default decoder
- Evidence: the plan 005 record says review found that the default decoder accepted positional arrays or numeric enum tags. The named-object and string-tag checks rejected the original cases on both systems. No separate failing job URL was stored.
- Root cause: the default decoder representation accepted those forms. The replacement checks require a named object and a string tag.
- Impact: the original cases were not a valid protocol encoding
- Fix plan: implemented in plan 005 before merge
- Retest result: the candidate for `995d148` passed 11 library tests and two integration tests on Linux, and Windows also passed the live System-event test. [Linux](https://github.com/zemeng5208/winstrata/actions/runs/45971619572/job/140413162125). [Windows](https://github.com/zemeng5208/winstrata/actions/runs/45971619572/job/140413162795). All six JSON blocks across the two document languages passed typed decode, encode, and decode again.
- Current state: resolved
