import assert from 'node:assert/strict';
import process from 'node:process';

let input = '';
process.stdin.setEncoding('utf8');
for await (const chunk of process.stdin) input += chunk;
const lines = input.trimEnd().split('\n');
assert.equal(lines.length, 3);
const [live, replay, diagnostic] = lines.map(line => JSON.parse(line));

assert.deepEqual(live.version, { major: 1, minor: 0 });
assert.equal(live.record.subject.kind, 'pid_observation');
assert.equal(live.record.subject.id, String(process.pid));
assert.equal(live.record.process ?? null, null);
assert.equal(live.record.completeness.reason, 'process_start_unavailable');
assert.ok(BigInt(live.time.unix_ns) > 0n);
assert.equal(BigInt(live.sequence), 1n);
assert.equal(typeof replay.sequence, 'string');
assert.equal(BigInt(replay.sequence), 2n ** 53n + 1n);
assert.equal(BigInt(replay.time.unix_ns), -1n);
assert.equal(BigInt(replay.record.process.started_unix_ns), -2n);

assert.equal(diagnostic.code, 'invalid_json');
assert.deepEqual(diagnostic.version, { major: 1, minor: 0 });
assert.equal(diagnostic.native, undefined);

const future = {
  ...replay,
  version: { major: 1, minor: 1 },
  sequence: (BigInt(replay.sequence) + 1n).toString(),
  trace_hint: 'optional-future-field',
};
for (const event of [
  live, future,
  { ...future, version: { major: 2, minor: 0 } },
  { ...future, version: { major: '1', minor: 0 } },
  { version: { major: 1, minor: 0 }, code: 'source.access_denied',
    detail: 'Cannot open this event channel', native: { domain: 'win32', code: '5' } },
]) {
  process.stdout.write(JSON.stringify(event) + '\n');
}
