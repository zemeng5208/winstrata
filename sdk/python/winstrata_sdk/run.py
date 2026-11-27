"""WinStrata Python 插件入口。

宿主传入清单、状态目录和模块目录。用户站点和外部环境变量不参与导入。
"""

import importlib.util
import json
import os
import sys


def main():
    args = parse_args(sys.argv[1:])
    manifest = read_manifest(args["manifest"])
    if declined(args["manifest"]):
        raise SystemExit(3)
    entry = os.path.join(args["package"], "plugin.py")
    if not os.path.isfile(entry):
        raise SystemExit(3)
    install_paths(args["private"], args.get("catalog", ""))
    plugin = load_plugin(entry)
    serve(manifest, args["state"], plugin)


def parse_args(argv):
    values = {}
    cursor = 0
    while cursor < len(argv):
        name = argv[cursor]
        if cursor + 1 >= len(argv) or not name.startswith("--"):
            raise SystemExit(1)
        values[name[2:]] = argv[cursor + 1]
        cursor += 2
    for name in ("manifest", "state", "package", "private"):
        if name not in values:
            raise SystemExit(1)
    return values


def read_manifest(path):
    with open(path, "r", encoding="utf-8") as handle:
        document = json.load(handle)
    version = document.get("plugin_version")
    if not isinstance(version, dict):
        raise SystemExit(1)
    return document


def declined(manifest_path):
    path = os.path.join(os.path.dirname(manifest_path), "startup.json")
    if not os.path.isfile(path):
        return False
    try:
        with open(path, "r", encoding="utf-8") as handle:
            document = json.load(handle)
        return document.get("ready") is not True
    except (OSError, json.JSONDecodeError):
        return True


def install_paths(private, catalog):
    script_dir = os.path.dirname(os.path.abspath(__file__))
    cleaned = []
    for entry in sys.path:
        if os.path.abspath(entry or ".") == script_dir:
            continue
        cleaned.append(entry)
    sys.path = cleaned
    sys.path.insert(0, private)
    if catalog:
        sys.path.insert(1, catalog)


def load_plugin(entry):
    spec = importlib.util.spec_from_file_location("winstrata_plugin", entry)
    if spec is None or spec.loader is None:
        raise SystemExit(3)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def serve(manifest, state, plugin):
    phase = "running"
    while True:
        request = read_frame()
        if request is None:
            return
        response, exit_code = handle(manifest, state, plugin, phase, request)
        if response.get("phase"):
            phase = response["phase"] if request.get("op") in ("pause", "resume") else phase
        write_frame(response)
        if exit_code is not None:
            raise SystemExit(exit_code)


def handle(manifest, state, plugin, phase, request):
    op = request.get("op")
    pid = os.getpid()
    if op == "ping":
        return reply(manifest, True, "ok", phase, phase, pid), None
    if op == "pause":
        return reply(manifest, True, "ok", "paused", "paused", pid), None
    if op == "resume":
        return reply(manifest, True, "ok", "running", "running", pid), None
    if op in ("stop", "disable"):
        return reply(manifest, True, "ok", phase, phase, pid), 0
    if op == "fault":
        return reply(manifest, True, "ok", "faulted", phase, pid), 7
    if op == "emit":
        event_type = request.get("event_type") or ""
        if event_type in manifest.get("events", []):
            return reply(manifest, True, "ok", event_type, phase, pid), None
        return reply(manifest, False, "rejected", "event is not declared", phase, pid), None
    if op == "invoke":
        capability = request.get("capability") or ""
        if capability not in manifest.get("capabilities", []):
            return (
                reply(manifest, False, "rejected", "capability is not provided", phase, pid),
                None,
            )
        payload = request.get("payload") or ""
        function = getattr(plugin, "invoke", None)
        if function is None:
            return reply(manifest, False, "rejected", "invoke is not defined", phase, pid), None
        try:
            detail = function(capability, payload)
        except Exception as error:
            return reply(manifest, False, "faulted", str(error), phase, pid), None
        if not isinstance(detail, str):
            return reply(manifest, False, "rejected", "invoke result is not text", phase, pid), None
        return reply(manifest, True, "ok", detail, phase, pid), None
    if op == "read_state":
        try:
            text = read_state(state, request.get("name") or "")
        except ValueError as error:
            code = str(error)
            return reply(manifest, False, code, code, phase, pid), None
        return reply(manifest, True, "ok", text, phase, pid), None
    return reply(manifest, False, "rejected", "operation is not supported", phase, pid), None


def read_state(state, name):
    if (
        not name
        or len(name.encode()) > 40
        or not name[0].islower()
        or any(not (character.islower() or character.isdigit() or character in "._-") for character in name)
        or ".." in name
    ):
        raise ValueError("rejected")
    path = os.path.join(state, name)
    if os.path.dirname(path) != state:
        raise ValueError("rejected")
    try:
        info = os.lstat(path)
    except OSError as error:
        raise ValueError("missing") from error
    if not os.path.isfile(path) or info.st_size > 1024 or os.path.islink(path):
        raise ValueError("rejected")
    with open(path, "r", encoding="utf-8") as handle:
        text = handle.read()
    if any(ord(character) < 32 or ord(character) == 127 for character in text):
        raise ValueError("rejected")
    return text


def reply(manifest, ok, code, detail, phase, pid):
    return {
        "version": {"major": 1, "minor": 0},
        "op": "result",
        "ok": ok,
        "code": code,
        "detail": bound(detail),
        "phase": phase,
        "pid": pid,
        "plugin_version": manifest["plugin_version"],
    }


def bound(value):
    text = "".join(character for character in value if ord(character) >= 32 and ord(character) != 127)
    return text[:1024] or " "


def read_frame():
    header = sys.stdin.buffer.read(4)
    if not header:
        return None
    if len(header) != 4:
        raise SystemExit(1)
    length = int.from_bytes(header, "little")
    if length <= 0 or length > 65536:
        raise SystemExit(1)
    body = sys.stdin.buffer.read(length)
    if len(body) != length:
        raise SystemExit(1)
    return json.loads(body.decode("utf-8"))


def write_frame(document):
    body = json.dumps(document, separators=(",", ":")).encode("utf-8")
    sys.stdout.buffer.write(len(body).to_bytes(4, "little"))
    sys.stdout.buffer.write(body)
    sys.stdout.buffer.flush()


if __name__ == "__main__":
    main()
