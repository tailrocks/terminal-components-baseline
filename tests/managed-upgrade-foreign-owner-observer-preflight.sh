#!/usr/bin/env bash
# SPDX-FileCopyrightText: 2026 Alexey Zhokhov
# SPDX-License-Identifier: Apache-2.0
set -eu

directory=${1:?}
proof_root=$(cd "$(dirname "$0")" && pwd -P)
runtime_helper="$proof_root/managed-upgrade-foreign-owner-observer-runtime.sh"
directory=$(cd "$directory" && pwd -P)

source "$runtime_helper"
mkdir -p "$directory"
/usr/bin/cp -a /opt/exec-observer/evidence/. "$directory/"
/usr/bin/mkdir -p "$directory/observer-old-generation/python/runtime/bin"
/usr/bin/printf 'data-only fixture\n' > "$directory/observer-old-generation/check-generated-workflows.sh"
/usr/bin/printf 'data-only fixture\n' > "$directory/observer-old-generation/python/runtime/bin/python3.15"
/usr/bin/mkdir -p "$directory/observer-tools"
/usr/bin/cp -a /opt/exec-observer/bin "$directory/observer-tools/bin"
/usr/bin/cp -a /opt/exec-observer/lib "$directory/observer-tools/lib"
/usr/bin/mkdir -p "$directory/observer-tools/amd64"
for executable in bash env find printf python3 true; do
  /usr/bin/cp -L "/usr/bin/$executable" "$directory/observer-tools/amd64/$executable"
done

/usr/bin/cp /usr/bin/printf "$directory/observer-child-elf-sentinel"
/usr/bin/cp /usr/bin/printf "$directory/observer-grandchild-elf-sentinel"
/usr/bin/cp /usr/bin/true "$directory/observer-unexecuted-sentinel"
/usr/bin/cp /usr/bin/printf "$directory/observer-execveat-target"
/usr/bin/cp "$proof_root/managed-upgrade-foreign-owner-observer-child.sh" "$directory/observer-child-sentinel"
/usr/bin/cp "$proof_root/managed-upgrade-foreign-owner-observer-grandchild.sh" "$directory/observer-grandchild-sentinel"
/usr/bin/cp "$proof_root/managed-upgrade-foreign-owner-observer-wrapper.sh" "$directory/observer-wrapper.sh"
/usr/bin/chmod 0755 "$directory/observer-child-sentinel" "$directory/observer-grandchild-sentinel" "$directory/observer-wrapper.sh"
/usr/bin/touch "$directory/observer-missing-executable.guard"
/usr/bin/rm "$directory/observer-missing-executable.guard"

observer_initialize || {
  /usr/bin/python3 "$proof_root/managed-upgrade-foreign-owner-observer-preflight.py" "$directory"
  exit 0
}

run_direct_control() {
  local name=$1
  shift
  local trace="$directory/$name.trace"
  local status="$directory/$name.capture-status.json"
  local log="$directory/$name.capture.log"
  local wrapper_status=0
  observer_prepare_trace "$trace" || return 1
  if (ulimit -f 16384; GITHUB_COM_TOKEN=observer-preflight-canary-only observer_capture_child "$status" "$log" /work "${observer_trace_argv[@]}" "$@"); then
    wrapper_status=0
  else
    wrapper_status=$?
  fi
  /usr/bin/printf 'wrapper_status=%s\n' "$wrapper_status" > "$directory/$name.wrapper-status.txt"
}

run_direct_control observer-native-true /opt/exec-observer/bin/observer-native-true
run_direct_control observer-translated-true /usr/bin/true
run_direct_control observer-descendant /bin/bash "$directory/observer-wrapper.sh"
run_direct_control observer-argv-only-decoy /usr/bin/find "$directory/observer-old-generation" -type f
run_direct_control observer-failed-exec /usr/bin/python3 "$proof_root/managed-upgrade-foreign-owner-observer-failed-exec.py" "$directory/observer-missing-executable"
run_direct_control observer-execveat /usr/bin/python3 "$proof_root/managed-upgrade-foreign-owner-observer-execveat.py" "$directory/observer-execveat-target" "$directory/observer-execveat-witness.json"

observer_prepare_trace /dev/stderr || exit 1
wrapper_capture_status=0
if (ulimit -f 16384; GITHUB_COM_TOKEN=observer-preflight-canary-only observer_capture_child "$directory/observer-candidate-wrapper.capture-status.json" "$directory/observer-candidate-wrapper.capture.log" /work "${observer_trace_argv[@]}" /bin/bash "$directory/observer-wrapper.sh"); then
  wrapper_capture_status=0
else
  wrapper_capture_status=$?
fi
/usr/bin/printf 'wrapper_status=%s\n' "$wrapper_capture_status" > "$directory/observer-candidate-wrapper.wrapper-status.txt"

execveat_parser_status=0
if /usr/bin/python3 "$proof_root/managed-upgrade-foreign-owner-execution-observer.py" "$directory/observer-execveat.trace" "$directory/observer-execveat-parser.json" "$directory/observer-old-generation" "$directory/observer-old-generation/python/runtime" > "$directory/observer-execveat-parser.stdout" 2> "$directory/observer-execveat-parser.stderr"; then
  execveat_parser_status=0
else
  execveat_parser_status=$?
fi
/usr/bin/printf 'status=%s\noutput_exists=%s\n' "$execveat_parser_status" "$([[ -e "$directory/observer-execveat-parser.json" ]] && /usr/bin/printf yes || /usr/bin/printf no)" > "$directory/observer-execveat-parser.status.txt"

argv_decoy_parser_status=0
if /usr/bin/python3 "$proof_root/managed-upgrade-foreign-owner-execution-observer.py" "$directory/observer-argv-only-decoy.trace" "$directory/observer-argv-only-decoy.json" "$directory/observer-old-generation" "$directory/observer-old-generation/python/runtime" > "$directory/observer-argv-only-decoy-parser.stdout" 2> "$directory/observer-argv-only-decoy-parser.stderr"; then
  argv_decoy_parser_status=0
else
  argv_decoy_parser_status=$?
fi
/usr/bin/printf 'status=%s\noutput_exists=%s\n' "$argv_decoy_parser_status" "$([[ -e "$directory/observer-argv-only-decoy.json" ]] && /usr/bin/printf yes || /usr/bin/printf no)" > "$directory/observer-argv-only-decoy-parser.status.txt"

failed_exec_parser_status=0
if /usr/bin/python3 "$proof_root/managed-upgrade-foreign-owner-execution-observer.py" "$directory/observer-failed-exec.trace" "$directory/observer-failed-exec-parser.json" "$directory/observer-old-generation" "$directory/observer-old-generation/python/runtime" > "$directory/observer-failed-exec-parser.stdout" 2> "$directory/observer-failed-exec-parser.stderr"; then
  failed_exec_parser_status=0
else
  failed_exec_parser_status=$?
fi
/usr/bin/printf 'status=%s\noutput_exists=%s\n' "$failed_exec_parser_status" "$([[ -e "$directory/observer-failed-exec-parser.json" ]] && /usr/bin/printf yes || /usr/bin/printf no)" > "$directory/observer-failed-exec-parser.status.txt"

/usr/bin/dpkg-query -W -f='${binary:Package}\t${Version}\n' apt bash ca-certificates coreutils curl diffutils dpkg findutils gawk git grep gzip passwd patch perl procps python3 sed tar util-linux xz-utils > "$directory/tool-versions.tsv" 2> "$directory/tool-versions.stderr" || true
/usr/bin/timeout 10 /usr/bin/sed -n '1,2p' /opt/exec-observer/evidence/observer-tools-strace-version.txt > "$directory/strace-version.txt" 2> "$directory/strace-version.stderr" || true
/usr/bin/printf 'uname_diagnostic_only=' > "$directory/observer-runtime.txt"
/usr/bin/uname -m >> "$directory/observer-runtime.txt" 2> "$directory/observer-runtime.stderr" || true
/usr/bin/printf 'dpkg_architecture=' >> "$directory/observer-runtime.txt"
/usr/bin/dpkg --print-architecture >> "$directory/observer-runtime.txt" 2>> "$directory/observer-runtime.stderr" || true
observer_prepare_trace __TRACE_OUTPUT__ || exit 1
/usr/bin/python3 - "$directory/observer-trace-argv-template.json" "${observer_trace_argv[@]}" <<'PY'
import json
import sys

with open(sys.argv[1], "x", encoding="utf-8") as stream:
    json.dump(sys.argv[2:], stream, separators=(",", ":"))
    stream.write("\n")
PY

/usr/bin/python3 "$proof_root/managed-upgrade-foreign-owner-observer-preflight.py" "$directory"
