# Services error register

[简体中文](../../zh-CN/records/modules/services.md) · [Global register](../ERRORS.md)

Authoritative error records for `services`. The crate directory is `services/`.

<a id="err-017"></a>

## ERR-017

- Error ID: ERR-017
- Discovery time: 2026-11-27. First Windows failure on `4f7dbf1254b647e92d1b06110cc3f4dfe86c8a8d`. Second Windows failure on `c056e32389f0e2a75ec0e1407d0e253a5cdde8ab`. Both were during plan 008.
- Related PR: GitHub PR #12, plan 008, before review head `fe4b59117f7b3948f2e1a6ecdcc181ab11ea9795`
- Environment: GitHub Actions `windows-2022`
- Reproduction method: the four service pipe tests in `cargo test --locked --workspace`
- Evidence: on `4f7dbf1`, three clients saw Win32 error 2 because the pipe did not exist when `listen` returned. The fourth test denied the restricted token, and the following ordinary client saw Win32 error 232. [Windows job](https://github.com/zemeng5208/winstrata/actions/runs/45998537771/job/140503975068). On `c056e32`, three clients saw end of stream or a transport error without a native code, and the ordinary client again saw Win32 error 232. [Windows job](https://github.com/zemeng5208/winstrata/actions/runs/45999133804/job/140506021647). Linux passed on both commits.
- Root cause: the listener returned before the pipe instance existed. After that was corrected, the client had not requested impersonation, so the service closed the pipe instead of answering.
- Impact: Windows workspace checks failed on both heads
- Fix plan: create the instance before `listen` returns and keep it open while polling. Open the client pipe with `SECURITY_IMPERSONATION`. Implemented in plan 008 before merge.
- Retest result: `fe4b59117f7b3948f2e1a6ecdcc181ab11ea9795` passed 7 Windows service tests, including the pipe exchange and the restricted-token denial. [Windows job](https://github.com/zemeng5208/winstrata/actions/runs/45999706617/job/140507919585).
- Current state: resolved

No other services error is registered. The missing Service Control Manager registration of `winstrata-service` is [OI-014](../OPEN_ITEMS.md#oi-014), not a confirmed functional failure of a test that was run.
