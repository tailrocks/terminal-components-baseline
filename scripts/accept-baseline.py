#!/usr/bin/env python3
"""Accept one functional baseline outside the candidate gate.

This is the only baseline writer. Functional tests do not import or run it.
Ordinary nextest runs do not call it. A candidate gate sets
FUNCTIONAL_CANDIDATE_GATE=1 and this entry point refuses.

The approval command is `tuiscotti accept` with --candidate,
--candidate-sha256, --expect-baseline, and --baselines. The receipt is
written only at --receipt, and only after that command exits 0.
"""

from __future__ import annotations

import argparse
import copy
import hashlib
import json
import os
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
CANDIDATE_GATE = "FUNCTIONAL_CANDIDATE_GATE"


def fail(message: str, code: int = 2) -> None:
    print(message, file=sys.stderr)
    raise SystemExit(code)


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def inside(child: Path, parent: Path) -> bool:
    return child.resolve().is_relative_to(parent.resolve())


def parse_env(items: list[str]) -> dict[str, str]:
    env: dict[str, str] = {}
    for item in items:
        if "=" not in item:
            fail(f"env entry {item!r} is not KEY=VALUE")
        key, value = item.split("=", 1)
        if not key:
            fail("env entry is missing a name")
        if key in env:
            fail(f"env entry {key} is repeated")
        env[key] = value
    return env


def load_manifest(path: Path) -> dict:
    try:
        manifest = json.loads(path.read_text())
    except (OSError, json.JSONDecodeError) as error:
        fail(f"reference manifest {path}: {error}")
    if not isinstance(manifest, dict):
        fail("reference manifest is not an object")
    acceptance = manifest.get("acceptance")
    if not isinstance(acceptance, dict):
        fail("reference manifest acceptance is missing")
    if acceptance.get("candidate_gate_env") != CANDIDATE_GATE:
        fail("manifest candidate gate env is not FUNCTIONAL_CANDIDATE_GATE")
    if acceptance.get("script") != "scripts/accept-baseline.py":
        fail("manifest acceptance script is not scripts/accept-baseline.py")
    return manifest


def receipt_body(manifest: dict, args: argparse.Namespace, binary: Path) -> dict:
    toolchain = manifest.get("toolchain")
    comparator = manifest.get("comparator")
    if not isinstance(toolchain, dict):
        fail("manifest toolchain is not an object")
    if not isinstance(comparator, dict) or not isinstance(comparator.get("rev"), str):
        fail("manifest comparator rev is missing")
    source = manifest.get("reference_sha")
    lockfile = manifest.get("lockfile_sha256")
    if not isinstance(source, str) or not isinstance(lockfile, str):
        fail("manifest source or lockfile sha is missing")
    return {
        "source_sha": source,
        "lockfile_sha256": lockfile,
        "toolchain": copy.deepcopy(toolchain),
        "binary_path": str(binary),
        "binary_sha256": args.binary_sha256,
        "fixture_args": list(args.fixture_arg),
        "env": parse_env(args.env),
        "comparator_rev": comparator["rev"],
        "candidate_sha256": args.candidate_sha256,
        "expect_baseline": args.expect_baseline,
        "precondition": args.precondition,
    }


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Accept one functional baseline.")
    parser.add_argument("--candidate", required=True, type=Path)
    parser.add_argument("--candidate-sha256", required=True)
    parser.add_argument("--expect-baseline", required=True)
    parser.add_argument("--baselines", required=True, type=Path)
    parser.add_argument("--receipt", required=True, type=Path)
    parser.add_argument("--precondition", required=True)
    parser.add_argument("--binary", required=True, type=Path)
    parser.add_argument("--binary-sha256", required=True)
    parser.add_argument("--fixture-arg", action="append", default=[])
    parser.add_argument("--env", action="append", default=[])
    parser.add_argument(
        "--manifest",
        type=Path,
        default=ROOT / "docs" / "testing" / "reference-manifest.json",
    )
    parser.add_argument("--tuiscotti", default="tuiscotti")
    return parser.parse_args()


def main() -> None:
    if os.environ.get(CANDIDATE_GATE) == "1":
        fail("candidate gate refuses baseline acceptance")
    args = parse_args()
    baselines = args.baselines
    receipt = args.receipt
    if inside(receipt, baselines):
        fail("receipt is inside the baseline root")
    binary = args.binary.resolve()
    if not binary.is_file():
        fail(f"binary does not exist: {binary}")
    actual_binary = sha256_file(binary)
    if actual_binary != args.binary_sha256:
        fail("binary sha256 does not match the file")
    if not args.candidate.is_file():
        fail(f"candidate does not exist: {args.candidate}")
    manifest = load_manifest(args.manifest)
    body = receipt_body(manifest, args, binary)
    # Invokes `tuiscotti accept` with the four approval flags. Tests never do this.
    command = [
        args.tuiscotti,
        "accept",
        "--candidate",
        str(args.candidate),
        "--candidate-sha256",
        args.candidate_sha256,
        "--expect-baseline",
        args.expect_baseline,
        "--baselines",
        str(baselines),
    ]
    completed = subprocess.run(command, check=False)
    if completed.returncode != 0:
        raise SystemExit(completed.returncode)
    receipt.parent.mkdir(parents=True, exist_ok=True)
    if inside(receipt, baselines):
        fail("receipt is inside the baseline root")
    text = json.dumps(body, indent=2) + "\n"
    temporary = receipt.with_name(receipt.name + ".tmp")
    temporary.write_text(text)
    temporary.replace(receipt)


if __name__ == "__main__":
    main()
