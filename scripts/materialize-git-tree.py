#!/usr/bin/env python3
"""Materialize a preflighted Git tree using one raw cat-file batch stream."""

from __future__ import annotations

import os
import re
import subprocess
import sys
import tempfile
from pathlib import Path


CHUNK_SIZE = 64 * 1024


def reject(message: str) -> None:
    raise ValueError(f"generated-workflows: cannot materialize snapshot: {message}")


def read_exact(stream, length: int, destination=None) -> bytes | None:
    remaining = length
    collected = bytearray() if destination is None else None
    while remaining:
        chunk = stream.read(min(CHUNK_SIZE, remaining))
        if not chunk:
            reject("Git batch object data is truncated")
        if destination is None:
            collected.extend(chunk)
        else:
            destination.write(chunk)
        remaining -= len(chunk)
    return bytes(collected) if collected is not None else None


def safe_path(raw: bytes) -> str:
    try:
        path = raw.decode("utf-8", "strict")
    except UnicodeError:
        reject("snapshot tree path is not UTF-8")
    parts = path.split("/")
    if (
        not path
        or path.startswith("/")
        or path.endswith("/")
        or "\\" in path
        or "//" in path
        or any(part in {"", ".", "..", ".git"} for part in parts)
        or any(ord(character) < 32 or ord(character) == 127 for character in path)
    ):
        reject("unsafe path")
    return path


def load_entries(entries_path: Path, statuses_path: Path, allowlist_path: Path, maximum_blob: int, maximum_total: int):
    raw = entries_path.read_bytes()
    if raw and not raw.endswith(b"\0"):
        reject("snapshot tree listing is incomplete")
    statuses = statuses_path.read_text(encoding="ascii").splitlines()
    allowlist: dict[tuple[str, str], int] = {}
    for number, line in enumerate(allowlist_path.read_text(encoding="utf-8").splitlines(), 1):
        if not line or line.startswith("#"):
            continue
        fields = line.split("\t")
        if len(fields) != 3 or not fields[2].isascii() or not fields[2].isdigit():
            reject(f"malformed large-blob allowlist row {number}")
        allowlist[(fields[0], fields[1])] = int(fields[2])

    entries = []
    total = 0
    for entry in raw.split(b"\0")[:-1] if raw else ():
        try:
            header, path_raw = entry.split(b"\t", 1)
            mode, kind, oid = header.decode("ascii").split(" ")
        except (UnicodeError, ValueError):
            reject("snapshot tree entry is malformed")
        path = safe_path(path_raw)
        if mode not in {"100644", "100755", "120000"} or kind != "blob" or re.fullmatch(r"[0-9a-f]{40,64}", oid) is None:
            reject("unsupported entry type")
        if len(entries) >= len(statuses):
            reject("snapshot blob inspection was incomplete")
        fields = statuses[len(entries)].split()
        if len(fields) != 3 or fields[:2] != [oid, "blob"] or not fields[2].isascii() or not fields[2].isdigit():
            reject("snapshot tree contains an unexpected blob object type")
        size = int(fields[2])
        path_limit = allowlist.get((path, oid), maximum_blob)
        if size > path_limit:
            reject("snapshot blob exceeds the per-object size limit")
        total += size
        if total > maximum_total:
            reject("snapshot exceeds the total materialized blob size limit")
        entries.append((mode, oid, path, size))
    if len(entries) != len(statuses):
        reject("snapshot blob inspection was incomplete")
    return entries


def ensure_parent(root: Path, relative: str) -> Path:
    target = root / relative
    current = root
    for component in relative.split("/")[:-1]:
        current = current / component
        if current.is_symlink():
            reject("path traverses a symlink")
        if current.exists():
            if not current.is_dir():
                reject("path traverses a non-directory")
        else:
            current.mkdir()
    if target.exists() or target.is_symlink():
        reject("duplicate path")
    return target


def main(arguments: list[str]) -> int:
    if len(arguments) != 10:
        raise SystemExit("generated-workflows: batch materializer received invalid arguments")
    git, reader_kind, reader, private_objects, verified_objects, source_objects, entries_file, statuses_file, allowlist_file, snapshot_dir = arguments
    maximum_blob = int(os.environ["TUISCO_MAX_BLOB"])
    maximum_total = int(os.environ["TUISCO_MAX_TOTAL"])
    entries = load_entries(Path(entries_file), Path(statuses_file), Path(allowlist_file), maximum_blob, maximum_total)
    snapshot = Path(snapshot_dir)

    if reader_kind == "trusted":
        home = os.environ["TUISCO_READER_HOME"]
        env = {
            "HOME": home,
            "PATH": os.environ["TUISCO_GIT_PATH"],
            "GIT_OBJECT_DIRECTORY": private_objects,
            "GIT_ALTERNATE_OBJECT_DIRECTORIES": f"{verified_objects}:{source_objects}",
            "GIT_CONFIG_NOSYSTEM": "1",
            "GIT_CONFIG_GLOBAL": "/dev/null",
            "GIT_CONFIG_SYSTEM": "/dev/null",
            "GIT_NO_REPLACE_OBJECTS": "1",
            "GIT_NO_LAZY_FETCH": "1",
            "GIT_OPTIONAL_LOCKS": "0",
        }
        command = [git, f"--git-dir={reader}", "-c", "core.hooksPath=/dev/null", "cat-file", "--batch"]
    elif reader_kind == "snapshot":
        env = {
            "HOME": os.environ["TUISCO_READER_HOME"],
            "PATH": os.environ["TUISCO_GIT_PATH"],
            "GIT_DIR": reader,
            "GIT_OBJECT_DIRECTORY": private_objects,
            "GIT_ALTERNATE_OBJECT_DIRECTORIES": f"{verified_objects}:{source_objects}",
            "GIT_CONFIG_NOSYSTEM": "1",
            "GIT_CONFIG_GLOBAL": "/dev/null",
            "GIT_CONFIG_SYSTEM": "/dev/null",
            "GIT_NO_REPLACE_OBJECTS": "1",
            "GIT_NO_LAZY_FETCH": "1",
            "GIT_OPTIONAL_LOCKS": "0",
        }
        command = [git, "-c", "core.hooksPath=/dev/null", "cat-file", "--batch"]
    else:
        reject("invalid Git reader")

    try:
        with tempfile.TemporaryFile(prefix="snapshot-cat-file-requests-") as requests:
            requests.write("".join(oid + "\n" for _, oid, _, _ in entries).encode("ascii"))
            requests.seek(0)
            process = subprocess.Popen(command, env=env, stdin=requests, stdout=subprocess.PIPE, stderr=subprocess.DEVNULL)
            assert process.stdout is not None
            try:
                for mode, oid, relative, size in entries:
                    header = process.stdout.readline(256)
                    if not header.endswith(b"\n"):
                        reject("Git batch object header is malformed")
                    match = re.fullmatch(rb"([0-9a-f]{40,64}) blob (0|[1-9][0-9]*)\n", header)
                    if match is None or match.group(1).decode("ascii") != oid or int(match.group(2)) != size:
                        reject("Git batch object header does not match preflight metadata")
                    target = ensure_parent(snapshot, relative)
                    if mode == "120000":
                        payload = read_exact(process.stdout, size)
                        assert payload is not None
                        if b"\0" in payload:
                            reject("symlink target contains a NUL byte")
                        if not payload:
                            reject("symlink target is empty")
                        os.symlink(payload, os.fsencode(target))
                    else:
                        with target.open("xb") as output:
                            read_exact(process.stdout, size, output)
                        target.chmod(0o755 if mode == "100755" else 0o644)
                    if process.stdout.read(1) != b"\n":
                        reject("Git batch object separator is malformed")
                if process.stdout.read(1):
                    reject("Git batch reader returned unexpected trailing data")
                if process.wait() != 0:
                    reject("Git batch reader failed")
            except BaseException:
                if process.poll() is None:
                    process.kill()
                process.wait()
                raise
    except OSError as error:
        reject(f"cannot start Git batch reader: {error}")
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main(sys.argv[1:]))
    except (OSError, UnicodeError, ValueError) as error:
        print(str(error), file=sys.stderr)
        raise SystemExit(1)
