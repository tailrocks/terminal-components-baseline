#!/usr/bin/env bash
# SPDX-FileCopyrightText: 2026 Alexey Zhokhov
# SPDX-License-Identifier: Apache-2.0

observer_root=/opt/exec-observer
observer_lib="$observer_root/lib"
observer_binary="$observer_root/bin/strace"
observer_capture_helper="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd -P)/managed-upgrade-foreign-owner-capture.py"
observer_runtime_argv=()
observer_trace_argv=()

observer_initialize() {
  local build_status="$observer_root/evidence/observer-tools-build-status.txt"
  local interpreter
  [[ -f "$build_status" && ! -L "$build_status" ]] || return 1
  /usr/bin/grep -F -x -q 'status=available' "$build_status" || return 1
  interpreter=$(/usr/bin/sed -n 's/^elf_interpreter=//p' "$build_status")
  [[ "$interpreter" == /* && "$interpreter" != *$'\n'* ]] || return 1
  local loader="$observer_lib/${interpreter##*/}"
  [[ -x "$loader" && -x "$observer_binary" ]] || return 1
  observer_runtime_argv=("$loader" --library-path "$observer_lib" "$observer_binary")
}

observer_prepare_trace() {
  local output=$1
  [[ ${#observer_runtime_argv[@]} -eq 3 ]] || return 1
  observer_trace_argv=(/usr/bin/timeout 300 "${observer_runtime_argv[@]}" -f -qq -e trace=execve,execveat -s 16384 -o "$output")
}

observer_capture_child() {
  local status_path=$1
  local log_path=$2
  local working_directory=$3
  shift 3
  local capture_canary=${GITHUB_COM_TOKEN-}
  [[ -n "$capture_canary" ]] || return 125
  GITHUB_COM_TOKEN="$capture_canary" /usr/bin/python3 "$observer_capture_helper" capture "$status_path" "$log_path" "$working_directory" -- "$@"
}
