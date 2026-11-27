import assert from 'node:assert/strict';
import { spawn } from 'node:child_process';
import { randomUUID } from 'node:crypto';
import { once } from 'node:events';
import { mkdtemp, readFile, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { createInterface } from 'node:readline';
import test from 'node:test';
import { fileURLToPath } from 'node:url';

const revision = '2026-07-28';
const versionKey = 'io.modelcontextprotocol/protocolVersion';
const capabilitiesKey = 'io.modelcontextprotocol/clientCapabilities';
const grantKey = 'winstrata.example/probe-grant';
const tool = { name: 'probe_pid', inputSchema: { type: 'object', additionalProperties: false } };
const noCache = { cacheScope: 'private', ttlMs: 0 };

function send(stream, message) {
  stream.write(JSON.stringify({ jsonrpc: '2.0', ...message }) + '\n');
}

// 仅验证本机管道与逐请求授权。测试进程独立管理授权文件。
async function serve(grantFile) {
  const input = createInterface({ input: process.stdin, crlfDelay: Infinity });
  for await (const line of input) {
    const { id, method, params } = JSON.parse(line);
    const meta = params?._meta ?? {};
    const error = (code, message, data) => send(process.stdout, { id, error: { code, message, data } });
    const complete = (fields) => send(process.stdout, {
      id, result: { resultType: 'complete', ...fields },
    });
    if (!Object.hasOwn(meta, versionKey) || !Object.hasOwn(meta, capabilitiesKey)) {
      error(-32602, 'Missing request metadata');
      continue;
    }
    if (meta[versionKey] !== revision) {
      error(-32022, 'Unsupported protocol version', { supported: [revision], requested: meta[versionKey] });
      continue;
    }
    if (method === 'server/discover') {
      complete({
        ...noCache, supportedVersions: [revision], capabilities: { tools: {} },
        _meta: { 'io.modelcontextprotocol/serverInfo': { name: 'stdio-probe', version: '1' } },
      });
      continue;
    }
    const activeGrant = await readFile(grantFile, 'utf8');
    const allowed = Boolean(activeGrant) && activeGrant === meta[grantKey];
    if (method === 'tools/list') {
      complete({ ...noCache, tools: allowed ? [tool] : [] });
    } else if (method === 'tools/call' && allowed && params.name === tool.name) {
      const args = params.arguments === undefined ? {} : params.arguments;
      if (args === null || typeof args !== 'object' || Array.isArray(args)) {
        error(-32602, 'Invalid tools/call arguments');
        continue;
      }
      if (Object.keys(args).length > 0) {
        complete({ isError: true, content: [{ type: 'text', text: 'Expected an empty argument object' }] });
        continue;
      }
      if (Object.hasOwn(meta, 'progressToken')) {
        send(process.stdout, {
          method: 'notifications/progress',
          params: { progressToken: meta.progressToken, progress: 1, total: 1 },
        });
      }
      complete({ content: [{ type: 'text', text: String(process.pid) }] });
    } else if (method === 'tools/call') {
      error(-32602, 'Unknown tool');
    } else {
      error(-32601, 'Unknown method');
    }
  }
}

if (process.argv[2] === '--serve') {
  await serve(process.argv[3]);
} else {
  test('MCP stdio discovery, version checks, scoped calls, revocation and EOF', { timeout: 10_000 }, async (t) => {
    const dir = await mkdtemp(join(tmpdir(), 'winstrata-mcp-'));
    t.after(() => rm(dir, { recursive: true, force: true }));
    const grantFile = join(dir, 'grant');
    const grant = randomUUID();
    await writeFile(grantFile, grant, { mode: 0o600 });
    const child = spawn(process.execPath, [fileURLToPath(import.meta.url), '--serve', grantFile], {
      stdio: ['pipe', 'pipe', 'inherit'], windowsHide: true, timeout: 8000,
    });
    t.after(() => child.kill());
    const closed = once(child, 'close');
    const lines = createInterface({ input: child.stdout, crlfDelay: Infinity })[Symbol.asyncIterator]();
    const next = async () => {
      const line = await lines.next();
      assert.equal(line.done, false, 'unexpected EOF');
      const message = JSON.parse(line.value);
      assert.equal(message.jsonrpc, '2.0');
      return message;
    };
    let id = 0;
    const clientMeta = {
      [versionKey]: revision, [capabilitiesKey]: {},
      'io.modelcontextprotocol/clientInfo': { name: 'probe-client', version: '1' },
    };
    const request = async (method, params = {}, meta = clientMeta) => {
      send(child.stdin, { id: ++id, method, params: { ...params, _meta: meta } });
      if (Object.hasOwn(meta, 'progressToken')) {
        const notification = await next();
        assert.deepEqual(notification, { jsonrpc: '2.0', method: 'notifications/progress',
          params: { progressToken: meta.progressToken, progress: 1, total: 1 } });
      }
      const response = await next();
      assert.equal(response.id, id);
      if (response.result) assert.equal(response.result.resultType, 'complete');
      return response;
    };
    const discovery = await request('server/discover');
    assert.deepEqual(discovery.result.supportedVersions, [revision]);
    assert.deepEqual(discovery.result.capabilities, { tools: {} });
    assert.equal(discovery.result.ttlMs, 0);
    assert.equal(discovery.result.cacheScope, 'private');
    assert.equal((await request('tools/list', {}, {})).error.code, -32602);
    const mismatch = await request('tools/list', {}, { ...clientMeta, [versionKey]: '1900-01-01' });
    assert.equal(mismatch.error.code, -32022);
    assert.deepEqual(mismatch.error.data, { supported: [revision], requested: '1900-01-01' });
    const call = { name: 'probe_pid', arguments: {} };
    const authorized = { ...clientMeta, [grantKey]: grant };
    assert.deepEqual((await request('tools/list')).result.tools, []);
    assert.equal((await request('tools/call', call)).error.code, -32602);
    const listed = (await request('tools/list', {}, authorized)).result;
    assert.deepEqual(listed.tools, [tool]);
    assert.equal(listed.ttlMs, 0);
    assert.equal(listed.cacheScope, 'private');
    assert.equal((await request('tools/call', { ...call, arguments: null }, authorized)).error.code, -32602);
    const invalidCall = await request('tools/call', { ...call, arguments: { extra: 1 } }, authorized);
    assert.equal(invalidCall.result.isError, true);
    const invocation = await request('tools/call', call, { ...authorized, progressToken: 'pid-read' });
    assert.deepEqual(invocation.result.content, [{ type: 'text', text: String(child.pid) }]);
    await writeFile(grantFile, '');
    assert.deepEqual((await request('tools/list', {}, authorized)).result.tools, []);
    assert.equal((await request('tools/call', call, authorized)).error.code, -32602);
    child.stdin.end();
    assert.deepEqual(await closed, [0, null]);
    assert.equal((await lines.next()).done, true);
  });
}
