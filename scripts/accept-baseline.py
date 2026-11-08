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
import tempfile
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


def atomic_write_text(destination: Path, text: str) -> None:
    descriptor, temporary_name = tempfile.mkstemp(
        prefix=f".{destination.name}.", suffix=".tmp", dir=destination.parent
    )
    temporary = Path(temporary_name)
    try:
        with os.fdopen(descriptor, "w", encoding="utf-8") as handle:
            handle.write(text)
            handle.flush()
            os.fsync(handle.fileno())
        os.replace(temporary, destination)
    finally:
        temporary.unlink(missing_ok=True)


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
    body = {
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
        "session": str(args.session.resolve()) if args.session else None,
        "expected_test_id": args.expected_test_id,
        "acceptance_receipt_path": str(args.receipt),
        "provenance_path": str(args.provenance) if args.provenance else None,
        "previous_archive_path": str(args.previous_archive) if args.previous_archive else None,
    }
    if args.expected_test_id:
        body["test_id"] = args.expected_test_id
        body["acceptance"] = {
            "expected_baseline_sha256": args.expect_baseline,
            "accepted_baseline_sha256": args.candidate_sha256,
        }
        body["previous_baseline"] = {"sha256": args.expect_baseline}
        body["candidate"] = {"sha256": args.candidate_sha256}
    return body


def verify_reference_session(
    manifest: dict,
    session_path: Path,
    binary: Path,
    binary_sha256: str,
    fixture_args: list[str],
    expected_profile: dict,
) -> dict:
    try:
        session = json.loads(session_path.read_text())
    except (OSError, json.JSONDecodeError) as error:
        fail(f"reference session {session_path}: {error}")
    if session.get("mode") != "reference":
        fail("acceptance session is not Reference mode")
    if session.get("binary_sha256") != binary_sha256:
        fail("session binary sha256 does not match the accepted binary")
    try:
        session_binary = Path(session["binary_path"]).resolve()
    except (KeyError, TypeError):
        fail("reference session binary path is missing")
    if session_binary != binary:
        fail("session binary path does not match the accepted binary")
    if session.get("reference_sha") != manifest.get("reference_sha"):
        fail("session source revision does not match the selected reference manifest")
    if session.get("lockfile_sha256") != manifest.get("lockfile_sha256"):
        fail("session lockfile digest does not match the selected reference manifest")
    if session.get("comparator_rev") != manifest.get("comparator", {}).get("rev"):
        fail("session comparator does not match the selected reference manifest")
    found = False
    expected_name = binary.name
    for name, entry in manifest.get("binaries", {}).items():
        identity_name = entry.get("identity_name", name)
        if identity_name != expected_name:
            continue
        platforms = entry.get("platforms", {})
        for platform in platforms.values():
            if platform.get("sha256") != binary_sha256:
                continue
            try:
                listed_path = (ROOT / platform["path"]).resolve()
            except (KeyError, TypeError):
                continue
            if listed_path == binary:
                found = True
                break
        if found:
            break
    if not found:
        fail("accepted binary path and digest are not listed in the selected reference manifest")
    if session.get("args") != fixture_args:
        fail("reference session arguments do not exactly match the acceptance arguments")
    for key, expected in expected_profile.items():
        if session.get(key) != expected:
            fail(f"reference session {key} does not match the expected capture profile")
    return session


def verify_capture_index(
    index_path: Path,
    session_path: Path,
    binary: Path,
    binary_sha256: str,
    candidate: Path,
    candidate_sha256: str,
    expected_test_id: str,
) -> dict:
    try:
        entries = [
            json.loads(line)
            for line in index_path.read_text().splitlines()
            if line.strip()
        ]
    except (OSError, json.JSONDecodeError) as error:
        fail(f"reference capture index {index_path}: {error}")
    matching = [
        entry
        for entry in entries
        if entry.get("candidate_path")
        and Path(entry["candidate_path"]).resolve() == candidate
        and entry.get("candidate_sha256") == candidate_sha256
    ]
    if len(matching) != 1:
        fail("candidate path and digest must identify exactly one reference capture record")
    entry = matching[0]
    if entry.get("test_id") != expected_test_id:
        fail("capture index test ID does not match the expected test ID")
    if Path(entry.get("session_path", "")).resolve() != session_path:
        fail("capture index does not bind this candidate to the supplied reference session")
    if Path(entry.get("binary_path", "")).resolve() != binary:
        fail("capture index binary path does not match the supplied binary")
    if entry.get("binary_sha256") != binary_sha256:
        fail("capture index binary digest does not match the supplied binary")
    if not index_path.resolve().is_relative_to(session_path.parent):
        fail("reference capture index is outside its evidence directory")
    if not candidate.is_relative_to(index_path.parent.resolve()):
        fail("candidate is outside the indexed reference capture directory")
    return entry


def inspect_sample(
    args: argparse.Namespace, baselines: Path, expected_test_id: str
) -> dict:
    with tempfile.TemporaryDirectory(prefix="tuiscotti-accept-check-") as artifacts:
        command = [
            args.tuiscotti,
            "--json",
            "check",
            "--sample",
            str(args.candidate),
            "--baselines",
            str(baselines),
            "--artifacts",
            artifacts,
        ]
        completed = subprocess.run(command, check=False, capture_output=True, text=True)
    try:
        result = json.loads(completed.stdout)
    except json.JSONDecodeError as error:
        fail(f"tuiscotti check returned invalid JSON: {error}")
    if result.get("test_id") != expected_test_id:
        fail("candidate sample test ID does not match the expected test ID")
    if result.get("candidate_sha256") != args.candidate_sha256:
        fail("tuiscotti check candidate digest does not match the accepted candidate")
    if result.get("status") not in {"pass", "different", "missing"}:
        fail(f"tuiscotti check failed: {result}")
    return result


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Accept one functional baseline.")
    parser.add_argument("--candidate", required=True, type=Path)
    parser.add_argument("--candidate-sha256", required=True)
    parser.add_argument("--expect-baseline", required=True)
    parser.add_argument("--baselines", required=True, type=Path)
    parser.add_argument("--receipt", required=True, type=Path)
    parser.add_argument("--provenance", type=Path)
    parser.add_argument("--previous-archive", type=Path)
    parser.add_argument("--precondition", required=True)
    parser.add_argument("--binary", required=True, type=Path)
    parser.add_argument("--binary-sha256", required=True)
    parser.add_argument("--session", type=Path)
    parser.add_argument("--capture-index", type=Path)
    parser.add_argument("--expected-test-id")
    parser.add_argument("--expected-cols", type=int)
    parser.add_argument("--expected-rows", type=int)
    parser.add_argument("--expected-color")
    parser.add_argument("--expected-platform")
    parser.add_argument("--expected-motion")
    parser.add_argument("--expected-term")
    parser.add_argument("--expected-clock", choices=["true", "false"])
    parser.add_argument("--expected-history", choices=["true", "false"])
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
    if args.provenance and inside(args.provenance, baselines):
        fail("provenance is inside the baseline root")
    binary = args.binary.resolve()
    if not binary.is_file():
        fail(f"binary does not exist: {binary}")
    actual_binary = sha256_file(binary)
    if actual_binary != args.binary_sha256:
        fail("binary sha256 does not match the file")
    if not args.candidate.is_file():
        fail(f"candidate does not exist: {args.candidate}")
    if sha256_file(args.candidate) != args.candidate_sha256:
        fail("candidate sha256 does not match the file")
    manifest = load_manifest(args.manifest)
    is_default_manifest = args.manifest.resolve() == (
        ROOT / "docs" / "testing" / "reference-manifest.json"
    ).resolve()
    expected_profile_values = [
        args.expected_cols,
        args.expected_rows,
        args.expected_color,
        args.expected_platform,
        args.expected_motion,
        args.expected_term,
        args.expected_clock,
        args.expected_history,
    ]
    if not is_default_manifest and (
        args.session is None
        or args.capture_index is None
        or args.expected_test_id is None
        or args.provenance is None
        or any(value is None for value in expected_profile_values)
    ):
        fail(
            "alternate reference manifests require a session, capture index, expected test ID, "
            "and complete capture profile"
        )
    if args.session is not None:
        if args.capture_index is None or args.expected_test_id is None:
            fail("Reference-mode acceptance requires --capture-index and --expected-test-id")
        reference_session = verify_reference_session(
            manifest,
            args.session,
            binary,
            actual_binary,
            args.fixture_arg,
            {
                key: value
                for key, value in {
                    "cols": args.expected_cols,
                    "rows": args.expected_rows,
                    "color": args.expected_color,
                    "platform": args.expected_platform,
                    "motion": args.expected_motion,
                    "TERM": args.expected_term,
                    "clock": args.expected_clock == "true"
                    if args.expected_clock is not None
                    else None,
                    "history": args.expected_history == "true"
                    if args.expected_history is not None
                    else None,
                }.items()
                if value is not None
            },
        )
        capture_entry = verify_capture_index(
            args.capture_index.resolve(),
            args.session.resolve(),
            binary,
            actual_binary,
            args.candidate.resolve(),
            args.candidate_sha256,
            args.expected_test_id,
        )
    else:
        if args.capture_index is not None:
            fail("--capture-index requires --session")
        reference_session = None
        capture_entry = None
    if args.expected_test_id:
        sample_check = inspect_sample(args, baselines, args.expected_test_id)
    else:
        sample_check = None
    expected_path = baselines / f"{args.expected_test_id}.tui.pb" if args.expected_test_id else None
    if args.expect_baseline == "missing":
        if expected_path is not None and expected_path.exists():
            fail("expected baseline is present but --expect-baseline is missing")
    elif expected_path is not None:
        if not expected_path.is_file() or sha256_file(expected_path) != args.expect_baseline:
            fail("current baseline does not match --expect-baseline")
    body = receipt_body(manifest, args, binary)
    if reference_session is not None:
        body["reference_session"] = {
            key: reference_session[key]
            for key in (
                "mode",
                "reference_sha",
                "lockfile_sha256",
                "binary_path",
                "binary_sha256",
                "args",
                "cols",
                "rows",
                "color",
                "platform",
                "history",
                "motion",
                "clock",
                "TERM",
                "comparator_rev",
            )
        }
        body["capture_record"] = capture_entry
        body["pre_accept_check"] = sample_check
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
    command.insert(1, "--json")
    completed = subprocess.run(command, check=False, capture_output=True, text=True)
    if completed.returncode != 0:
        sys.stderr.write(completed.stderr)
        raise SystemExit(completed.returncode)
    try:
        cli_receipt = json.loads(completed.stdout)
    except json.JSONDecodeError as error:
        fail(f"tuiscotti accept returned invalid JSON: {error}")
    if args.expected_test_id and cli_receipt.get("test_id") != args.expected_test_id:
        fail("accepted sample id does not match the requested test id")
    if reference_session is not None:
        body["cli_receipt"] = cli_receipt
        body["capture_index_path"] = str(args.capture_index.resolve())
    receipt.parent.mkdir(parents=True, exist_ok=True)
    if args.provenance:
        args.provenance.parent.mkdir(parents=True, exist_ok=True)
    if inside(receipt, baselines):
        fail("receipt is inside the baseline root")
    if args.provenance and inside(args.provenance, baselines):
        fail("provenance is inside the baseline root")
    text = json.dumps(cli_receipt if reference_session is not None else body, indent=2) + "\n"
    atomic_write_text(receipt, text)
    if args.provenance:
        provenance = args.provenance
        provenance_text = json.dumps(body, indent=2) + "\n"
        atomic_write_text(provenance, provenance_text)


if __name__ == "__main__":
    main()
