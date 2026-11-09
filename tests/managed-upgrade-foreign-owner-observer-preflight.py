#!/usr/bin/env python3
# SPDX-FileCopyrightText: 2026 Alexey Zhokhov
# SPDX-License-Identifier: Apache-2.0
import hashlib
import json
import re
import struct
import sys
from pathlib import Path


directory = Path(sys.argv[1])
unexecuted = str(directory / "observer-unexecuted-sentinel")


def sha(path):
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for block in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(block)
    return digest.hexdigest()


def text(name):
    return (directory / name).read_text(encoding="utf-8", errors="strict")


def elf_machine(path):
    header = Path(path).read_bytes()[:20]
    if len(header) != 20 or header[:4] != b"\x7fELF" or header[4] != 2 or header[5] not in (1, 2):
        raise RuntimeError(f"not an ELF64 file: {path}")
    return struct.unpack("<H" if header[5] == 1 else ">H", header[18:20])[0]


def assert_capture(name, expected_child=0):
    status_path = directory / f"{name}.capture-status.json"
    status = json.loads(status_path.read_text(encoding="utf-8"))
    expected = {
        "child_argv_contains_secret": False,
        "capture_rejected": False,
        "child_exit_status": expected_child,
        "log_redacted_before_first_write": True,
        "output_truncated": False,
        "secret_output_detected": False,
    }
    if any(status.get(key) != value for key, value in expected.items()) or status.get("output_bytes_total") != status.get("output_bytes_retained"):
        raise RuntimeError(f"{name} bounded capture status differs from its expected child state")
    wrapper = text(f"{name}.wrapper-status.txt")
    if wrapper != "wrapper_status=0\n":
        raise RuntimeError(f"{name} capture wrapper did not return zero")
    log = directory / f"{name}.capture.log"
    if log.is_symlink() or not log.is_file() or log.stat().st_size > 8 * 1024 * 1024:
        raise RuntimeError(f"{name} bounded capture log is missing or oversized")
    return {
        "child_exit_status": status["child_exit_status"],
        "capture_wrapper_status": 0,
        "output_bytes_total": status["output_bytes_total"],
        "output_bytes_retained": status["output_bytes_retained"],
        "output_truncated": status["output_truncated"],
        "capture_rejected": status["capture_rejected"],
        "log_sha256": sha(log),
        "log_bytes": log.stat().st_size,
    }


def successful_exec(trace, path):
    escaped = json.dumps(path, ensure_ascii=False)
    return any(re.search(r"execve\(" + re.escape(escaped) + r",", line) is not None and line.rstrip().endswith(" = 0") for line in trace.splitlines())


def main():
    results = {}
    try:
        tools_status = text("observer-tools-build-status.txt")
        if "status=available\n" not in tools_status or "source=7.3\n" not in tools_status or "source_signature=verified\n" not in tools_status or "strace_machine=183\n" not in tools_status:
            raise RuntimeError("official-source native observer build or signature status is unavailable")
        signature_status = text("observer-tools-signature-status.txt")
        signature_match = re.search(r"^primary_fingerprint=([A-F0-9]{40,64})$", signature_status, re.MULTILINE)
        if "status=0\n" not in signature_status or "valid_signature_record=present\n" not in signature_status or signature_match is None:
            raise RuntimeError("frozen official source detached signature was not verified")
        signature_output = text("observer-tools-signature.stdout")
        subkeys_match = re.search(r"^subkey_fingerprints=([A-F0-9,]+)$", signature_status, re.MULTILINE)
        signing_subkeys_match = re.search(r"^signing_subkey_fingerprints=([A-F0-9,]*)$", signature_status, re.MULTILINE)
        signing_match = re.search(r"^signing_fingerprint=([A-F0-9]{40,64})$", signature_status, re.MULTILINE)
        primary_match = re.search(r"^signature_primary_fingerprint=([A-F0-9]{40,64})$", signature_status, re.MULTILINE)
        valid_signatures = [line.split() for line in signature_output.splitlines() if line.startswith("[GNUPG:] VALIDSIG ")]
        expected_primary = "7BECFE3AF7B280BB52FF77F104BA4521C996DDE1"
        expected_subkeys = {"FB3DCD19FF874735E4F61CE64E9FE8522F160EC2", "9D9065873A2C5345A447602A12A6C4AD7D939856"}
        expected_signing_subkeys = {"FB3DCD19FF874735E4F61CE64E9FE8522F160EC2"}
        observed_subkeys = set(subkeys_match.group(1).split(",")) if subkeys_match is not None else set()
        observed_signing_subkeys = set(signing_subkeys_match.group(1).split(",")) if signing_subkeys_match is not None and signing_subkeys_match.group(1) else set()
        allowed_fingerprints = {expected_primary, *expected_signing_subkeys}
        if len(valid_signatures) != 1 or signing_match is None or primary_match is None or signature_match.group(1) != expected_primary or observed_subkeys != expected_subkeys or observed_signing_subkeys != expected_signing_subkeys or primary_match.group(1) != expected_primary or signing_match.group(1) not in allowed_fingerprints:
            raise RuntimeError("GPG status output does not bind one verified signature to the pinned primary key and certified subkeys")
        fields = valid_signatures[0]
        status_primary = fields[-1] if len(fields) >= 12 else fields[2]
        if fields[2] != signing_match.group(1) or status_primary != signature_match.group(1):
            raise RuntimeError("GPG VALIDSIG signer and primary-key fingerprints differ from the retained verification status")
        source = (directory / "observer-tools-source.tsv").read_text(encoding="utf-8")
        source_expected = {
            "tarball_sha256": "529634c40910895a9a1f3c5c1e65da2ca9ed72fb158dfc4bcc89efdf05e39b72",
            "signature_sha256": "c7aea1a61162024da80e92852dc78e88edc03d2b61bb72579ece6da7d77f2873",
            "key_sha256": "8d038c52364a3937067d847c2c0397a53f5c3b5fc12b725f0d1be6d6eb52f2ad",
            "origin_metadata_sha256": "8e673c2c43cb4d78d6e8ac8d736b8dd2beb09ec33ea41130007118bd7b1293be",
        }
        for key, value in source_expected.items():
            if f"{key}\t{value}\n" not in source:
                raise RuntimeError(f"official source evidence does not bind the expected {key}")
        closure_rows = text("observer-tools-closure.tsv").splitlines()
        if not closure_rows or closure_rows[0] != "original_path\tresolved_path\tprivate_path\tsha256\telf_machine" or len(closure_rows) < 3:
            raise RuntimeError("private AArch64 observer dependency closure is incomplete")
        for row in closure_rows[1:]:
            original, resolved, private, digest, machine = row.split("\t")
            private_path = Path(private)
            exported_path = directory / "observer-tools" / Path(private).relative_to("/opt/exec-observer")
            if not private_path.is_file() or private_path.is_symlink() or sha(private_path) != digest or not exported_path.is_file() or exported_path.is_symlink() or sha(exported_path) != digest or elf_machine(private_path) != 183 or elf_machine(exported_path) != 183 or machine != "183":
                raise RuntimeError(f"private observer dependency differs from its AArch64 closure record: {original}")
        interpreter = re.search(r"^elf_interpreter=(/\S+)$", tools_status, re.MULTILINE)
        if interpreter is None or not any(row.split("\t", 1)[0] == interpreter.group(1) for row in closure_rows[1:]):
            raise RuntimeError("private loader is absent from the AArch64 observer closure")
        tracer = Path("/opt/exec-observer/bin/strace")
        native_true = Path("/opt/exec-observer/bin/observer-native-true")
        if elf_machine(tracer) != 183 or elf_machine(native_true) != 183 or "strace -- version 7.3" not in text("observer-tools-strace-version.txt"):
            raise RuntimeError("observer binary or native true control is not the expected AArch64 build")
        amd64_files = [directory / "observer-child-elf-sentinel", directory / "observer-grandchild-elf-sentinel", directory / "observer-execveat-target", Path("/usr/bin/bash"), Path("/usr/bin/python3"), Path("/usr/bin/true"), Path("/usr/bin/printf")]
        if any(elf_machine(path) != 62 for path in amd64_files):
            raise RuntimeError("the unchanged Debian AMD64 fixture or one of its runtime controls is not x86-64")
        for executable in ("bash", "env", "find", "printf", "python3", "true"):
            source_path = Path("/usr/bin") / executable
            exported_path = directory / "observer-tools/amd64" / executable
            if not exported_path.is_file() or exported_path.is_symlink() or sha(exported_path) != sha(source_path) or elf_machine(exported_path) != 62:
                raise RuntimeError(f"exported AMD64 runtime executable differs from the in-stage runtime: {executable}")
        trace_names = [
            "observer-native-true.trace",
            "observer-translated-true.trace",
            "observer-descendant.trace",
            "observer-argv-only-decoy.trace",
            "observer-failed-exec.trace",
            "observer-execveat.trace",
        ]
        traces = {}
        for name in trace_names:
            path = directory / name
            if path.is_symlink() or not path.is_file() or not 0 < path.stat().st_size <= 8 * 1024 * 1024:
                raise RuntimeError(f"bounded raw trace is absent, empty, or oversized: {name}")
            trace = path.read_text(encoding="utf-8", errors="strict")
            if unexecuted in trace:
                raise RuntimeError("unexecuted sentinel path appeared in raw exec telemetry")
            traces[name] = trace
        if not successful_exec(traces["observer-native-true.trace"], str(native_true)):
            raise RuntimeError("native ARM64 true control has no exact successful execve record")
        if not successful_exec(traces["observer-translated-true.trace"], "/usr/bin/true"):
            raise RuntimeError("translated AMD64 true control has no exact successful execve record")
        descendant = traces["observer-descendant.trace"]
        for path in (
            "/bin/bash",
            "/usr/bin/env",
            "/work/evidence/observer-child-sentinel",
            "/work/evidence/observer-child-elf-sentinel",
            "/work/evidence/observer-grandchild-sentinel",
            "/work/evidence/observer-grandchild-elf-sentinel",
        ):
            if not successful_exec(descendant, path):
                raise RuntimeError(f"child/grandchild trace lacks an exact successful executable record: {path}")
        if descendant.count('"-i"') < 2:
            raise RuntimeError("child/grandchild trace does not prove env -i execution")
        if not successful_exec(traces["observer-argv-only-decoy.trace"], "/usr/bin/find") or "/work/evidence/observer-old-generation" not in traces["observer-argv-only-decoy.trace"]:
            raise RuntimeError("argv-only old-generation data control was not observed through successful find execution")
        missing = "/work/evidence/observer-missing-executable"
        failed = [line for line in traces["observer-failed-exec.trace"].splitlines() if re.search(r'execve\(' + re.escape(json.dumps(missing)) + r",", line) is not None]
        if len(failed) != 1 or not failed[0].rstrip().endswith("= -1 ENOENT (No such file or directory)") or any(line.rstrip().endswith("= 0") for line in failed):
            raise RuntimeError("failed exec control did not retain one exact ENOENT without a false successful execution")
        execveat_lines = [line for line in traces["observer-execveat.trace"].splitlines() if "execveat(" in line]
        if len(execveat_lines) != 1 or '""' not in execveat_lines[0] or "AT_EMPTY_PATH" not in execveat_lines[0] or not execveat_lines[0].rstrip().endswith(" = 0"):
            raise RuntimeError("execveat control lacks an exact successful AT_EMPTY_PATH event")
        witness = json.loads(text("observer-execveat-witness.json"))
        target = directory / "observer-execveat-target"
        target_stat = target.stat()
        if witness != {"device": target_stat.st_dev, "fd": witness.get("fd"), "inode": target_stat.st_ino, "path": str(target), "sha256": sha(target), "size": target_stat.st_size} or not isinstance(witness["fd"], int) or witness["fd"] <= 0:
            raise RuntimeError("execveat descriptor witness does not bind the target path, inode, size, and bytes")
        parser_status = text("observer-execveat-parser.status.txt")
        if parser_status != "status=1\noutput_exists=no\n" or "execveat observation is unresolved by the pathname-only observer" not in text("observer-execveat-parser.stderr"):
            raise RuntimeError("actual current parser did not reject the observed execveat control without output JSON")
        argv_status = text("observer-argv-only-decoy-parser.status.txt")
        argv_json = json.loads(text("observer-argv-only-decoy.json"))
        if argv_status != "status=0\noutput_exists=yes\n" or argv_json.get("record_count") < 1:
            raise RuntimeError("actual current parser did not accept the successful argv-only data decoy")
        failed_parser_status = text("observer-failed-exec-parser.status.txt")
        failed_parser = json.loads(text("observer-failed-exec-parser.json"))
        if failed_parser_status != "status=0\noutput_exists=yes\n" or any(record["executable"] == missing for record in failed_parser.get("records", [])):
            raise RuntimeError("actual current parser counted a failed ENOENT execve as an executed program")
        direct_names = ["observer-native-true", "observer-translated-true", "observer-descendant", "observer-argv-only-decoy", "observer-failed-exec", "observer-execveat"]
        for name in direct_names:
            results[name] = assert_capture(name)
            trace_path = directory / f"{name}.trace"
            results[name]["trace_bytes"] = trace_path.stat().st_size
            results[name]["trace_sha256"] = sha(trace_path)
            results[name]["capture_status_sha256"] = sha(directory / f"{name}.capture-status.json")
            results[name]["wrapper_status_sha256"] = sha(directory / f"{name}.wrapper-status.txt")
        wrapper_status = json.loads(text("observer-candidate-wrapper.capture-status.json"))
        wrapper_log = text("observer-candidate-wrapper.capture.log")
        if wrapper_status.get("child_exit_status") != 0 or wrapper_status.get("output_truncated") is not False or wrapper_status.get("capture_rejected") is not False or wrapper_status.get("secret_output_detected") is not False or wrapper_status.get("child_argv_contains_secret") is not False or text("observer-candidate-wrapper.wrapper-status.txt") != "wrapper_status=0\n":
            raise RuntimeError("candidate observer wrapper/capture path rejected, truncated, or failed its child")
        for path in ("/work/evidence/observer-child-elf-sentinel", "/work/evidence/observer-grandchild-elf-sentinel"):
            if not successful_exec(wrapper_log, path):
                raise RuntimeError(f"candidate wrapper capture log lacks exact descendant execution: {path}")
        for marker in ("observer-wrapper-start", "observer-child-shell", "observer-grandchild-shell"):
            if marker not in wrapper_log:
                raise RuntimeError(f"candidate wrapper capture log lacks its executed output marker: {marker}")
        results["candidate_capture_wrapper"] = {
            "child_exit_status": wrapper_status["child_exit_status"],
            "capture_wrapper_status": 0,
            "output_bytes_total": wrapper_status["output_bytes_total"],
            "output_bytes_retained": wrapper_status["output_bytes_retained"],
            "output_truncated": wrapper_status["output_truncated"],
            "capture_rejected": wrapper_status["capture_rejected"],
            "log_sha256": sha(directory / "observer-candidate-wrapper.capture.log"),
            "log_bytes": (directory / "observer-candidate-wrapper.capture.log").stat().st_size,
        }
        runtime_args = json.loads(text("observer-trace-argv-template.json"))
        if runtime_args[0:3] != ["/usr/bin/timeout", "300", f"/opt/exec-observer/lib/{Path(interpreter.group(1)).name}"] or "-e" not in runtime_args or "trace=execve,execveat" not in runtime_args:
            raise RuntimeError("the retained observer argv does not show the private ARM64 loader, bounded timeout, and exec trace")
        if text("observer-runtime.txt").splitlines()[-1] != "dpkg_architecture=amd64":
            raise RuntimeError("AMD64 runtime architecture diagnostic differs from the pinned base fixture")
        elf_records = [
            f"/opt/exec-observer/bin/strace\t{elf_machine(tracer)}\t{sha(tracer)}",
            f"/opt/exec-observer/bin/observer-native-true\t{elf_machine(native_true)}\t{sha(native_true)}",
            f"/usr/bin/true\t{elf_machine('/usr/bin/true')}\t{sha(Path('/usr/bin/true'))}",
            f"/usr/bin/bash\t{elf_machine('/usr/bin/bash')}\t{sha(Path('/usr/bin/bash'))}",
            f"/usr/bin/python3\t{elf_machine('/usr/bin/python3')}\t{sha(Path('/usr/bin/python3'))}",
            f"{target}\t{elf_machine(target)}\t{sha(target)}",
            f"{directory / 'observer-unexecuted-sentinel'}\t{elf_machine(directory / 'observer-unexecuted-sentinel')}\t{sha(directory / 'observer-unexecuted-sentinel')}",
        ]
        for executable in ("bash", "env", "find", "printf", "python3", "true"):
            exported_path = directory / "observer-tools/amd64" / executable
            elf_records.append(f"/work/evidence/observer-tools/amd64/{executable}\t{elf_machine(exported_path)}\t{sha(exported_path)}")
        for row in closure_rows[1:]:
            original, resolved, private, digest, machine = row.split("\t")
            path = directory / "observer-tools" / Path(private).relative_to("/opt/exec-observer")
            elf_records.append(f"{private}\t{elf_machine(path)}\t{sha(path)}")
        (directory / "elf-identity.tsv").write_text("path\telf_machine\tsha256\n" + "\n".join(elf_records) + "\n", encoding="utf-8")
        results["native_machine"] = 183
        results["amd64_machine"] = 62
        results["unexecuted_sentinel_exists_but_unobserved"] = True
        results["execveat_parser_refusal_status"] = 1
        results["argv_only_decoy_parser_status"] = 0
        results["observer_trace_argv_template"] = runtime_args
        results["trace_sha256"] = {name: sha(directory / name) for name in trace_names}
        results["observer_tools_build_status_sha256"] = sha(directory / "observer-tools-build-status.txt")
        results["observer_tools_signature_status_sha256"] = sha(directory / "observer-tools-signature-status.txt")
        results["observer_tools_source_sha256"] = sha(directory / "observer-tools-source.tsv")
        results["observer_tools_closure_sha256"] = sha(directory / "observer-tools-closure.tsv")
        results["observer_tools_elf_sha256"] = sha(directory / "observer-tools-elf.tsv")
        results["observer_tools_versions_sha256"] = sha(directory / "observer-tools-versions.tsv")
        results["available"] = True
        reasons = []
    except (OSError, ValueError, KeyError, TypeError, RuntimeError, json.JSONDecodeError, struct.error) as error:
        results["available"] = False
        reasons = [str(error).replace("\n", " ")]
    status = {"schema": 1, "available": results["available"], "results": results, "reasons": reasons}
    (directory / "observer-control-status.json").write_text(json.dumps(status, sort_keys=True, indent=2) + "\n", encoding="utf-8")
    if results["available"]:
        (directory / "observer-capability.txt").write_text("observer=strace-7.3\ncapability=available\narchitecture_evidence=ELF-machine-183-with-private-loader-and-closure\ncontrols=native,translated,env-i-child-grandchild,argv-decoy,failed-exec,execveat,wrapper-capture\n", encoding="utf-8")
    else:
        (directory / "observer-capability.txt").write_text("observer=strace-7.3\ncapability=unavailable\narchitecture_evidence=ELF-only\n", encoding="utf-8")
        (directory / "observer-control-failure.txt").write_text("\n".join(reasons) + "\n", encoding="utf-8")


if len(sys.argv) == 3 and sys.argv[2] == "--assert":
    status = json.loads((Path(sys.argv[1]) / "observer-control-status.json").read_text(encoding="utf-8"))
    marker = (Path(sys.argv[1]) / "observer-capability.txt").read_text(encoding="utf-8")
    if status.get("schema") != 1 or status.get("available") is not True or marker != "observer=strace-7.3\ncapability=available\narchitecture_evidence=ELF-machine-183-with-private-loader-and-closure\ncontrols=native,translated,env-i-child-grandchild,argv-decoy,failed-exec,execveat,wrapper-capture\n":
        raise SystemExit("observer evidence does not qualify the exact native ARM64 observer controls")
elif len(sys.argv) == 2:
    main()
else:
    raise SystemExit("usage: observer-preflight EVIDENCE_DIRECTORY | observer-preflight EVIDENCE_DIRECTORY --assert")
