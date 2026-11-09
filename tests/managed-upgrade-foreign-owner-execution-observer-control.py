#!/usr/bin/env python3
# SPDX-FileCopyrightText: 2026 Alexey Zhokhov
# SPDX-License-Identifier: Apache-2.0
import importlib.util
import difflib
import hashlib
import json
import os
import subprocess
import sys
import tempfile
from pathlib import Path


worktree = Path(__file__).resolve().parents[1]
observer_path = worktree / "tests/managed-upgrade-foreign-owner-execution-observer.py"
driver_path = Path("/Users/donbeave/Projects/jackin-project/handoff/state-2026-11-05/current/baseline-foreign-owner-proof-v1/driver.py")
sys.dont_write_bytecode = True
spec = importlib.util.spec_from_file_location("foreign_owner_driver_for_observer_control", driver_path)
driver = importlib.util.module_from_spec(spec)
spec.loader.exec_module(driver)


def line(executable, arguments, status="0"):
    return f"execve({json.dumps(executable)}, {json.dumps(arguments)}, 0x0) = {status}"


def main():
    if len(sys.argv) != 2:
        raise SystemExit("usage: execution-observer-control EVIDENCE_DIRECTORY")
    root = Path(sys.argv[1])
    root.mkdir(mode=0o700, parents=True, exist_ok=False)
    try:
        actual = observer_path.read_bytes()
        actual_hash = hashlib.sha256(actual).hexdigest()
        private_path = root / "private-current-parser-before-mutation.py"
        restored_path = root / "restored-current-parser.py"
        mutant_path = root / "focused-current-parser-mutant.py"
        private_path.write_bytes(actual)
        restored_path.write_bytes(actual)
        needle = b'    if forbidden(executable_path, "executable"):\n        raise ObservationFailure("an installed prior executable was invoked")\n'
        insertion = needle + b'    for argument in arguments[1:]:\n        if os.path.isabs(argument) and forbidden(argument, "argument"):\n            raise ObservationFailure("a forbidden prior path was received as an invocation argument")\n'
        if actual.count(needle) != 1:
            raise SystemExit("focused parser mutation anchor did not occur exactly once in the actual current parser")
        mutant = actual.replace(needle, insertion, 1)
        mutant_path.write_bytes(mutant)
        mutation_diff = "".join(difflib.unified_diff(actual.decode("utf-8").splitlines(keepends=True), mutant.decode("utf-8").splitlines(keepends=True), fromfile="actual-current-parser.py", tofile="focused-current-parser-mutant.py"))
        (root / "focused-mutant.diff").write_text(mutation_diff, encoding="utf-8")
        if mutation_diff.count("+    for argument in arguments[1:]:") != 1 or mutation_diff.count("-    for argument in arguments[1:]:"):
            raise SystemExit("focused parser mutation diff contains changes outside the approved operand check")
        old_generation = root / "old-generation"
        old_runtime = old_generation / "python" / "runtime"
        hooks = root / "hooks"
        current = root / "current"
        for directory in (old_runtime / "bin", hooks, current):
            directory.mkdir(parents=True, exist_ok=True)
        old_checker = old_generation / "check-generated-workflows.sh"
        old_helper = old_generation / "python-inventory.sh"
        old_commit = hooks / "pre-commit"
        old_push = hooks / "pre-push"
        old_python = old_runtime / "bin/python3.15"
        old_inventory = old_generation / "python" / "inventory"
        current_helper = current / "python-inventory.sh"
        current_checker = current / "check-generated-workflows.sh"
        for path in (old_checker, old_helper, old_commit, old_push, old_python, old_inventory, current_helper, current_checker):
            path.write_text("control\n", encoding="utf-8")
        neighboring = root / "old-generation-neighbor"
        neighboring.mkdir()
        forbidden = (old_generation, old_runtime, old_checker, old_helper, old_commit, old_push)
        forbidden_arguments = tuple(str(path) for path in forbidden)
        status = {
            "old_generation_path": str(old_generation),
            "old_python_runtime_path": str(old_runtime),
            "old_checker_path": str(old_checker),
            "old_inventory_helper_path": str(old_helper),
            "pre_commit_hook_path": str(old_commit),
            "pre_push_hook_path": str(old_push),
        }
        cases = root / "cases"
        cases.mkdir()

        def invoke(name, records, accepted, parser=restored_path):
            trace = cases / f"{name}.trace"
            output = cases / f"{name}.json"
            trace.write_text("\n".join(records) + "\n", encoding="utf-8")
            command = [sys.executable, str(parser), str(trace), str(output), *forbidden_arguments]
            result = subprocess.run(command, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True, check=False)
            (cases / f"{name}.stdout").write_text(result.stdout, encoding="utf-8")
            (cases / f"{name}.stderr").write_text(result.stderr, encoding="utf-8")
            (cases / f"{name}.status.txt").write_text(f"status={result.returncode}\noutput_exists={str(output.exists()).lower()}\n", encoding="utf-8")
            (cases / f"{name}.command.json").write_text(json.dumps(command, ensure_ascii=False, separators=(",", ":")) + "\n", encoding="utf-8")
            if accepted:
                if result.returncode != 0 or not output.is_file():
                    raise SystemExit(f"observer control {name} rejected an allowed invocation: {result.stderr.strip()}")
                data = json.loads(output.read_text(encoding="utf-8"))
                successful_count = sum(record.rstrip().endswith(" = 0") for record in records)
                if set(data) != {"observer", "record_count", "records"} or data["observer"] != "strace-execve" or data["record_count"] != successful_count:
                    raise SystemExit(f"observer control {name} returned an unexpected evidence schema")
                parsed = driver.verify_execution_observer(output)
                driver.assert_old_code_not_executed(parsed, status)
                return parsed
            if result.returncode == 0 or output.exists():
                raise SystemExit(f"observer control {name} accepted forbidden or unclassifiable input")
            return None

        allowed_records = [
            line("/usr/bin/env", ["env", "-i", "PATH=/usr/bin:/bin", "/bin/bash", str(current_helper), "check", str(old_runtime), str(old_inventory)]),
            line("/usr/bin/find", ["find", str(old_runtime), "-type", "f"]),
            line("/bin/bash", ["bash", "-o", "posix", "--", str(current_checker), str(old_runtime)]),
            line("/usr/bin/env", ["env", "-i", "-S", "GITHUB_COM_TOKEN=${GITHUB_COM_TOKEN}", "HOME=/tmp", "/bin/bash", str(current_helper), "check", str(old_runtime), str(old_inventory)]),
            line("/usr/bin/find", ["find", str(neighboring), "-type", "f"]),
        ]
        focused_trace = cases / "allowed-data-arguments.trace"
        focused_trace.write_text("\n".join(allowed_records) + "\n", encoding="utf-8")
        red_output = cases / "focused-mutant-red.json"
        red_command = [sys.executable, str(mutant_path), str(focused_trace), str(red_output), *forbidden_arguments]
        red = subprocess.run(red_command, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True, check=False)
        (cases / "focused-mutant-red.stdout").write_text(red.stdout, encoding="utf-8")
        (cases / "focused-mutant-red.stderr").write_text(red.stderr, encoding="utf-8")
        (cases / "focused-mutant-red.status.txt").write_text(f"status={red.returncode}\noutput_exists={str(red_output.exists()).lower()}\n", encoding="utf-8")
        (cases / "focused-mutant-red.command.json").write_text(json.dumps(red_command, ensure_ascii=False, separators=(",", ":")) + "\n", encoding="utf-8")
        expected_diagnostic = "a forbidden prior path was received as an invocation argument"
        if red.returncode == 0 or red_output.exists() or expected_diagnostic not in red.stderr:
            raise SystemExit("focused current parser mutant did not fail on the intended forbidden data argument without JSON output")
        allowed = invoke("allowed-data-arguments", allowed_records, True)
        missing_exec = str(root / "missing-executable")
        failed_exec_records = [
            line(missing_exec, [missing_exec], "-1 ENOENT (No such file or directory)"),
            line("/usr/bin/true", ["true"]),
        ]
        failed_exec = invoke("failed-exec-result-filter", failed_exec_records, True)
        if failed_exec["record_count"] != 1 or failed_exec["records"][0]["executable"] != "/usr/bin/true":
            raise SystemExit("observer control counted a failed execve attempt as an executed program")
        for record in allowed["records"]:
            if record["arguments"] not in [json.loads(json.dumps(arguments)) for arguments in [
                ["env", "-i", "PATH=/usr/bin:/bin", "/bin/bash", str(current_helper), "check", str(old_runtime), str(old_inventory)],
                ["find", str(old_runtime), "-type", "f"],
                ["bash", "-o", "posix", "--", str(current_checker), str(old_runtime)],
                ["env", "-i", "-S", "GITHUB_COM_TOKEN=${GITHUB_COM_TOKEN}", "HOME=/tmp", "/bin/bash", str(current_helper), "check", str(old_runtime), str(old_inventory)],
                ["find", str(neighboring), "-type", "f"],
            ]]:
                raise SystemExit("observer control changed the retained argv data")
        restored_output = cases / "focused-restored-green.json"
        restored_command = [sys.executable, str(restored_path), str(focused_trace), str(restored_output), *forbidden_arguments]
        green = subprocess.run(restored_command, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True, check=False)
        (cases / "focused-restored-green.stdout").write_text(green.stdout, encoding="utf-8")
        (cases / "focused-restored-green.stderr").write_text(green.stderr, encoding="utf-8")
        (cases / "focused-restored-green.status.txt").write_text(f"status={green.returncode}\noutput_exists={str(restored_output.exists()).lower()}\n", encoding="utf-8")
        (cases / "focused-restored-green.command.json").write_text(json.dumps(restored_command, ensure_ascii=False, separators=(",", ":")) + "\n", encoding="utf-8")
        if green.returncode != 0 or not restored_output.is_file() or restored_output.read_bytes() == b"":
            raise SystemExit("exact restored current parser did not accept the same allowed-data trace")
        restored_data = json.loads(restored_output.read_text(encoding="utf-8"))
        if restored_data["record_count"] != len(allowed_records) or restored_data["records"] != allowed["records"] or hashlib.sha256(restored_path.read_bytes()).hexdigest() != actual_hash:
            raise SystemExit("exact restored current parser output or source hash differs from the approved green control")
        red_compare = json.loads((cases / "focused-mutant-red.command.json").read_text(encoding="utf-8"))
        green_compare = json.loads((cases / "focused-restored-green.command.json").read_text(encoding="utf-8"))
        red_compare[1] = green_compare[1] = "<parser-copy>"
        red_compare[3] = green_compare[3] = "<fresh-output>"
        if red_compare != green_compare:
            raise SystemExit("focused red and restored green CLI arguments differ beyond their fresh output path")
        hashes = {
            "actual_current_parser_sha256": actual_hash,
            "private_parser_before_mutation_sha256": hashlib.sha256(private_path.read_bytes()).hexdigest(),
            "focused_mutant_sha256": hashlib.sha256(mutant).hexdigest(),
            "restored_parser_sha256": hashlib.sha256(restored_path.read_bytes()).hexdigest(),
            "allowed_data_trace_sha256": hashlib.sha256(focused_trace.read_bytes()).hexdigest(),
            "focused_mutant_diff_sha256": hashlib.sha256(mutation_diff.encode("utf-8")).hexdigest(),
            "focused_mutant_red_stderr_sha256": hashlib.sha256(red.stderr.encode("utf-8")).hexdigest(),
            "focused_mutant_red_stdout_sha256": hashlib.sha256(red.stdout.encode("utf-8")).hexdigest(),
            "focused_mutant_red_command_sha256": hashlib.sha256(json.dumps(red_command, ensure_ascii=False, separators=(",", ":")).encode("utf-8")).hexdigest(),
            "focused_restored_green_output_sha256": hashlib.sha256(restored_output.read_bytes()).hexdigest(),
            "focused_restored_green_stderr_sha256": hashlib.sha256(green.stderr.encode("utf-8")).hexdigest(),
            "focused_restored_green_stdout_sha256": hashlib.sha256(green.stdout.encode("utf-8")).hexdigest(),
            "focused_restored_green_command_sha256": hashlib.sha256(json.dumps(restored_command, ensure_ascii=False, separators=(",", ":")).encode("utf-8")).hexdigest(),
        }
        (root / "source-and-control-hashes.json").write_text(json.dumps(hashes, sort_keys=True, indent=2) + "\n", encoding="utf-8")

        denied_records = [
            line(str(old_python), [str(old_python), "-I", "-S"]),
            line("/bin/bash", ["bash", "--", str(old_commit)]),
            line("/usr/bin/python3", ["python3", "-W", "ignore", "-X", "dev", "--", str(old_helper)]),
            line("/usr/bin/env", ["env", "-i", "/bin/bash", str(old_checker)]),
        ]
        for index, record in enumerate(denied_records):
            invoke(f"forbidden-program-slot-{index}", [record], False)
        host_denied = (
            {"executable": str(old_python), "arguments": [str(old_python), "-I", "-S"]},
            {"executable": "/bin/bash", "arguments": ["bash", "--", str(old_commit)]},
            {"executable": "/usr/bin/python3", "arguments": ["python3", "-W", "ignore", "-X", "dev", "--", str(old_helper)]},
            {"executable": "/usr/bin/env", "arguments": ["env", "-i", "/bin/bash", str(old_checker)]},
            {"executable": "/bin/bash", "arguments": ["bash", "--unknown-option", str(old_commit)]},
            {"executable": "/usr/bin/python3", "arguments": ["python3", "--unknown-option", str(old_helper)]},
            {"executable": "/bin/bash", "arguments": ["bash", "old-generation/pre-commit"]},
            {"executable": "relative/python3", "arguments": ["python3", str(old_python)]},
        )
        for entry in host_denied:
            try:
                driver.assert_old_code_not_executed({"observer": "strace-execve", "record_count": 1, "records": [entry]}, status)
            except SystemExit:
                continue
            raise SystemExit("host observer oracle accepted forbidden or unresolved program-slot data")

        opaque_mode_records = [
            line("/bin/bash", ["bash", "-c", f"source {old_commit}", "bash-data", str(old_runtime)]),
            line("/bin/bash", ["bash", "-s", "--", str(old_runtime)]),
            line("/usr/bin/python3", ["python3", "-c", f"exec(open({str(old_helper)!r}).read())", str(old_runtime)]),
            line("/usr/bin/python3", ["python3", "-m", "old.module", str(old_runtime)]),
            line("/usr/bin/python3", ["python3", "-", str(old_runtime)]),
            line("/usr/bin/python3", ["python3", "-X", "dev", str(current_checker), str(old_runtime)]),
        ]
        invoke("opaque-command-module-stdin-and-data", opaque_mode_records, True)
        for index, record in enumerate((
            line("/bin/bash", ["bash", "--unknown-option", str(old_commit)]),
            line("/usr/bin/python3", ["python3", "--unknown-option", str(old_helper)]),
            line("/bin/bash", ["bash", "old-generation/pre-commit"]),
            line("relative/python3", ["python3", str(old_python)]),
        )):
            invoke(f"insufficient-or-unknown-{index}", [record], False)
        malformed_trace = cases / "malformed.trace"
        malformed_trace.write_text('execve("/bin/bash", [unquoted], 0x0) = 0\n', encoding="utf-8")
        malformed_output = cases / "malformed.json"
        malformed = subprocess.run([sys.executable, str(restored_path), str(malformed_trace), str(malformed_output), *forbidden_arguments], stdout=subprocess.PIPE, stderr=subprocess.PIPE, check=False)
        (cases / "malformed.stdout").write_bytes(malformed.stdout)
        (cases / "malformed.stderr").write_bytes(malformed.stderr)
        (cases / "malformed.status.txt").write_text(f"status={malformed.returncode}\noutput_exists={str(malformed_output.exists()).lower()}\n", encoding="utf-8")
        if malformed.returncode == 0 or malformed_output.exists():
            raise SystemExit("observer control accepted malformed execve telemetry")
        empty_trace = cases / "empty.trace"
        empty_trace.write_text("+++ exited with 0 +++\n", encoding="utf-8")
        empty_output = cases / "empty.json"
        empty = subprocess.run([sys.executable, str(restored_path), str(empty_trace), str(empty_output), *forbidden_arguments], stdout=subprocess.PIPE, stderr=subprocess.PIPE, check=False)
        (cases / "empty.stdout").write_bytes(empty.stdout)
        (cases / "empty.stderr").write_bytes(empty.stderr)
        (cases / "empty.status.txt").write_text(f"status={empty.returncode}\noutput_exists={str(empty_output.exists()).lower()}\n", encoding="utf-8")
        if empty.returncode == 0 or empty_output.exists():
            raise SystemExit("observer control accepted a trace with no execve records")
        execveat_trace = cases / "execveat.trace"
        execveat_trace.write_text('execveat(AT_FDCWD, "/bin/true", ["/bin/true"], 0x0) = 0\n', encoding="utf-8")
        execveat_output = cases / "execveat.json"
        execveat = subprocess.run([sys.executable, str(restored_path), str(execveat_trace), str(execveat_output), *forbidden_arguments], stdout=subprocess.PIPE, stderr=subprocess.PIPE, check=False)
        (cases / "execveat.stdout").write_bytes(execveat.stdout)
        (cases / "execveat.stderr").write_bytes(execveat.stderr)
        (cases / "execveat.status.txt").write_text(f"status={execveat.returncode}\noutput_exists={str(execveat_output.exists()).lower()}\n", encoding="utf-8")
        if execveat.returncode == 0 or execveat_output.exists():
            raise SystemExit("observer control silently accepted unclassified execveat telemetry")
        print("cases=slot-aware-positive,failed-exec-result-filter,forbidden-executable,bash-script,python-script,env-wrapper,command-mode,stdin,module,option-values,relative-path,unknown-option,malformed,empty,execveat")
        print("historical_original_parser_witness=unavailable")
        print("focused_current_parser_mutant_red=passed")
        print("restored_current_parser_green=passed")
        print("guest_observer_and_host_oracle=passed")
    except BaseException:
        raise


if __name__ == "__main__":
    main()
