#!/usr/bin/env python3
# SPDX-FileCopyrightText: 2026 Alexey Zhokhov
# SPDX-License-Identifier: Apache-2.0
import json
import os
import subprocess
import sys
from pathlib import Path

MAX_CAPTURE_BYTES = 8 * 1024 * 1024
REDACTION = b"[REDACTED_GITHUB_SETUP_TOKEN]"


def write_once(path, content):
    target = Path(path)
    flags = os.O_WRONLY | os.O_CREAT | os.O_EXCL
    descriptor = os.open(target, flags, 0o600)
    with os.fdopen(descriptor, "wb") as stream:
        stream.write(content)
        stream.flush()
        os.fsync(stream.fileno())


def sanitize(data, secret):
    output = data.replace(secret, REDACTION) if secret else data
    if secret:
        for size in range(1, len(secret)):
            suffix = secret[:size]
            if output.endswith(suffix):
                output = output[:-size] + REDACTION
    return output


def capture(status_path, log_path, cwd, command):
    secret = os.environ.get("GITHUB_COM_TOKEN", "").encode("utf-8")
    if not secret:
        raise SystemExit("managed-upgrade-capture: setup credential is absent")
    if not command:
        raise SystemExit("managed-upgrade-capture: child command is absent")
    if secret in b"\0".join(os.fsencode(argument) for argument in command):
        raise SystemExit("managed-upgrade-capture: setup credential appeared in child argv")
    process = subprocess.Popen(command, cwd=cwd, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, env=dict(os.environ))
    retained = bytearray()
    total = 0
    while True:
        chunk = process.stdout.read(65536)
        if not chunk:
            break
        total += len(chunk)
        if len(retained) < MAX_CAPTURE_BYTES:
            retained.extend(chunk[: MAX_CAPTURE_BYTES - len(retained)])
    child_status = process.wait()
    raw = bytes(retained)
    partial_secret = any(raw.endswith(secret[:size]) for size in range(1, len(secret)))
    secret_output = bool(secret in raw) or partial_secret
    safe = sanitize(raw, secret)
    truncated = total > len(raw)
    if truncated:
        safe += b"\n[child output truncated after the bounded capture limit]\n"
    status = {
        "child_exit_status": child_status,
        "output_bytes_total": total,
        "output_bytes_retained": len(raw),
        "output_truncated": truncated,
        "secret_output_detected": secret_output,
        "capture_rejected": secret_output or truncated,
        "log_redacted_before_first_write": True,
        "child_argv_contains_secret": False,
    }
    write_once(log_path, safe)
    write_once(status_path, (json.dumps(status, sort_keys=True) + "\n").encode("utf-8"))
    return 125 if secret_output or truncated else 0


def canary_control(directory):
    root = Path(directory)
    root.mkdir(mode=0o700, parents=True, exist_ok=False)
    canary = "canary-only-foreign-owner-proof-credential"
    status = root / "status.json"
    log = root / "output.log"
    env = dict(os.environ)
    env["GITHUB_COM_TOKEN"] = canary
    command = [sys.executable, "-c", "import os,sys; sys.stdout.write(os.environ['GITHUB_COM_TOKEN']); sys.exit(37)"]
    if canary.encode() in b"\0".join(os.fsencode(argument) for argument in command):
        raise SystemExit("managed-upgrade-capture: canary unexpectedly entered test argv")
    result = subprocess.run([sys.executable, __file__, "capture", str(status), str(log), str(root), "--", *command], env=env, check=False)
    if result.returncode != 125:
        raise SystemExit("managed-upgrade-capture: canary child did not fail closed")
    record = json.loads(status.read_text(encoding="utf-8"))
    if record != {
        "child_argv_contains_secret": False,
        "capture_rejected": True,
        "child_exit_status": 37,
        "log_redacted_before_first_write": True,
        "output_bytes_retained": len(canary.encode()),
        "output_bytes_total": len(canary.encode()),
        "output_truncated": False,
        "secret_output_detected": True,
    }:
        raise SystemExit("managed-upgrade-capture: canary status record differs from the expected bounded diagnostic")
    log_bytes = log.read_bytes()
    if canary.encode() in log_bytes or canary.encode() in status.read_bytes() or REDACTION not in log_bytes:
        raise SystemExit("managed-upgrade-capture: canary was persisted")
    print("case=secret-output-redacted-before-first-write\nchild_exit_status=37\nwrapper_status=125\ncanary_persisted=false")


if len(sys.argv) >= 2 and sys.argv[1] == "capture":
    if len(sys.argv) < 6 or sys.argv[5] != "--":
        raise SystemExit("usage: capture STATUS LOG CWD -- COMMAND...")
    raise SystemExit(capture(sys.argv[2], sys.argv[3], sys.argv[4], sys.argv[6:]))
if len(sys.argv) == 3 and sys.argv[1] == "canary-control":
    canary_control(sys.argv[2])
else:
    raise SystemExit("usage: capture STATUS LOG CWD -- COMMAND... | canary-control DIRECTORY")
