#!/usr/bin/env python3
# SPDX-FileCopyrightText: 2026 Alexey Zhokhov
# SPDX-License-Identifier: Apache-2.0
import base64
import hashlib
import json
import os
import shutil
import subprocess
import sys
import tempfile
import tomllib
from pathlib import Path

SEED_COMMIT = "f4d9999ace7d6deb4294cd89c00afcc80e748d49"
SEED_TREE = "95406daef6ba234fde62fc47b933fbb8bdfce320"
MANIFEST_PATHS = ("Cargo.toml", "apps/jackin/Cargo.toml")
EXPECTED_AGGREGATE = "4e7f83b2eaf98c6b3413d10cc4c0dacc4c123d84ecdfd401e5da90daf7ad7f19"
# The reviewed artifact is only ever written under this exact name in the
# current working directory. The command-line output argument must match it
# verbatim, so no caller-controlled path component can ever reach the file
# creation below (no absolute paths, no traversal, no directory separators).
OUTPUT_NAME = "trusted-seed-cargo-policy.json"


def git(repository, *arguments):
    executable = shutil.which("git")
    if executable is None:
        raise SystemExit("trusted seed Cargo policy generation requires Git")
    # A private per-invocation home avoids the shared, publicly writable
    # directory: git must not read or write any state outside our control.
    with tempfile.TemporaryDirectory(prefix="trusted-seed-policy-home.") as private_home:
        environment = {
            "PATH": os.environ.get("PATH", "/usr/bin:/bin"),
            "HOME": private_home,
            "GIT_CONFIG_NOSYSTEM": "1",
            "GIT_CONFIG_GLOBAL": "/dev/null",
            "GIT_CONFIG_SYSTEM": "/dev/null",
            "GIT_NO_REPLACE_OBJECTS": "1",
            "GIT_NO_LAZY_FETCH": "1",
            "GIT_OPTIONAL_LOCKS": "0",
            "LC_ALL": "C",
            "LANG": "C",
        }
        try:
            return subprocess.run(
                [executable, "-C", str(repository), *arguments],
                check=True,
                env=environment,
                stdout=subprocess.PIPE,
                stderr=subprocess.PIPE,
            ).stdout
        except subprocess.CalledProcessError as error:
            detail = error.stderr.decode("utf-8", "backslashreplace").strip()
            raise SystemExit(f"trusted seed Cargo policy generation failed: {detail}") from error


def main():
    if len(sys.argv) != 3:
        raise SystemExit("usage: generate-trusted-seed-cargo-policy.py SEED_REPOSITORY OUTPUT")
    repository = Path(sys.argv[1]).resolve(strict=True)
    output = Path(sys.argv[2])
    if len(output.parts) != 1 or output.name != OUTPUT_NAME or output.is_absolute():
        raise SystemExit(
            f"trusted seed Cargo policy output must be exactly {OUTPUT_NAME!r} in the current directory"
        )
    if git(repository, "rev-parse", "--verify", f"{SEED_COMMIT}^{{commit}}").decode().strip() != SEED_COMMIT:
        raise SystemExit("trusted seed Cargo policy source commit differs from the reviewed commit")
    if git(repository, "rev-parse", "--verify", f"{SEED_COMMIT}^{{tree}}").decode().strip() != SEED_TREE:
        raise SystemExit("trusted seed Cargo policy source tree differs from the reviewed tree")
    message = git(repository, "show", "-s", "--format=%B", SEED_COMMIT).decode("utf-8")
    if "Signed-off-by: Alexey Zhokhov <alexey@zhokhov.com>" not in message.splitlines():
        raise SystemExit("trusted seed Cargo policy source commit lacks its reviewed sign-off")
    listing = git(repository, "ls-tree", "-r", "-z", SEED_COMMIT)
    manifests = {}
    for entry in listing.split(b"\0"):
        if not entry:
            continue
        header, encoded_path = entry.split(b"\t", 1)
        mode, kind, object_id = header.split()
        path = encoded_path.decode("utf-8")
        if path.rsplit("/", 1)[-1] == "Cargo.toml":
            if mode != b"100644" or kind != b"blob" or path in manifests:
                raise SystemExit("trusted seed Cargo policy source contains an unsafe manifest entry")
            manifests[path] = object_id.decode("ascii")
    if tuple(sorted(manifests)) != MANIFEST_PATHS:
        raise SystemExit("trusted seed Cargo policy source manifest path set differs from the reviewed set")
    entries = []
    aggregate = hashlib.sha256()
    for path in MANIFEST_PATHS:
        data = git(repository, "show", f"{SEED_COMMIT}:{path}")
        try:
            tomllib.loads(data.decode("utf-8"))
        except (UnicodeError, tomllib.TOMLDecodeError) as error:
            raise SystemExit(f"trusted seed Cargo policy source manifest is invalid TOML: {path}") from error
        digest = hashlib.sha256(data).hexdigest()
        aggregate.update(path.encode("utf-8"))
        aggregate.update(b"\0")
        aggregate.update(digest.encode("ascii"))
        aggregate.update(b"\n")
        entries.append({"path": path, "sha256": digest, "bytes_base64": base64.b64encode(data).decode("ascii")})
    if aggregate.hexdigest() != EXPECTED_AGGREGATE:
        raise SystemExit("trusted seed Cargo policy source manifest aggregate differs from the reviewed identity")
    policy = {
        "schema": 1,
        "seed_commit": SEED_COMMIT,
        "seed_tree": SEED_TREE,
        "manifest_aggregate_sha256": aggregate.hexdigest(),
        "manifests": entries,
    }
    encoded = (json.dumps(policy, ensure_ascii=True, separators=(",", ":"), sort_keys=True) + "\n").encode("ascii")
    if len(encoded) > 262144:
        raise SystemExit("trusted seed Cargo policy artifact exceeds its fixed size bound")
    if os.path.islink(OUTPUT_NAME) or (os.path.exists(OUTPUT_NAME) and not os.path.isfile(OUTPUT_NAME)):
        raise SystemExit("trusted seed Cargo policy output path is not a regular file")
    # Create the reviewed artifact under its fixed, reviewed name with an
    # explicit, umask-independent mode so the pinned bytes never land in a
    # group- or world-writable file. O_NOFOLLOW refuses a symlink that is
    # swapped in between the check above and the open below.
    descriptor = os.open(OUTPUT_NAME, os.O_WRONLY | os.O_CREAT | os.O_TRUNC | os.O_NOFOLLOW, 0o600)
    with os.fdopen(descriptor, "wb") as handle:
        handle.write(encoded)


if __name__ == "__main__":
    main()
