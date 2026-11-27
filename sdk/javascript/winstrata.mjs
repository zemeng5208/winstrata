/**
 * WinStrata JavaScript / TypeScript 插件协议。
 * 模块只从插件私有目录和已声明的项目目录加载。
 */

import { existsSync, lstatSync, readFileSync, readSync, writeSync } from "node:fs";
import { pathToFileURL } from "node:url";
import path from "node:path";

export async function serve() {
  const args = parseArgs(process.argv.slice(2));
  const manifest = readManifest(args.manifest);
  if (declined(args.manifest)) process.exit(3);
  if (!existsSync(args.entry)) process.exit(3);
  const plugin = await import(pathToFileURL(args.entry).href);
  globalThis.winstrata = {
    loadModule(name) {
      return loadModule(name, args.private, args.catalog);
    },
  };
  let phase = "running";
  for (;;) {
    const request = readFrame();
    if (request === null) return;
    const [response, exitCode] = await handle(manifest, args.state, plugin, phase, request);
    if (request.op === "pause" || request.op === "resume") phase = response.phase;
    writeFrame(response);
    if (exitCode !== undefined) process.exit(exitCode);
  }
}

function parseArgs(argv) {
  const values = {};
  for (let index = 0; index < argv.length; index += 2) {
    const name = argv[index];
    if (!name?.startsWith("--") || argv[index + 1] === undefined) process.exit(1);
    values[name.slice(2)] = argv[index + 1];
  }
  for (const name of ["manifest", "state", "entry", "private"]) {
    if (!values[name]) process.exit(1);
  }
  return values;
}

function readManifest(file) {
  const document = JSON.parse(readFileSync(file, "utf8"));
  if (!document.plugin_version) process.exit(1);
  return document;
}

function declined(manifestPath) {
  const file = path.join(path.dirname(manifestPath), "startup.json");
  if (!existsSync(file)) return false;
  try {
    return JSON.parse(readFileSync(file, "utf8")).ready !== true;
  } catch {
    return true;
  }
}

export async function loadModule(name, privateDir, catalog) {
  if (!/^[a-z][a-z0-9_]{0,31}$/.test(name)) throw new Error("rejected");
  const privateFile = path.join(privateDir, `${name}.mjs`);
  if (existsSync(privateFile)) return import(pathToFileURL(privateFile).href);
  if (catalog) {
    const catalogFile = path.join(catalog, `${name}.mjs`);
    if (existsSync(catalogFile)) return import(pathToFileURL(catalogFile).href);
  }
  throw new Error("missing");
}

async function handle(manifest, state, plugin, phase, request) {
  const pid = process.pid;
  switch (request.op) {
    case "ping":
      return [reply(manifest, true, "ok", phase, phase, pid)];
    case "pause":
      return [reply(manifest, true, "ok", "paused", "paused", pid)];
    case "resume":
      return [reply(manifest, true, "ok", "running", "running", pid)];
    case "stop":
    case "disable":
      return [reply(manifest, true, "ok", phase, phase, pid), 0];
    case "fault":
      return [reply(manifest, true, "ok", "faulted", phase, pid), 7];
    case "emit": {
      const eventType = request.event_type ?? "";
      if ((manifest.events ?? []).includes(eventType)) {
        return [reply(manifest, true, "ok", eventType, phase, pid)];
      }
      return [reply(manifest, false, "rejected", "event is not declared", phase, pid)];
    }
    case "invoke": {
      const capability = request.capability ?? "";
      if (!(manifest.capabilities ?? []).includes(capability)) {
        return [reply(manifest, false, "rejected", "capability is not provided", phase, pid)];
      }
      if (typeof plugin.invoke !== "function") {
        return [reply(manifest, false, "rejected", "invoke is not defined", phase, pid)];
      }
      try {
        const detail = await plugin.invoke(capability, request.payload ?? "");
        if (typeof detail !== "string") {
          return [reply(manifest, false, "rejected", "invoke result is not text", phase, pid)];
        }
        return [reply(manifest, true, "ok", detail, phase, pid)];
      } catch (error) {
        return [reply(manifest, false, "faulted", String(error), phase, pid)];
      }
    }
    case "read_state":
      try {
        const text = readState(state, request.name ?? "");
        return [reply(manifest, true, "ok", text, phase, pid)];
      } catch (error) {
        const code = error instanceof Error ? error.message : "rejected";
        return [reply(manifest, false, code, code, phase, pid)];
      }
    default:
      return [reply(manifest, false, "rejected", "operation is not supported", phase, pid)];
  }
}

function readState(state, name) {
  if (
    !/^[a-z][a-z0-9._-]{0,39}$/.test(name) ||
    name.includes("..")
  ) {
    throw new Error("rejected");
  }
  const file = path.join(state, name);
  if (path.dirname(file) !== state) throw new Error("rejected");
  let info;
  try {
    info = lstatSync(file);
  } catch {
    throw new Error("missing");
  }
  if (!info.isFile() || info.isSymbolicLink() || info.size > 1024) throw new Error("rejected");
  const text = readFileSync(file, "utf8");
  if ([...text].some((character) => character.charCodeAt(0) < 32 || character.charCodeAt(0) === 127)) {
    throw new Error("rejected");
  }
  return text;
}

function reply(manifest, ok, code, detail, phase, pid) {
  return {
    version: { major: 1, minor: 0 },
    op: "result",
    ok,
    code,
    detail: bound(detail),
    phase,
    pid,
    plugin_version: manifest.plugin_version,
  };
}

function bound(value) {
  const text = [...value]
    .filter((character) => character.charCodeAt(0) >= 32 && character.charCodeAt(0) !== 127)
    .join("")
    .slice(0, 1024);
  return text || " ";
}

function readFrame() {
  const header = Buffer.alloc(4);
  const headerBytes = readFull(0, header);
  if (headerBytes === 0) return null;
  if (headerBytes !== 4) process.exit(1);
  const length = header.readUInt32LE(0);
  if (length <= 0 || length > 65536) process.exit(1);
  const body = Buffer.alloc(length);
  if (readFull(0, body) !== length) process.exit(1);
  return JSON.parse(body.toString("utf8"));
}

function readFull(fd, buffer) {
  let offset = 0;
  while (offset < buffer.length) {
    const count = readSync(fd, buffer, offset, buffer.length - offset);
    if (count === 0) return offset;
    offset += count;
  }
  return offset;
}

function writeFrame(document) {
  const body = Buffer.from(JSON.stringify(document));
  const header = Buffer.alloc(4);
  header.writeUInt32LE(body.length, 0);
  writeSync(1, Buffer.concat([header, body]));
}
