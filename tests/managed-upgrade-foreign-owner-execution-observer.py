#!/usr/bin/env python3
# SPDX-FileCopyrightText: 2026 Alexey Zhokhov
# SPDX-License-Identifier: Apache-2.0
import json
import os
import re
import shlex
import sys
from pathlib import Path


class ObservationFailure(Exception):
    pass


def parse_execve(line):
    match = re.search(r'execve\("((?:\\.|[^"\\])*)", \[(.*?)\],', line)
    if match is None:
        raise ObservationFailure("an execve observation could not be parsed")
    executable = json.loads('"' + match.group(1) + '"')
    arguments = [json.loads(value) for value in re.findall(r'"(?:\\.|[^"\\])*"', match.group(2))]
    if any(not isinstance(value, str) for value in [executable, *arguments]):
        raise ObservationFailure("an execve observation contains a non-string path or argument")
    return executable, arguments


def execve_succeeded(line):
    match = re.search(r"\)\s+=\s+(-?\d+)(?:\s+([A-Z][A-Z0-9_]*))?(?:\s+\([^)]*\))?(?:\s+<[^>]+>)?\s*$", line)
    if match is None:
        raise ObservationFailure("an execve result could not be classified as successful or failed")
    if match.group(1) == "0":
        return True
    if match.group(1) == "-1" and match.group(2) is not None:
        return False
    raise ObservationFailure("an execve result has an unsupported status")


def absolute_path(value, description):
    if not os.path.isabs(value):
        raise ObservationFailure(f"relative {description} path cannot be resolved from execve-only evidence")
    return os.path.normpath(value)


def make_forbidden_matcher(paths):
    normalized = {absolute_path(path, "forbidden") for path in paths}
    directories = {path for path in normalized if Path(path).is_dir()}

    def matches(value, description):
        candidate = absolute_path(value, description)
        return candidate in normalized or any(candidate.startswith(root + os.sep) for root in directories)

    return matches


def shell_script_operand(arguments):
    if not arguments:
        raise ObservationFailure("shell execve has no argv needed to classify its program slot")
    words = arguments[1:]
    index = 0
    short_options = set("abefhkmnptuvxBCEHPTD")
    long_options = {
        "--debugger",
        "--dump-po-strings",
        "--dump-strings",
        "--help",
        "--login",
        "--noediting",
        "--noprofile",
        "--norc",
        "--posix",
        "--restricted",
        "--verbose",
        "--version",
        "--wordexp",
    }
    while index < len(words):
        word = words[index]
        if word == "--":
            return words[index + 1] if index + 1 < len(words) else None
        if word == "-":
            return None
        if word in ("-c", "-s"):
            if word == "-c" and index + 1 >= len(words):
                raise ObservationFailure("bash -c lacks its command-string operand")
            return None
        if word in ("-o", "+o", "-O", "+O", "--rcfile", "--init-file"):
            if index + 1 >= len(words):
                raise ObservationFailure("bash option lacks its value operand")
            index += 2
            continue
        if word.startswith("--rcfile=") or word.startswith("--init-file="):
            index += 1
            continue
        if word in long_options:
            index += 1
            continue
        if word.startswith("-") and len(word) > 1:
            flags = word[1:]
            if any(flag not in short_options and flag not in "cs" for flag in flags):
                raise ObservationFailure("unsupported or ambiguous bash option prevents script-slot classification")
            if "c" in flags:
                if index + 1 >= len(words):
                    raise ObservationFailure("combined bash -c option lacks its command-string operand")
                return None
            if "s" in flags:
                return None
            index += 1
            continue
        if word in ("+o", "+O"):
            index += 1
            continue
        if word.startswith("+"):
            raise ObservationFailure("unsupported or ambiguous bash option prevents script-slot classification")
        return word
    return None


def python_script_operand(arguments):
    if not arguments:
        raise ObservationFailure("Python execve has no argv needed to classify its program slot")
    words = arguments[1:]
    index = 0
    no_value_flags = set("bBdEhiIOPqRsSuvV")
    while index < len(words):
        word = words[index]
        if word == "--":
            return words[index + 1] if index + 1 < len(words) else None
        if word == "-":
            return None
        if word in ("-c", "-m"):
            if index + 1 >= len(words):
                raise ObservationFailure("Python execution mode lacks its code or module operand")
            return None
        if word in ("-W", "-X", "--check-hash-based-pycs"):
            if index + 1 >= len(words):
                raise ObservationFailure("Python option lacks its value operand")
            index += 2
            continue
        if word.startswith("--check-hash-based-pycs=") or word.startswith("-W") or word.startswith("-X"):
            index += 1
            continue
        if word in ("--help", "--help-env", "--help-xoptions", "--help-all", "--version"):
            return None
        if word.startswith("-") and len(word) > 1:
            flags = word[1:]
            if any(flag not in no_value_flags for flag in flags):
                raise ObservationFailure("unsupported or ambiguous Python option prevents script-slot classification")
            index += 1
            continue
        return word
    return None


def env_command(arguments):
    if not arguments:
        raise ObservationFailure("env execve has no argv needed to classify its command slot")
    words = list(arguments[1:])
    index = 0
    split_count = 0
    while index < len(words):
        word = words[index]
        if word == "--":
            index += 1
            break
        if word in ("-i", "--ignore-environment", "-0", "--null", "-v", "--debug"):
            index += 1
            continue
        if word in ("-u", "--unset", "-C", "--chdir"):
            if index + 1 >= len(words):
                raise ObservationFailure("env option lacks its value operand")
            index += 2
            continue
        if word.startswith("--unset=") or word.startswith("--chdir="):
            index += 1
            continue
        if word in ("-S", "--split-string") or word.startswith("--split-string="):
            if word.startswith("--split-string="):
                split_text = word.partition("=")[2]
                index += 1
            else:
                if index + 1 >= len(words):
                    raise ObservationFailure("env split-string option lacks its value operand")
                split_text = words[index + 1]
                index += 2
            split_count += 1
            if split_count > 4:
                raise ObservationFailure("repeated env split-string options are ambiguous")
            try:
                words[index:index] = shlex.split(split_text, posix=True)
            except ValueError as error:
                raise ObservationFailure("env split-string operand cannot be parsed") from error
            continue
        if word.startswith("-"):
            raise ObservationFailure("unsupported or ambiguous env option prevents command-slot classification")
        if re.fullmatch(r"[A-Za-z_][A-Za-z0-9_]*=.*", word, re.DOTALL):
            index += 1
            continue
        return word, words[index + 1:]
    while index < len(words) and re.fullmatch(r"[A-Za-z_][A-Za-z0-9_]*=.*", words[index], re.DOTALL):
        index += 1
    if index < len(words):
        return words[index], words[index + 1:]
    return None, []


def inspect_invocation(executable, arguments, forbidden, depth=0):
    if depth > 3:
        raise ObservationFailure("nested env/interpreter command exceeds the classified observation bound")
    executable_path = absolute_path(executable, "execve executable")
    if forbidden(executable_path, "executable"):
        raise ObservationFailure("an installed prior executable was invoked")
    executable_name = Path(executable_path).name
    if executable_name == "env":
        target, target_arguments = env_command(arguments)
        if target is not None:
            inspect_invocation(target, [target, *target_arguments], forbidden, depth + 1)
        return
    if executable_name in {"bash", "sh", "dash", "zsh"}:
        script = shell_script_operand(arguments)
    elif executable_name in {"python", "python2", "python3", "python3.15", "pypy", "pypy3"} or re.fullmatch(r"python(?:2|3)(?:\.\d+)*", executable_name):
        script = python_script_operand(arguments)
    else:
        return
    if script is None or script in ("-", "/dev/stdin"):
        return
    script_path = absolute_path(script, "interpreter script operand")
    if forbidden(script_path, "interpreter script"):
        raise ObservationFailure("an installed prior script occupied an interpreter program slot")


def main():
    if len(sys.argv) < 5:
        raise SystemExit("usage: execution-observer INPUT_LOG OUTPUT_LOG FORBIDDEN_EXECUTABLE...")
    source = Path(sys.argv[1])
    target = Path(sys.argv[2])
    forbidden = make_forbidden_matcher(sys.argv[3:])
    records = []
    for line in source.read_text(encoding="utf-8", errors="strict").splitlines():
        if "execveat(" in line:
            raise ObservationFailure("execveat observation is unresolved by the pathname-only observer")
        if "execve(" not in line:
            continue
        if not execve_succeeded(line):
            continue
        executable, arguments = parse_execve(line)
        inspect_invocation(executable, arguments, forbidden)
        records.append({"executable": executable, "arguments": arguments})
    if not records:
        raise ObservationFailure("no execve observations were retained")
    with target.open("x", encoding="utf-8") as stream:
        stream.write(json.dumps({"observer": "strace-execve", "record_count": len(records), "records": records}, sort_keys=True, separators=(",", ":")) + "\n")


if __name__ == "__main__":
    try:
        main()
    except ObservationFailure as error:
        raise SystemExit(f"managed-upgrade-exec-observer: {error}") from error
