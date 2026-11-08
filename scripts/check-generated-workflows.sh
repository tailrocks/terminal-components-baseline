#!/usr/bin/env bash
# SPDX-FileCopyrightText: 2026 Alexey Zhokhov
# SPDX-License-Identifier: Apache-2.0
set -euo pipefail

export GIT_NO_REPLACE_OBJECTS=1

repo_root=$(cd "$(dirname "$0")/.." && pwd -P)
mise_command=$(command -v mise) || {
  printf 'generated-workflows: mise is unavailable\n' >&2
  exit 127
}
git_command=$(command -v git) || {
  printf 'generated-workflows: git is unavailable\n' >&2
  exit 127
}
caller_path=$PATH
clean_git_environment=("PATH=$caller_path" "HOME=${TMPDIR:-/tmp}" GIT_CONFIG_NOSYSTEM=1 GIT_CONFIG_GLOBAL=/dev/null GIT_CONFIG_SYSTEM=/dev/null GIT_NO_REPLACE_OBJECTS=1 GIT_NO_LAZY_FETCH=1)
repo_root=$(env -i "${clean_git_environment[@]}" "$git_command" -C "$repo_root" rev-parse --show-toplevel) || {
  printf 'generated-workflows: cannot resolve the checker repository\n' >&2
  exit 1
}
repo_root=$(cd "$repo_root" && pwd -P)
objects=$(env -i "${clean_git_environment[@]}" "$git_command" -C "$repo_root" rev-parse --path-format=absolute --git-path objects) || {
  printf 'generated-workflows: cannot resolve the source object directory\n' >&2
  exit 1
}
objects=$(cd "$objects" && pwd -P) || {
  printf 'generated-workflows: cannot resolve the source object directory\n' >&2
  exit 1
}
object_format=$(env -i "${clean_git_environment[@]}" "$git_command" -C "$repo_root" rev-parse --show-object-format) || {
  printf 'generated-workflows: cannot resolve the source object format\n' >&2
  exit 1
}
effective_index=$(env -i "${clean_git_environment[@]}" "$git_command" -C "$repo_root" rev-parse --path-format=absolute --git-path index)
python_command=
for candidate in /opt/homebrew/bin/python3 /usr/local/bin/python3 /usr/bin/python3 /opt/local/bin/python3; do
  if [[ -x "$candidate" ]] && "$candidate" -I -S -c 'import sys, tomllib; sys.exit(sys.version_info < (3, 11))' >/dev/null 2>&1; then
    python_command=$candidate
    break
  fi
done
if [[ -z "$python_command" ]]; then
  printf 'generated-workflows: isolated Python 3.11 or newer with tomllib is unavailable\n' >&2
  exit 127
fi
curl_candidate=$(command -v curl) || {
  printf 'generated-workflows: curl is unavailable in the caller PATH\n' >&2
  exit 127
}
curl_command=$("$python_command" -I -S - "$curl_candidate" <<'PY'
import os
import sys
from pathlib import Path

try:
    path = Path(sys.argv[1]).resolve(strict=True)
except OSError:
    raise SystemExit(1)
if not path.is_absolute() or not path.is_file() or not os.access(path, os.X_OK):
    raise SystemExit(1)
print(path)
PY
) || {
  printf 'generated-workflows: caller PATH curl is not an executable file\n' >&2
  exit 127
}

case "$object_format" in
  sha1) object_length=40 ;;
  sha256) object_length=64 ;;
  *) printf 'generated-workflows: unsupported Git object format %s\n' "$object_format" >&2; exit 2 ;;
esac

temporary_base=${TMPDIR:-/tmp}
temporary_root=$(mktemp -d "$temporary_base/jackin-generated-workflows.XXXXXX") || {
  printf 'generated-workflows: cannot create a temporary directory under %s\n' "$temporary_base" >&2
  exit 1
}
cleanup_temporary_root() {
  chmod u+w "$temporary_root" 2>/dev/null || true
  find "$temporary_root" -type d -exec chmod u+w {} + 2>/dev/null || true
  find "$temporary_root" -type f -exec chmod u+w {} + 2>/dev/null || true
  find "$temporary_root" -depth -delete
}
trap cleanup_temporary_root EXIT
temporary_root=$(cd "$temporary_root" && pwd -P) || {
  printf 'generated-workflows: cannot resolve the temporary directory\n' >&2
  exit 1
}
source_alternate_dirs="$temporary_root/source-alternate-directories"
env -i "${clean_git_environment[@]}" "$git_command" -C "$repo_root" count-objects -v | sed -n 's/^alternate: //p' > "$source_alternate_dirs" || {
  printf 'generated-workflows: cannot resolve source alternate object directories\n' >&2
  exit 1
}
trusted_private_objects="$temporary_root/trusted-seed-objects"
index_tree_objects="$temporary_root/index-tree-objects"
verified_blob_objects="$temporary_root/verified-blob-objects"
snapshot_reader="$temporary_root/snapshot-reader.git"
maximum_snapshot_entries=8192
maximum_snapshot_blob_bytes=$((16 * 1024 * 1024))
maximum_snapshot_total_bytes=$((5 * 512 * 1024 * 1024))
snapshot_blob_allowlist="$repo_root/scripts/snapshot-large-blob-allowlist.tsv"
maximum_snapshot_listing_bytes=$((8 * 1024 * 1024))
maximum_hydration_missing_blobs=1024
maximum_hydration_total_bytes=$((256 * 1024 * 1024))
hydration_budget_file="$temporary_root/blob-hydration-budget"
hydration_deadline_file="$temporary_root/blob-hydration-deadline"

trusted_seed_commit=a7413118476128a1c9b4f290277eb91ddf64ea7a
trusted_lock_sha256=54035a5337e01cd95129b464ffbd0db45e4b0c989a699742efa1f0ab6329046d
snapshot_lock_sha256=c3d0a169604e33af6f0288d4be3f8dbdb69c08db73c748d97f44e802142640bf
snapshot_manifests_sha256=a05ef9aa0b74c5bb2301994ddff8ed5f7eb20f53e905e990cde19587ad063b71

fail_materialization() {
  printf 'generated-workflows: cannot materialize snapshot: %s\n' "$1" >&2
  return 1
}

git_in_snapshot_context() {
  env -i \
    "HOME=$temporary_root/private-git-home" \
    "PATH=${git_command%/*}:/usr/bin:/bin:/usr/sbin:/sbin" \
    GIT_CONFIG_NOSYSTEM=1 \
    GIT_CONFIG_GLOBAL=/dev/null \
    GIT_CONFIG_SYSTEM=/dev/null \
    GIT_NO_REPLACE_OBJECTS=1 \
    GIT_NO_LAZY_FETCH=1 \
    GIT_OPTIONAL_LOCKS=0 \
    "$git_command" "$@"
}

git_in_trusted_reader() {
  local reader=$1
  shift
  env -i \
    "HOME=$temporary_root/trusted-reader-home" \
    "PATH=${git_command%/*}:/usr/bin:/bin:/usr/sbin:/sbin" \
    "GIT_OBJECT_DIRECTORY=$trusted_private_objects" \
    "GIT_ALTERNATE_OBJECT_DIRECTORIES=$verified_blob_objects:$objects" \
    GIT_CONFIG_NOSYSTEM=1 \
    GIT_CONFIG_GLOBAL=/dev/null \
    GIT_CONFIG_SYSTEM=/dev/null \
    GIT_NO_REPLACE_OBJECTS=1 \
    GIT_NO_LAZY_FETCH=1 \
    GIT_OPTIONAL_LOCKS=0 \
    "$git_command" --git-dir="$reader" -c core.hooksPath=/dev/null "$@"
}

git_in_private_snapshot_reader() {
  env -i \
    "HOME=$temporary_root/private-git-home" \
    "PATH=${git_command%/*}:/usr/bin:/bin:/usr/sbin:/sbin" \
    "GIT_DIR=$snapshot_reader" \
    "GIT_OBJECT_DIRECTORY=$index_tree_objects" \
    "GIT_ALTERNATE_OBJECT_DIRECTORIES=$verified_blob_objects:$objects" \
    GIT_CONFIG_NOSYSTEM=1 \
    GIT_CONFIG_GLOBAL=/dev/null \
    GIT_CONFIG_SYSTEM=/dev/null \
    GIT_NO_REPLACE_OBJECTS=1 \
    GIT_NO_LAZY_FETCH=1 \
    GIT_OPTIONAL_LOCKS=0 \
    "$git_command" -c core.hooksPath=/dev/null "$@"
}

git_with_private_tree_objects() {
  local private_index=${1-}
  local -a index_environment=()
  shift || true
  if [[ -n "$private_index" ]]; then
    index_environment+=("GIT_INDEX_FILE=$private_index")
  fi
  env -i \
    "HOME=$temporary_root/private-git-home" \
    "PATH=${git_command%/*}:/usr/bin:/bin:/usr/sbin:/sbin" \
    "GIT_DIR=$snapshot_reader" \
    "GIT_OBJECT_DIRECTORY=$index_tree_objects" \
    "GIT_ALTERNATE_OBJECT_DIRECTORIES=$verified_blob_objects:$objects" \
    GIT_CONFIG_NOSYSTEM=1 \
    GIT_CONFIG_GLOBAL=/dev/null \
    GIT_CONFIG_SYSTEM=/dev/null \
    GIT_NO_REPLACE_OBJECTS=1 \
    GIT_NO_LAZY_FETCH=1 \
    GIT_OPTIONAL_LOCKS=0 \
    "${index_environment[@]}" \
    "$git_command" -c core.hooksPath=/dev/null "$@"
}

validate_source_object_store() {
  local path
  local alternates="$objects/info/alternates"
  local http_alternates="$objects/info/http-alternates"

  if [[ -L "$objects" || ! -d "$objects" ]]; then
    printf 'generated-workflows: source object directory is not a regular directory\n' >&2
    return 1
  fi
  for path in "$alternates" "$http_alternates"; do
    if [[ -L "$path" || -s "$path" ]]; then
      printf 'generated-workflows: source object store uses an unsupported alternate object source\n' >&2
      return 1
    fi
  done
  if [[ "$objects" == *:* || "$objects" == *$'\n'* || "$objects" == *$'\r'* || "$objects" == *\\* || "$objects" == *\"* ]]; then
    printf 'generated-workflows: source object directory cannot be represented as a trusted reader alternate\n' >&2
    return 1
  fi
  if [[ "$verified_blob_objects" == "$objects" || -L "$verified_blob_objects" ]]; then
    printf 'generated-workflows: private verified blob object directory is unsafe\n' >&2
    return 1
  fi
  if [[ "$verified_blob_objects" == *:* || "$verified_blob_objects" == *$'\n'* || "$verified_blob_objects" == *$'\r'* || "$verified_blob_objects" == *\\* || "$verified_blob_objects" == *\"* ]]; then
    printf 'generated-workflows: private verified blob directory cannot be represented as a reader alternate\n' >&2
    return 1
  fi
  (umask 077 && mkdir -p "$index_tree_objects/info" "$index_tree_objects/pack" "$verified_blob_objects/info" "$verified_blob_objects/pack") || return $?
}

initialize_snapshot_reader() {
  validate_source_object_store || return $?
  if [[ -d "$snapshot_reader" && ! -L "$snapshot_reader" && -f "$snapshot_reader/HEAD" ]]; then
    return 0
  fi
  mkdir -p "$temporary_root/private-git-home" || return $?
  env -i \
    "HOME=$temporary_root/private-git-home" \
    "PATH=${git_command%/*}:/usr/bin:/bin:/usr/sbin:/sbin" \
    GIT_CONFIG_NOSYSTEM=1 \
    GIT_CONFIG_GLOBAL=/dev/null \
    GIT_CONFIG_SYSTEM=/dev/null \
    GIT_NO_REPLACE_OBJECTS=1 \
    GIT_NO_LAZY_FETCH=1 \
    "$git_command" init --bare --quiet --object-format="$object_format" "$snapshot_reader" || {
      printf 'generated-workflows: cannot create the isolated snapshot Git reader\n' >&2
      return 1
    }
}

initialize_trusted_reader() {
  local reader=$1
  validate_source_object_store || return $?
  if [[ "$trusted_private_objects" == "$objects" || -L "$trusted_private_objects" ]]; then
    printf 'generated-workflows: private trusted seed object directory is unsafe\n' >&2
    return 1
  fi
  mkdir -p "$temporary_root/trusted-reader-home" || return $?
  (umask 077 && mkdir -p "$trusted_private_objects/info" "$trusted_private_objects/pack") || return $?
  env -i \
    "HOME=$temporary_root/trusted-reader-home" \
    "PATH=${git_command%/*}:/usr/bin:/bin:/usr/sbin:/sbin" \
    GIT_CONFIG_NOSYSTEM=1 \
    GIT_CONFIG_GLOBAL=/dev/null \
    GIT_CONFIG_SYSTEM=/dev/null \
    GIT_NO_REPLACE_OBJECTS=1 \
    "$git_command" init --bare --quiet --object-format="$object_format" "$reader" || {
      printf 'generated-workflows: cannot create the isolated trusted source reader\n' >&2
      return 1
    }
}

validate_trusted_tree_entries() {
  "$python_command" -I -S - "$1" "$object_length" <<'PY'
import re
import sys
from pathlib import PurePosixPath

entries_path, object_length = sys.argv[1:]
raw = open(entries_path, "rb").read()
if not raw.endswith(b"\0"):
    raise SystemExit("generated-workflows: trusted seed tree listing is incomplete")
seen = set()
for entry in raw[:-1].split(b"\0"):
    try:
        header, path_bytes = entry.split(b"\t", 1)
        mode, kind, object_id = header.decode("ascii").split(" ")
        path = path_bytes.decode("utf-8", "strict")
    except (ValueError, UnicodeError):
        raise SystemExit("generated-workflows: trusted seed tree entry is malformed")
    parts = PurePosixPath(path).parts
    if not path or path.startswith("/") or "\\" in path or any(part in {"", ".", "..", ".git"} for part in parts):
        raise SystemExit("generated-workflows: trusted seed contains an unsafe path")
    if any(part == ".gitmodules" for part in parts):
        raise SystemExit("generated-workflows: trusted seed contains submodule metadata")
    if any(ord(character) < 32 or ord(character) == 127 for character in path):
        raise SystemExit("generated-workflows: trusted seed path contains control characters")
    if path in seen:
        raise SystemExit("generated-workflows: trusted seed contains a duplicate path")
    seen.add(path)
    if re.fullmatch(r"[0-9a-f]{" + object_length + r"}", object_id) is None:
        raise SystemExit("generated-workflows: trusted seed has an invalid Git object identity")
    if mode == "160000" or kind == "commit":
        raise SystemExit("generated-workflows: trusted seed contains a Git submodule")
    if (mode, kind) not in {("100644", "blob"), ("100755", "blob"), ("120000", "blob")}:
        raise SystemExit("generated-workflows: trusted seed contains an unsupported Git tree entry")
PY
}

bounded_tree_listing() {
  local kind=$1
  local reader=${2-}
  local tree=$3
  local destination=$4
  local -a args=(ls-tree -r -z --full-tree "$tree")
  local program='import sys
from pathlib import Path

destination, maximum_entries, maximum_bytes = sys.argv[1:]
maximum_entries = int(maximum_entries)
maximum_bytes = int(maximum_bytes)
path = Path(destination)
entries = 0
size = 0
partial = b""
with path.open("wb") as output:
    while True:
        chunk = sys.stdin.buffer.read(65536)
        if not chunk:
            break
        size += len(chunk)
        if size > maximum_bytes:
            raise SystemExit("generated-workflows: snapshot tree listing exceeds the size limit")
        combined = partial + chunk
        entries += combined.count(b"\0")
        if entries > maximum_entries:
            raise SystemExit("generated-workflows: snapshot tree exceeds the entry count limit")
        partial = combined.rsplit(b"\0", 1)[-1]
        output.write(chunk)
if partial:
    raise SystemExit("generated-workflows: snapshot tree listing is incomplete")'
  if [[ "$kind" == trusted ]]; then
    git_in_trusted_reader "$reader" "${args[@]}" | "$python_command" -I -S -c "$program" "$destination" "$maximum_snapshot_entries" "$maximum_snapshot_listing_bytes"
  else
    git_in_private_snapshot_reader "${args[@]}" | "$python_command" -I -S -c "$program" "$destination" "$maximum_snapshot_entries" "$maximum_snapshot_listing_bytes"
  fi
}

validate_snapshot_tree_entries() {
  "$python_command" -I -S - "$1" "$object_length" <<'PY'
import re
import sys


entries_path, object_length = sys.argv[1:]
raw = open(entries_path, "rb").read()
if raw and not raw.endswith(b"\0"):
    raise SystemExit("generated-workflows: snapshot tree listing is incomplete")
seen = set()
for entry in raw.split(b"\0")[:-1] if raw else ():
    try:
        header, path_bytes = entry.split(b"\t", 1)
        mode, kind, object_id = header.decode("ascii").split(" ")
        path = path_bytes.decode("utf-8", "strict")
    except (ValueError, UnicodeError):
        raise SystemExit("generated-workflows: snapshot tree entry is malformed")
    parts = path.split("/")
    if not path or path.startswith("/") or path.endswith("/") or "//" in path or "\\" in path or any(part in {"", ".", "..", ".git"} for part in parts):
        raise SystemExit("generated-workflows: snapshot tree contains an unsafe path")
    if any(ord(character) < 32 or ord(character) == 127 for character in path):
        raise SystemExit("generated-workflows: snapshot tree path contains control characters")
    if path in seen:
        raise SystemExit("generated-workflows: snapshot tree contains a duplicate path")
    seen.add(path)
    if re.fullmatch(r"[0-9a-f]{" + object_length + r"}", object_id) is None:
        raise SystemExit("generated-workflows: snapshot tree has an invalid Git object identity")
    if mode == "160000" or kind == "commit":
        raise SystemExit("generated-workflows: cannot materialize snapshot: snapshot contains unsupported gitlink entries")
    if (mode, kind) not in {("100644", "blob"), ("100755", "blob"), ("120000", "blob")}:
        raise SystemExit("generated-workflows: snapshot tree contains an unsupported entry type")
PY
}

preflight_tree_blob_sizes() {
  local kind=$1
  local reader=${2-}
  local entries=$3
  local allow_missing=${4:-false}
  local oids="$temporary_root/tree-blob-oids"
  local statuses="$temporary_root/tree-blob-status"
  "$python_command" -I -S - "$entries" "$oids" <<'PY'
import sys
from pathlib import Path

entries_path, output_path = sys.argv[1:]
raw = Path(entries_path).read_bytes()
if raw and not raw.endswith(b"\0"):
    raise SystemExit("generated-workflows: snapshot tree listing is incomplete")
objects = []
for entry in raw.split(b"\0")[:-1] if raw else ():
    header, _ = entry.split(b"\t", 1)
    mode, kind, oid = header.decode("ascii").split(" ")
    if kind == "blob" and mode in {"100644", "100755", "120000"}:
        objects.append(oid)
Path(output_path).write_text("".join(oid + "\n" for oid in objects))
PY
  if [[ "$kind" == trusted ]]; then
    git_in_trusted_reader "$reader" cat-file --batch-check < "$oids" > "$statuses" || return $?
  else
    git_in_private_snapshot_reader cat-file --batch-check < "$oids" > "$statuses" || return $?
  fi
  "$python_command" -I -S "$repo_root/scripts/validate-snapshot-blob-sizes.py" \
    "$entries" "$statuses" "$allow_missing" "$maximum_snapshot_blob_bytes" \
    "$maximum_snapshot_total_bytes" "$kind" "$snapshot_blob_allowlist"
}

project_trusted_tree_entries() {
  "$python_command" -I -S - "$1" "$2" <<'PY'
import sys
from pathlib import Path

source_path, destination_path = sys.argv[1:]
prefixes = (
    "tests/functional/baselines/functional/",
    "apps/jackin/tests/functional/baselines/functional/",
)
raw = Path(source_path).read_bytes()
if not raw.endswith(b"\0"):
    raise SystemExit("generated-workflows: snapshot tree listing is incomplete")
projected = []
for entry in raw[:-1].split(b"\0"):
    header, path_bytes = entry.split(b"\t", 1)
    mode, kind, object_id = header.decode("ascii").split(" ")
    path = path_bytes.decode("utf-8", "strict")
    excluded = False
    for prefix in prefixes:
        if path.startswith(prefix):
            name = path[len(prefix):]
            if "/" not in name and name.endswith(".tui.json"):
                if (mode, kind) != ("100644", "blob"):
                    raise SystemExit("generated-workflows: trusted seed baseline projection contains an unsupported entry type")
                excluded = True
                break
    if not excluded:
        projected.append(entry)
Path(destination_path).write_bytes(b"\0".join(projected) + (b"\0" if projected else b""))
PY
}

hydrate_blobs() {
  local kind=$1
  local reader=${2-}
  local entries=$3
  local scope=$4
  local description=$5
  local all_oids="$temporary_root/$scope-blob-oids"
  local object_status="$temporary_root/$scope-blob-object-status"
  local missing_oids="$temporary_root/$scope-missing-blob-oids"
  local response_dir="$temporary_root/$scope-blob-responses"
  local object_id
  local written_oid
  local verified_oid
  local missing_count
  local remaining_count
  local remaining_bytes
  local downloaded_bytes
  local writer
  local writer_home
  local -a writer_environment
  local used_count
  local used_bytes
  local extra_budget_field

  "$python_command" -I -S - "$entries" "$all_oids" <<'PY'
import sys
from pathlib import Path

source_path, destination_path = sys.argv[1:]
raw = Path(source_path).read_bytes()
if not raw.endswith(b"\0"):
    raise SystemExit("generated-workflows: trusted seed tree listing is incomplete")
objects = set()
for entry in raw[:-1].split(b"\0"):
    header, _ = entry.split(b"\t", 1)
    mode, kind, object_id = header.decode("ascii").split(" ")
    if kind == "blob":
        objects.add(object_id)
Path(destination_path).write_text("".join(f"{object_id}\n" for object_id in sorted(objects)))
PY
  if [[ "$kind" == trusted ]]; then
    git_in_trusted_reader "$reader" cat-file --batch-check < "$all_oids" > "$object_status" || return $?
  else
    git_in_private_snapshot_reader cat-file --batch-check < "$all_oids" > "$object_status" || return $?
  fi
  "$python_command" -I -S - "$all_oids" "$object_status" "$missing_oids" "$kind" <<'PY' || return $?
import sys
from pathlib import Path

oids_path, status_path, missing_path, kind = sys.argv[1:]
oids = Path(oids_path).read_text().splitlines()
statuses = Path(status_path).read_text().splitlines()
if len(oids) != len(statuses):
    raise SystemExit(f"generated-workflows: {kind} snapshot object inspection was incomplete")
missing = []
for expected, line in zip(oids, statuses, strict=True):
    fields = line.split()
    if len(fields) == 2 and fields == [expected, "missing"]:
        missing.append(expected)
    elif len(fields) == 3 and fields[0] == expected and fields[1] == "blob" and fields[2].isdigit():
        continue
    else:
        raise SystemExit(f"generated-workflows: {kind} snapshot blob object has an unexpected type")
Path(missing_path).write_text("".join(f"{object_id}\n" for object_id in missing))
PY
  if [[ ! -s "$missing_oids" ]]; then
    return 0
  fi
  if [[ "$object_format" != sha1 ]]; then
    fail_materialization "missing snapshot blobs can only be hydrated for SHA-1 Git repositories"
    return 1
  fi
  if [[ "$kind" == trusted ]]; then
    writer=$reader
    writer_home="$temporary_root/trusted-reader-home"
  else
    writer=$snapshot_reader
    writer_home="$temporary_root/private-git-home"
  fi
  if [[ ! -f "$hydration_budget_file" ]]; then
    (umask 077 && printf '0 0\n' > "$hydration_budget_file") || return $?
  fi
  if ! IFS=' ' read -r used_count used_bytes extra_budget_field < "$hydration_budget_file" ||
    [[ ! "$used_count" =~ ^[0-9]+$ || ! "$used_bytes" =~ ^[0-9]+$ || -n "$extra_budget_field" ]]; then
    fail_materialization "blob hydration invocation budget is invalid"
    return 1
  fi
  missing_count=$(wc -l < "$missing_oids" | tr -d ' ') || return $?
  remaining_count=$((maximum_hydration_missing_blobs - used_count))
  remaining_bytes=$((maximum_hydration_total_bytes - used_bytes))
  if (( missing_count > remaining_count )); then
    fail_materialization "missing snapshot blobs exceed the invocation object count limit"
    return 1
  fi
  if (( remaining_bytes < 0 )); then
    fail_materialization "missing snapshot blobs exceed the invocation byte limit"
    return 1
  fi
  if [[ ! -f "$hydration_deadline_file" ]]; then
    "$python_command" -I -S - "$hydration_deadline_file" <<'PY' || return $?
import sys
import time
from pathlib import Path

with Path(sys.argv[1]).open("x") as deadline:
    deadline.write(str(time.monotonic() + 15 * 60))
PY
    chmod 600 "$hydration_deadline_file" || return $?
  fi
  mkdir -m 700 "$response_dir" || return $?
  downloaded_bytes=$(
    env -i \
    "HOME=$temporary_root/trusted-reader-home" \
    "PATH=${git_command%/*}:/usr/bin:/bin:/usr/sbin:/sbin" \
    "TMPDIR=$temporary_root" \
    LANG=C \
    LC_ALL=C \
      "$python_command" -I -S - "$missing_oids" "$response_dir" "$curl_command" "$temporary_root" "$description" "$remaining_count" "$remaining_bytes" "$hydration_deadline_file" <<'PY'
import base64
import hashlib
import json
import os
import re
import selectors
import subprocess
import sys
import tempfile
import time
from pathlib import Path

oids_path, response_dir, curl, temporary_root, description, maximum_missing_blobs, maximum_total_bytes, deadline_path = sys.argv[1:]
maximum_missing_blobs = int(maximum_missing_blobs)
maximum_total_bytes = int(maximum_total_bytes)
maximum_blob_bytes = 4 * 1024 * 1024
maximum_json_bytes = 6 * 1024 * 1024
maximum_header_bytes = 32 * 1024
phase_deadline = float(Path(deadline_path).read_text())
endpoint = "https://api.github.com/repos/tailrocks/terminal-components-baseline/git/blobs/"
redirect_statuses = {300, 301, 302, 303, 304, 305, 307, 308}

def reject(message):
    raise SystemExit(f"generated-workflows: {description} blob hydration {message}")

oids = Path(oids_path).read_text().splitlines()
if len(oids) > maximum_missing_blobs:
    reject("exceeds the missing object count limit")
if len(set(oids)) != len(oids) or any(re.fullmatch(r"[0-9a-f]{40}", oid) is None for oid in oids):
    reject("received an invalid verified object list")

def terminate(process):
    if process.poll() is None:
        process.terminate()
        try:
            process.wait(timeout=1)
        except subprocess.TimeoutExpired:
            process.kill()
            process.wait(timeout=1)

def response_headers(path):
    raw = path.read_bytes()
    if not raw:
        reject("response headers have an invalid size")
    if len(raw) > maximum_header_bytes:
        reject("response headers exceed the size limit")
    try:
        lines = raw.decode("iso-8859-1").replace("\r\n", "\n").split("\n")
    except UnicodeError:
        reject("response headers are malformed")
    status_lines = [line for line in lines if line.startswith("HTTP/")]
    if len(status_lines) != 1:
        reject("response has ambiguous status headers")
    match = re.fullmatch(r"HTTP/[0-9]+(?:\.[0-9]+)?[ \t]+([1-5][0-9]{2})(?:[ \t].*)?", status_lines[0])
    if match is None:
        reject("response has malformed status headers")
    seen = set()
    locations = []
    for line in lines[lines.index(status_lines[0]) + 1:]:
        if not line:
            continue
        if line[0] in " \t" or ":" not in line:
            reject("response headers are malformed")
        name, value = line.split(":", 1)
        lowered = name.lower()
        if not name or re.fullmatch(r"[!#$%&'*+.^_`|~0-9A-Za-z-]+", name) is None or lowered in seen:
            reject("response headers are ambiguous")
        seen.add(lowered)
        if lowered == "location":
            location = value.strip(" \t")
            if not location or any(ord(character) < 32 or ord(character) == 127 for character in location):
                reject("redirect location is malformed")
            locations.append(location)
    return int(match.group(1)), locations

def fetch(oid):
    if time.monotonic() >= phase_deadline:
        reject("exceeded the hydration time limit")
    current_url = endpoint + oid
    if re.fullmatch(re.escape(endpoint) + r"[0-9a-f]{40}", current_url) is None:
        reject("constructed an invalid fixed endpoint")
    with tempfile.TemporaryDirectory(prefix="trusted-blob-http-", dir=temporary_root) as temporary:
        header_path = Path(temporary) / "headers"
        command = [
            curl,
            "--disable",
            "--proto",
            "=https",
            "--silent",
            "--show-error",
            "--connect-timeout",
            "10",
            "--max-time",
            "30",
            "--max-filesize",
            str(maximum_json_bytes),
            "--dump-header",
            str(header_path),
            "--header",
            "Accept: application/vnd.github+json",
            "--header",
            "X-GitHub-Api-Version: 2022-11-28",
            current_url,
        ]
        environment = {
            "HOME": os.environ["HOME"],
            "PATH": os.environ["PATH"],
            "TMPDIR": os.environ["TMPDIR"],
            "LANG": "C",
            "LC_ALL": "C",
        }
        try:
            process = subprocess.Popen(command, env=environment, stdin=subprocess.DEVNULL, stdout=subprocess.PIPE, stderr=subprocess.DEVNULL)
        except OSError:
            reject("cannot start the trusted caller curl executable")
        selector = selectors.DefaultSelector()
        selector.register(process.stdout, selectors.EVENT_READ)
        payload = bytearray()
        request_deadline = min(time.monotonic() + 32, phase_deadline)
        try:
            while True:
                if header_path.exists() and header_path.stat().st_size > maximum_header_bytes:
                    terminate(process)
                    reject("response headers exceed the size limit")
                remaining = request_deadline - time.monotonic()
                if remaining <= 0:
                    terminate(process)
                    reject("request exceeded the time limit")
                ready = selector.select(min(remaining, 0.1))
                if ready:
                    chunk = os.read(process.stdout.fileno(), min(65536, maximum_json_bytes + 1 - len(payload)))
                    if not chunk:
                        break
                    payload.extend(chunk)
                    if len(payload) > maximum_json_bytes:
                        terminate(process)
                        reject("response exceeds the encoded size limit")
            try:
                process.wait(timeout=max(0.1, request_deadline - time.monotonic()))
            except subprocess.TimeoutExpired:
                terminate(process)
                reject("request exceeded the time limit")
        finally:
            selector.close()
            if process.poll() is None:
                terminate(process)
        if process.returncode != 0:
            if process.returncode == 63 or len(payload) > maximum_json_bytes:
                reject("response exceeds the encoded size limit")
            reject("cannot retrieve the fixed official blob endpoint")
        if not header_path.is_file():
            reject("response headers have an invalid size")
        if header_path.stat().st_size > maximum_header_bytes:
            reject("response headers exceed the size limit")
        status, locations = response_headers(header_path)
        if status in redirect_statuses or locations:
            reject("redirects are not permitted")
        if status != 200:
            reject(f"endpoint returned HTTP {status}")
        if not payload:
            reject("response is empty")
        return bytes(payload)

def unique_pairs(pairs):
    result = {}
    for key, value in pairs:
        if key in result:
            raise ValueError("duplicate JSON key")
        result[key] = value
    return result

total_decoded = 0
for oid in oids:
    response = fetch(oid)
    try:
        document = json.loads(
            response.decode("utf-8", "strict"),
            object_pairs_hook=unique_pairs,
            parse_constant=lambda value: (_ for _ in ()).throw(ValueError("invalid JSON constant")),
        )
    except (UnicodeError, ValueError, RecursionError, json.JSONDecodeError):
        reject("response JSON is malformed or ambiguous")
    if not isinstance(document, dict) or document.get("sha") != oid:
        reject("response object identity does not match the requested blob")
    if document.get("encoding") != "base64":
        reject("response encoding is unsupported")
    declared_size = document.get("size")
    if type(declared_size) is not int or declared_size < 0 or declared_size > maximum_blob_bytes:
        reject("response declares an invalid blob size")
    content = document.get("content")
    if not isinstance(content, str):
        reject("response content is not base64 text")
    content_without_crlf = content.replace("\r\n", "")
    if "\r" in content_without_crlf:
        reject("response base64 contains an unsupported line break")
    encoded = content_without_crlf.replace("\n", "")
    try:
        encoded_bytes = encoded.encode("ascii", "strict")
        decoded = base64.b64decode(encoded_bytes, validate=True)
    except (UnicodeError, ValueError):
        reject("response base64 is malformed")
    if len(decoded) != declared_size:
        reject("decoded blob size does not match the declaration")
    if len(decoded) > maximum_blob_bytes:
        reject("decoded blob exceeds the size limit")
    actual_oid = hashlib.sha1(b"blob " + str(len(decoded)).encode() + b"\0" + decoded).hexdigest()
    if actual_oid != oid:
        reject("decoded blob does not match its Git object identity")
    total_decoded += len(decoded)
    if total_decoded > maximum_total_bytes:
        reject("decoded data exceeds the total size limit")
    output = Path(response_dir) / oid
    with output.open("xb") as destination:
        destination.write(decoded)
    output.chmod(0o600)
print(total_decoded)
PY
  ) || return $?
  if [[ ! "$downloaded_bytes" =~ ^[0-9]+$ ]]; then
    fail_materialization "blob hydration returned an invalid invocation budget update"
    return 1
  fi
  printf '%s %s\n' "$((used_count + missing_count))" "$((used_bytes + downloaded_bytes))" > "$hydration_budget_file" || return $?
  writer_environment=(
    "HOME=$writer_home"
    "PATH=${git_command%/*}:/usr/bin:/bin:/usr/sbin:/sbin"
    "GIT_DIR=$writer"
    "GIT_OBJECT_DIRECTORY=$verified_blob_objects"
    GIT_CONFIG_NOSYSTEM=1
    GIT_CONFIG_GLOBAL=/dev/null
    GIT_CONFIG_SYSTEM=/dev/null
    GIT_NO_REPLACE_OBJECTS=1
    GIT_NO_LAZY_FETCH=1
    GIT_OPTIONAL_LOCKS=0
  )
  while IFS= read -r object_id || [[ -n "$object_id" ]]; do
    verified_oid=$(env -i "${writer_environment[@]}" "$git_command" hash-object --stdin < "$response_dir/$object_id") || {
      printf 'generated-workflows: %s blob hydration could not recheck the verified object privately\n' "$description" >&2
      return 1
    }
    if [[ "$verified_oid" != "$object_id" ]]; then
      printf 'generated-workflows: %s blob hydration private object identity changed before publication\n' "$description" >&2
      return 1
    fi
    written_oid=$(env -i "${writer_environment[@]}" "$git_command" hash-object -w --stdin < "$response_dir/$object_id") || {
      printf 'generated-workflows: %s blob hydration could not write the verified object privately\n' "$description" >&2
      return 1
    }
    if [[ "$written_oid" != "$object_id" ]]; then
      printf 'generated-workflows: %s blob hydration private object identity changed\n' "$description" >&2
      return 1
    fi
  done < "$missing_oids"
}

read_link_target() {
  local target
  target=$(readlink "$1" || exit $?; printf '\001') || return $?
  target=${target%$'\001'}
  target=${target%$'\n'}
  printf '%s' "$target"
}

validate_symlink_containment() {
  local snapshot=$1
  local link_path=$2
  local target
  local resolved
  local queue
  local component
  local candidate
  local link_depth=0

  target=$(read_link_target "$snapshot/$link_path") || return $?
  if [[ -z "$target" || "$target" == /* ]]; then
    fail_materialization "symlink target leaves snapshot"
    return 1
  fi

  resolved=${link_path%/*}
  [[ "$resolved" == "$link_path" ]] && resolved=
  queue=$target
  while [[ -n "$queue" ]]; do
    if [[ "$queue" == */* ]]; then
      component=${queue%%/*}
      queue=${queue#*/}
    else
      component=$queue
      queue=
    fi
    case "$component" in
      ''|.) ;;
      ..)
        if [[ -z "$resolved" ]]; then
          fail_materialization "symlink target leaves snapshot"
          return 1
        fi
        if [[ "$resolved" == */* ]]; then
          resolved=${resolved%/*}
        else
          resolved=
        fi
        ;;
      *)
        if [[ -n "$resolved" ]]; then
          candidate="$resolved/$component"
        else
          candidate=$component
        fi
        if [[ -L "$snapshot/$candidate" ]]; then
          link_depth=$((link_depth + 1))
          if [[ "$link_depth" -gt 40 ]]; then
            fail_materialization "symlink chain cannot be resolved within snapshot"
            return 1
          fi
          target=$(read_link_target "$snapshot/$candidate") || return $?
          if [[ -z "$target" || "$target" == /* ]]; then
            fail_materialization "symlink target leaves snapshot"
            return 1
          fi
          resolved=${candidate%/*}
          [[ "$resolved" == "$candidate" ]] && resolved=
          if [[ -n "$queue" ]]; then
            queue="$target/$queue"
          else
            queue=$target
          fi
        else
          resolved=$candidate
        fi
        ;;
    esac
  done
}

validate_snapshot_symlinks() {
  local snapshot=$1
  local path
  local links_file
  local status
  links_file=$(mktemp "$temporary_root/symlinks.XXXXXX") || return $?
  if ! find "$snapshot" -type l -print0 > "$links_file"; then
    rm -f "$links_file"
    return 1
  fi
  while IFS= read -r -d '' path || [[ -n "$path" ]]; do
    validate_symlink_containment "$snapshot" "${path#"$snapshot"/}" || {
      status=$?
      return "$status"
    }
  done < "$links_file"
  rm -f "$links_file" || return $?
}

materialize_tree() {
  local snapshot=$1
  local entries=$2
  local reader=${3-}
  local reader_kind=snapshot
  local reader_home=$temporary_root/private-git-home
  local private_objects=$index_tree_objects
  if [[ -n "$reader" ]]; then
    reader_kind=trusted
    reader_home=$temporary_root/trusted-reader-home
    private_objects=$trusted_private_objects
  else
    reader=$snapshot_reader
  fi
  TUISCO_READER_HOME="$reader_home" \
    TUISCO_GIT_PATH="${git_command%/*}:/usr/bin:/bin:/usr/sbin:/sbin" \
    TUISCO_MAX_BLOB="$maximum_snapshot_blob_bytes" \
    TUISCO_MAX_TOTAL="$maximum_snapshot_total_bytes" \
    "$python_command" -I -S "$repo_root/scripts/materialize-git-tree.py" \
    "$git_command" "$reader_kind" "$reader" "$private_objects" \
    "$verified_blob_objects" "$objects" "$entries" \
    "$temporary_root/tree-blob-status" "$snapshot_blob_allowlist" "$snapshot"
}

read_snapshot_tool_selection() {
  "$python_command" -I -S - "$1" <<'PY'
import re
import sys
import tomllib
from pathlib import Path
import os

snapshot = Path(sys.argv[1])
supported = {
    "0.1.74": {
        "rust": "1.99.1",
        "jactionlint": "2.4.0",
        "shellcheck": "0.11.0",
    },
}
selected = {"github:tailrocks/velnor-new": "velnor-actions", "rust": None, "jactionlint": None, "shellcheck": None}
supported_targets = ["aarch64-apple-darwin", "x86_64-apple-darwin", "aarch64-unknown-linux-gnu", "x86_64-unknown-linux-gnu"]

def literal_version(value, name):
    if type(value) is not str or re.fullmatch(r"(?:0|[1-9][0-9]*)\.(?:0|[1-9][0-9]*)\.(?:0|[1-9][0-9]*)", value) is None:
        raise SystemExit(f"generated-workflows: snapshot {name} version is not a stable literal release")
    return value

def config_paths():
    found = []
    for directory, names, filenames in os.walk(snapshot, followlinks=False):
        names[:] = [name for name in names if name != ".git" and not (Path(directory) / name).is_symlink()]
        for filename in filenames:
            if filename in {"mise.toml", ".mise.toml", "mise.local.toml", ".mise.local.toml"} or (filename.startswith(("mise.", ".mise.")) and filename.endswith(".toml")):
                found.append(Path(directory) / filename)
    return found

def load_config(path, label):
    if path.is_symlink() or not path.is_file():
        raise SystemExit(f"generated-workflows: snapshot {label} is not a regular file")
    try:
        with path.open("rb") as source:
            return tomllib.load(source)
    except (OSError, tomllib.TOMLDecodeError) as error:
        raise SystemExit(f"generated-workflows: snapshot {label} is invalid TOML") from error

def check_aliases(config):
    aliases = config.get("tool_alias", {})
    if type(aliases) is not dict:
        raise SystemExit("generated-workflows: snapshot tool_alias is not a table")
    protected = set(selected) | set(selected.values())
    for alias, target in aliases.items():
        if type(alias) is not str or type(target) is not str:
            raise SystemExit("generated-workflows: snapshot tool_alias contains a nonliteral entry")
        if alias in protected or target in protected:
            raise SystemExit("generated-workflows: snapshot aliases a selected tool")

configs = config_paths()
if snapshot / "mise.toml" not in configs:
    raise SystemExit("generated-workflows: snapshot mise.toml is missing")
versions = {}
for path in configs:
    config = load_config(path, "mise config")
    check_aliases(config)
    tools = config.get("tools", {})
    if type(tools) is not dict:
        raise SystemExit("generated-workflows: snapshot tools is not a table")
    for name, value in tools.items():
        if name not in selected:
            continue
        if name == "github:tailrocks/velnor-new":
            if type(value) is not dict or set(value) != {"version", "bin"} or value.get("bin") != selected[name]:
                raise SystemExit("generated-workflows: snapshot Velnor entry has unsupported options or executable")
            version = literal_version(value.get("version"), "Velnor")
            key = "velnor"
        elif name == "rust":
            if type(value) is not dict or set(value) - {"version", "profile", "components", "targets", "mr_boxington"}:
                raise SystemExit("generated-workflows: snapshot Rust entry has unsupported options")
            version = literal_version(value.get("version"), "Rust")
            if value.get("profile", "minimal") != "minimal" or value.get("components", ["clippy", "rustfmt"]) != ["clippy", "rustfmt"]:
                raise SystemExit("generated-workflows: snapshot Rust profile or components are unsupported")
            if type(value.get("mr_boxington", False)) is not bool:
                raise SystemExit("generated-workflows: snapshot Rust wrapper option is not a literal boolean")
            targets = value.get("targets", [])
            if type(targets) is not list or targets != supported_targets:
                raise SystemExit("generated-workflows: snapshot Rust targets are outside the reviewed support policy")
            key = name
        else:
            if type(value) is dict:
                if set(value) - {"version"}:
                    raise SystemExit(f"generated-workflows: snapshot {name} entry has unsupported options")
                value = value.get("version")
            version = literal_version(value, name)
            key = name
        if key in versions and versions[key] != version:
            raise SystemExit(f"generated-workflows: snapshot contains conflicting {key} pins")
        versions[key] = version

if set(versions) != {"velnor", "rust", "jactionlint", "shellcheck"}:
    raise SystemExit("generated-workflows: snapshot is missing a selected tool pin")
policy = supported.get(versions["velnor"])
if policy is None or any(versions[name] != policy[name] for name in policy):
    raise SystemExit("generated-workflows: snapshot tool tuple is outside the reviewed support policy")

toolchains = []
for directory, names, filenames in os.walk(snapshot, followlinks=False):
    names[:] = [name for name in names if name != ".git" and not (Path(directory) / name).is_symlink()]
    if "rust-toolchain" in filenames:
        raise SystemExit("generated-workflows: legacy rust-toolchain file is unsupported")
    if "rust-toolchain.toml" in filenames:
        toolchains.append(Path(directory) / "rust-toolchain.toml")
if not toolchains:
    raise SystemExit("generated-workflows: snapshot rust-toolchain.toml is missing")
for path in toolchains:
    config = load_config(path, "rust-toolchain.toml")
    toolchain = config.get("toolchain")
    if type(toolchain) is not dict or set(toolchain) - {"channel", "profile", "components", "targets"}:
        raise SystemExit("generated-workflows: snapshot Rust toolchain declaration is unsupported")
    if toolchain.get("channel") != versions["rust"] or toolchain.get("profile", "minimal") != "minimal":
        raise SystemExit("generated-workflows: snapshot Rust toolchain channel or profile is unsupported")
    if toolchain.get("components", ["clippy", "rustfmt"]) != ["clippy", "rustfmt"]:
        raise SystemExit("generated-workflows: snapshot Rust toolchain components are unsupported")
    targets = toolchain.get("targets", [])
    if type(targets) is not list or targets != supported_targets:
        raise SystemExit("generated-workflows: snapshot Rust toolchain targets are outside the reviewed support policy")

sys.stdout.write("\n".join((versions["velnor"], versions["rust"], versions["jactionlint"], versions["shellcheck"])) + "\n")
PY
}

validate_cargo_inputs() {
  "$python_command" -I -S - "$1" "$2" "$3" "$4" <<'PY'
import os
import re
import sys
import tomllib
from pathlib import Path, PurePosixPath

snapshot = Path(sys.argv[1]).resolve(strict=True)
cargo_home = Path(sys.argv[2]).resolve(strict=True)
trusted_lock_sha256 = sys.argv[3]
trusted_manifests_sha256 = sys.argv[4]

def reject(message):
    raise SystemExit(f"generated-workflows: {message}")

def load_toml(path, label):
    if path.is_symlink() or not path.is_file():
        reject(f"{label} is not a regular file")
    try:
        with path.open("rb") as source:
            return tomllib.load(source)
    except (OSError, tomllib.TOMLDecodeError) as error:
        raise SystemExit(f"generated-workflows: {label} is invalid TOML") from error

def validate_cargo_config(path):
    config = load_toml(path, "Cargo config")
    if set(config) - {"term"}:
        reject("Cargo config contains unsupported settings")
    term = config.get("term", {})
    if type(term) is not dict or set(term) - {"color", "verbose"}:
        reject("Cargo config contains unsupported term settings")
    if "color" in term and (type(term["color"]) is not str or term["color"] not in {"auto", "always", "never"}):
        reject("Cargo config color is not a supported literal")
    if "verbose" in term and type(term["verbose"]) is not bool:
        reject("Cargo config verbose is not a literal boolean")

def effective_config_paths(directory):
    current = directory
    while True:
        for name in ("config", "config.toml"):
            path = current / ".cargo" / name
            if path.exists() or path.is_symlink():
                yield path
        if current.parent == current:
            break
        current = current.parent

def path_is_inside(base, raw_path, label):
    if type(raw_path) is not str or not raw_path or "\\" in raw_path or re.match(r"^[A-Za-z]:", raw_path):
        reject(f"{label} is not a supported relative path")
    relative = PurePosixPath(raw_path)
    if relative.is_absolute():
        reject(f"{label} escapes the snapshot")
    resolved = Path(os.path.realpath(base.joinpath(*relative.parts)))
    try:
        resolved.relative_to(snapshot)
    except ValueError:
        reject(f"{label} escapes the snapshot")

def inspect_paths(value, base, location):
    if isinstance(value, dict):
        for key, item in value.items():
            label = f"{location}.{key}"
            if key in {"path", "readme", "license-file"} and isinstance(item, str):
                path_is_inside(base, item, label)
            elif key == "build" and isinstance(item, str):
                path_is_inside(base, item, label)
            elif key in {"include", "exclude"} and isinstance(item, list):
                for entry in item:
                    path_is_inside(base, entry, label)
            inspect_paths(item, base, label)
    elif isinstance(value, list):
        for index, item in enumerate(value):
            inspect_paths(item, base, f"{location}[{index}]")

config_files = set()
manifests = []
cargo_locks = []
for directory, names, filenames in os.walk(snapshot, followlinks=False):
    current = Path(directory)
    for name in tuple(names):
        child = current / name
        if name == ".git":
            names.remove(name)
        elif child.is_symlink() and child.is_dir():
            reject("Cargo inputs are nested beneath a symlink directory")
    for filename in filenames:
        path = current / filename
        if path.parent.name == ".cargo" and filename in {"config", "config.toml"}:
            config_files.add(path)
        if filename == "Cargo.toml":
            manifests.append(path)
        if filename == "Cargo.lock":
            cargo_locks.append(path)
for manifest in manifests:
    for path in effective_config_paths(manifest.parent):
        config_files.add(path)
for path in effective_config_paths(snapshot):
    config_files.add(path)
for path in sorted(config_files):
    validate_cargo_config(path)
for name in ("config", "config.toml"):
    path = cargo_home / name
    if path.exists() or path.is_symlink():
        validate_cargo_config(path)

if not manifests:
    reject("snapshot contains no Cargo manifests")
if trusted_manifests_sha256 != "-":
    manifest_digest = __import__("hashlib").sha256()
    for path in sorted(manifests, key=lambda item: item.relative_to(snapshot).as_posix()):
        relative = path.relative_to(snapshot).as_posix()
        manifest_digest.update(relative.encode("utf-8"))
        manifest_digest.update(b"\0")
        manifest_digest.update(__import__("hashlib").sha256(path.read_bytes()).hexdigest().encode("ascii"))
        manifest_digest.update(b"\n")
    if manifest_digest.hexdigest() != trusted_manifests_sha256:
        reject("Cargo manifests are outside the reviewed dependency support policy")
lock_path = snapshot / "Cargo.lock"
if lock_path.is_symlink() or not lock_path.is_file():
    reject("snapshot Cargo.lock is not a regular file")
if lock_path not in cargo_locks:
    reject("snapshot Cargo.lock was not discovered as a regular input")
for path in cargo_locks:
    if path.is_symlink() or not path.is_file():
        reject("nested Cargo.lock is not a regular file")
    if __import__("hashlib").sha256(path.read_bytes()).hexdigest() != trusted_lock_sha256:
        reject("Cargo.lock is outside the reviewed dependency support policy")
for manifest in manifests:
    if manifest.is_symlink():
        reject("Cargo manifest is symlinked")
    data = load_toml(manifest, "Cargo manifest")
    base = manifest.parent
    inspect_paths(data, base, str(manifest.relative_to(snapshot)))
    workspace = data.get("workspace", {})
    if type(workspace) is dict:
        for key in ("members", "default-members", "exclude"):
            values = workspace.get(key, [])
            if type(values) is not list:
                reject("Cargo workspace paths are not an array")
            for value in values:
                path_is_inside(base, value, f"{manifest.relative_to(snapshot)} workspace {key}")
PY
}

validate_trusted_lock_sources() {
  "$python_command" -I -S - "$1" "$trusted_lock_sha256" <<'PY'
import hashlib
import re
import sys
import tomllib
from pathlib import Path

path = Path(sys.argv[1])
expected_hash = sys.argv[2]
expected_git = {
    "git+https://github.com/tailrocks/termpane?rev=46cc0977d8ee95d5284f75080d507ca2d116c007#46cc0977d8ee95d5284f75080d507ca2d116c007": "termpane",
    "git+https://github.com/tailrocks/tuiscotti?rev=d8d54155947f6303a41dfd6079b53c0abc16f29a#d8d54155947f6303a41dfd6079b53c0abc16f29a": "tuiscotti",
}
expected_local = {"junie-tui", "jackin-console"}
registry_source = "registry+https://github.com/rust-lang/crates.io-index"

def reject(message):
    raise SystemExit(f"generated-workflows: trusted seed lock {message}")

try:
    data = path.read_bytes()
    lock = tomllib.loads(data.decode("utf-8"))
except (OSError, UnicodeError, tomllib.TOMLDecodeError):
    reject("is unreadable or invalid TOML")
if hashlib.sha256(data).hexdigest() != expected_hash:
    reject("does not match the reviewed lock identity")
if lock.get("version") != 4 or set(lock) - {"version", "package", "metadata"}:
    reject("has an unsupported format")
packages = lock.get("package")
if type(packages) is not list:
    reject("has no package inventory")
registry_count = 0
git_packages = {}
local_names = set()
identities = set()
for package in packages:
    if type(package) is not dict or set(package) - {"name", "version", "source", "checksum", "dependencies"}:
        reject("contains an unsupported package entry")
    name = package.get("name")
    version = package.get("version")
    if type(name) is not str or not name or type(version) is not str or not version:
        reject("contains a package without a literal identity")
    source = package.get("source")
    identity = (name, version, source)
    if identity in identities:
        reject("contains a duplicate package identity")
    identities.add(identity)
    if source == registry_source:
        checksum = package.get("checksum")
        if type(checksum) is not str or re.fullmatch(r"[0-9a-f]{64}", checksum) is None:
            reject("contains a registry package without a valid SHA-256 checksum")
        registry_count += 1
    elif source in expected_git:
        if "checksum" in package or name != expected_git[source]:
            reject("contains an unexpected Git package identity")
        git_packages[source] = git_packages.get(source, 0) + 1
    elif source is None:
        if "checksum" in package or name not in expected_local:
            reject("contains an unsupported local package")
        local_names.add(name)
    else:
        reject("contains an unsupported registry or Git source")
if registry_count != 291 or git_packages != {source: 1 for source in expected_git} or local_names != expected_local or len(packages) != 295:
    reject("does not match the reviewed crates.io, Git, and workspace source inventory")
PY
}

prepare_trusted_seed() {
  local seed=$1
  local reader=$2
  local entries=$3
  local projected_entries="$temporary_root/trusted-seed-projected-tree-entries"
  local commit_type
  local tree
  local tree_type
  local seed_selection
  local expected_selection=$4
  local cargo_home=$5

  if [[ "$object_format" != sha1 ]]; then
    printf 'generated-workflows: trusted seed requires the reviewed SHA-1 Git object format\n' >&2
    return 1
  fi
  initialize_trusted_reader "$reader" || return $?
  commit_type=$(git_in_trusted_reader "$reader" cat-file -t "$trusted_seed_commit" 2>/dev/null) || {
    printf 'generated-workflows: reviewed trusted seed commit object is unavailable\n' >&2
    return 1
  }
  if [[ "$commit_type" != commit ]]; then
    printf 'generated-workflows: reviewed trusted seed object is not a commit\n' >&2
    return 1
  fi
  tree=$(git_in_trusted_reader "$reader" rev-parse --verify "$trusted_seed_commit^{tree}") || {
    printf 'generated-workflows: reviewed trusted seed commit tree is unavailable\n' >&2
    return 1
  }
  tree_type=$(git_in_trusted_reader "$reader" cat-file -t "$tree" 2>/dev/null) || {
    printf 'generated-workflows: reviewed trusted seed tree object is unavailable\n' >&2
    return 1
  }
  if [[ "$tree_type" != tree ]]; then
    printf 'generated-workflows: reviewed trusted seed object is not a tree\n' >&2
    return 1
  fi
  bounded_tree_listing trusted "$reader" "$tree" "$entries" || {
    printf 'generated-workflows: cannot read the complete reviewed trusted seed tree\n' >&2
    return 1
  }
  validate_trusted_tree_entries "$entries" || return $?
  project_trusted_tree_entries "$entries" "$projected_entries" || return $?
  preflight_tree_blob_sizes trusted "$reader" "$projected_entries" true || return $?
  hydrate_blobs trusted "$reader" "$projected_entries" trusted-seed "trusted seed" || return $?
  preflight_tree_blob_sizes trusted "$reader" "$projected_entries" false || return $?
  mkdir -p "$seed" || return $?
  materialize_tree "$seed" "$projected_entries" "$reader" || {
    printf 'generated-workflows: cannot materialize the reviewed trusted seed\n' >&2
    return 1
  }
  validate_snapshot_symlinks "$seed" || return $?
  validate_cargo_inputs "$seed" "$cargo_home" "$trusted_lock_sha256" - || return $?
  validate_trusted_lock_sources "$seed/Cargo.lock" || return $?
  seed_selection=$(read_snapshot_tool_selection "$seed") || {
    printf 'generated-workflows: trusted seed toolchain inputs are unsupported\n' >&2
    return 1
  }
  if [[ "$seed_selection" != "$expected_selection" ]]; then
    printf 'generated-workflows: trusted seed toolchain does not match the reviewed snapshot pins\n' >&2
    return 1
  fi
  find "$seed" -type f -exec chmod a-w {} + || return $?
  find "$seed" -type d -exec chmod a-w {} + || return $?
}

trusted_seed_inventory() {
  "$python_command" -I -S - "$1" <<'PY'
import hashlib
import os
import stat
import sys
from pathlib import Path

root = Path(sys.argv[1])
digest = hashlib.sha256()
entries = []
for directory, names, filenames in os.walk(root, followlinks=False):
    current = Path(directory)
    for name in names + filenames:
        path = current / name
        entries.append(path)
for path in sorted(entries, key=lambda item: os.fsencode(item.relative_to(root))):
    relative = os.fsencode(path.relative_to(root))
    info = path.lstat()
    mode = stat.S_IMODE(info.st_mode)
    if stat.S_ISLNK(info.st_mode):
        kind = b"link"
        content = os.fsencode(os.readlink(path))
    elif stat.S_ISREG(info.st_mode):
        kind = b"file"
        content = path.read_bytes()
    elif stat.S_ISDIR(info.st_mode):
        kind = b"directory"
        content = b""
    else:
        raise SystemExit("generated-workflows: trusted seed contains a special file")
    for field in (relative, kind, str(mode).encode("ascii"), content):
        digest.update(len(field).to_bytes(8, "big"))
        digest.update(field)
print(digest.hexdigest())
PY
}

validate_private_lock() {
  "$python_command" -I -S - "$1" "$2" "$3" "$4" "$5" "$6" "$7" <<'PY'
import re
import sys
import tomllib

path, velnor_version, rust_version, jactionlint_version, shellcheck_version, platform, rust_host = sys.argv[1:]
platforms = {
    "macos-arm64": {
        "velnor_suffix": "aarch64-apple-darwin",
        "velnor_asset": "712371787",
        "jactionlint_suffix": "darwin_arm64",
        "shellcheck_suffix": "darwin.aarch64",
        "shellcheck_asset": "279056932",
    },
    "linux-x64": {
        "velnor_suffix": "x86_64-unknown-linux-gnu",
        "velnor_asset": "712371796",
        "jactionlint_suffix": "linux_amd64",
        "shellcheck_suffix": "linux.x86_64",
        "shellcheck_asset": "279056942",
    },
}
identity = platforms.get(platform)
if identity is None:
    raise SystemExit("generated-workflows: current platform has no reviewed tool artifacts")

try:
    with open(path, "rb") as lock_file:
        lock = tomllib.load(lock_file)
except (OSError, tomllib.TOMLDecodeError) as error:
    raise SystemExit("generated-workflows: private tool lock is invalid TOML") from error

tools = lock.get("tools")
expected_keys = {"github:tailrocks/velnor-new", "rust", "jactionlint", "shellcheck"}
if type(tools) is not dict or set(tools) != expected_keys:
    raise SystemExit("generated-workflows: private lock contains missing or unexpected tools")

def entry_for(name, version, backend):
    entries = tools.get(name)
    if type(entries) is not list or len(entries) != 1:
        raise SystemExit(f"generated-workflows: private lock does not contain one {name} release")
    entry = entries[0]
    if entry.get("version") != version or entry.get("backend") != backend:
        raise SystemExit(f"generated-workflows: private lock selected an unsupported {name} backend or version")
    return entry

def platform_entry(entry):
    result = entry.get(f"platforms.{platform}")
    if type(result) is not dict:
        raise SystemExit("generated-workflows: private tool lock has no current platform artifact")
    if re.fullmatch(r"sha256:[0-9a-f]{64}", str(result.get("checksum", ""))) is None:
        raise SystemExit("generated-workflows: private tool lock has no SHA-256 artifact identity")
    return result

velnor = entry_for("github:tailrocks/velnor-new", velnor_version, "github:tailrocks/velnor-new")
velnor_platform = platform_entry(velnor)
velnor_url = f"https://github.com/tailrocks/velnor-new/releases/download/v{velnor_version}/velnor-actions-{velnor_version}-{identity['velnor_suffix']}"
velnor_api = f"https://api.github.com/repos/tailrocks/velnor-new/releases/assets/{identity['velnor_asset']}"
if velnor_platform.get("url") != velnor_url or velnor_platform.get("url_api") != velnor_api or velnor_platform.get("provenance") != "github-attestations":
    raise SystemExit("generated-workflows: private Velnor lock does not match the authenticated official release")

jactionlint = entry_for("jactionlint", jactionlint_version, "packslip:github.com/jdx/jactionlint")
jactionlint_platform = platform_entry(jactionlint)
jactionlint_url = f"https://github.com/jdx/jactionlint/releases/download/v{jactionlint_version}/jactionlint_{jactionlint_version}_{identity['jactionlint_suffix']}.tar.gz"
if jactionlint_platform.get("url") != jactionlint_url or jactionlint_platform.get("signer") != "sigstore-oidc:https://github.com/jdx/jactionlint/.github/workflows/release.yaml" or jactionlint_platform.get("repository_ids") != {"repository": "1404951382"}:
    raise SystemExit("generated-workflows: private jactionlint lock lacks the official release identity")

shellcheck = entry_for("shellcheck", shellcheck_version, "aqua:koalaman/shellcheck")
shellcheck_platform = platform_entry(shellcheck)
shellcheck_url = f"https://github.com/koalaman/shellcheck/releases/download/v{shellcheck_version}/shellcheck-v{shellcheck_version}.{identity['shellcheck_suffix']}.tar.xz"
shellcheck_api = f"https://api.github.com/repos/koalaman/shellcheck/releases/assets/{identity['shellcheck_asset']}"
if shellcheck_platform.get("url") != shellcheck_url or shellcheck_platform.get("url_api") != shellcheck_api:
    raise SystemExit("generated-workflows: private shellcheck lock does not match the official release asset")

rust = entry_for("rust", rust_version, "core:rust")
options = rust.get("options")
if type(options) is not dict or options != {"components": "clippy,rustfmt", "profile": "minimal", "targets": rust_host}:
    raise SystemExit("generated-workflows: private Rust lock has unsupported components or host targets")

print("authenticated private tool lock validated")
PY
}

validate_private_tool_path() {
  "$python_command" -I -S - "$1" "$2" "$3" "$4" "$5" "$6" "$7" "$8" "$9" <<'PY'
import os
import sys
from pathlib import Path

kind, state_value, data_value, cargo_value, rustup_value, root_value, executable_value, name, version = sys.argv[1:]

def inside(path, parent):
    try:
        return os.path.commonpath((str(path), str(parent))) == str(parent)
    except ValueError:
        return False

def shown(value):
    return ascii(str(value)[:240])

def fail(message, **values):
    details = " ".join(f"{key}={shown(value)}" for key, value in values.items())
    raise SystemExit("generated-workflows: " + message + (" " + details if details else ""))

def resolve(label, value):
    try:
        return Path(value).resolve(strict=True)
    except OSError as error:
        fail(f"cannot resolve {label}", path=value, errno=error.errno)

state = resolve("private resolver state", state_value)
data = resolve("private Mise data directory", data_value)
cargo = resolve("private Cargo home", cargo_value)
rustup = resolve("private Rustup home", rustup_value)

if not all(inside(path, state) for path in (data, cargo, rustup)):
    fail("private tool roots leave the resolver state", state=state, data=data, cargo=cargo, rustup=rustup)

root = Path(root_value)
if kind != "rust-sysroot" and not root.is_absolute():
    fail("Mise returned a relative tool root", reported_root=root)
root_lexical = Path(os.path.abspath(root))
if kind != "rust-sysroot":
    root_real = resolve("Mise-reported tool root", root)
    installs = resolve("private Mise install directory", data / "installs")
    if not inside(root_lexical, installs):
        fail("Mise-reported tool root is outside private installs", reported_root=root_lexical, private_installs=installs)

    def validate_install_root(canonical_install_root=True):
        if root_lexical.name != version or (canonical_install_root and (root_real.name != version or not inside(root_real, installs))):
            fail(
                f"{name} Mise root does not match its authenticated private pin",
                reported_root=root_lexical,
                canonical_root=root_real,
                private_installs=installs,
                expected_version=version,
            )

if kind == "rust-root":
    validate_install_root(False)
    cargo_bin = resolve("private Cargo proxy directory", cargo / "bin")
    if root_real != cargo_bin or not inside(cargo_bin, cargo):
        fail("Rust Mise root does not resolve to the private Cargo proxy directory", reported_root=root_lexical, canonical_root=root_real, cargo_bin=cargo_bin)
elif kind == "tool-root":
    validate_install_root()
elif kind == "rust-executable":
    validate_install_root(False)
    executable = Path(executable_value)
    cargo_bin = resolve("private Cargo proxy directory", cargo / "bin")
    if not executable.is_absolute() or executable.name != name:
        fail(f"Mise selected an unexpected Rust {name} executable", executable=executable, reported_root=root_lexical)
    resolved = resolve(f"Mise-selected Rust {name} proxy", executable)
    if not resolved.is_file() or not os.access(resolved, os.X_OK) or not inside(resolved, cargo_bin):
        fail(f"Rust {name} executable leaves the private Cargo proxy directory", executable=executable, canonical_executable=resolved, cargo_bin=cargo_bin)
elif kind == "tool-executable":
    validate_install_root()
    executable = Path(executable_value)
    if not executable.is_absolute() or executable.name != name:
        fail(f"Mise selected an unexpected {name} executable path", executable=executable, reported_root=root_lexical)
    resolved = resolve(f"Mise-selected {name} executable", executable)
    if not resolved.is_file() or not os.access(resolved, os.X_OK) or not inside(resolved, root_real):
        fail(f"{name} executable leaves its authenticated private install", executable=executable, canonical_executable=resolved, authenticated_root=root_real)
elif kind == "rust-sysroot":
    actual = Path(executable_value)
    toolchains = resolve("private Rustup toolchains directory", rustup / "toolchains")
    expected = toolchains / f"{version}-{name}"
    expected_real = resolve("pinned private Rustup toolchain", expected)
    actual_real = resolve("rustc-reported sysroot", actual)
    expected_leaf = f"{version}-{name}"
    if actual_real.name != expected_leaf or not inside(actual_real, toolchains) or actual_real != expected_real or not inside(expected_real, state):
        fail("rustc sysroot is outside the pinned private Rustup toolchain", reported_sysroot=actual, canonical_sysroot=actual_real, private_toolchains=toolchains, expected_toolchain=expected_leaf)
else:
    fail("invalid private tool path validation mode", mode=kind)
PY
}

validate_trusted_git_source_trees() {
  local lock_path=$1
  local lock_role=$2
  local home_dir=$3
  local runtime_tmp=$4
  local safe_path=$5
  env -i \
    "HOME=$home_dir" \
    "PATH=$safe_path" \
    "TMPDIR=$runtime_tmp" \
    LANG=C \
    LC_ALL=C \
    "$python_command" -I -S - "$lock_path" "$curl_command" "$lock_role" <<'PY'
import json
import os
import re
import selectors
import subprocess
import sys
import tempfile
import time
from pathlib import Path
from urllib.parse import urljoin, urlsplit

lock_path, curl, lock_role = sys.argv[1:]

def fail(message):
    raise SystemExit(f"generated-workflows: {message}")

expected = {
    "git+https://github.com/tailrocks/termpane?rev=46cc0977d8ee95d5284f75080d507ca2d116c007#46cc0977d8ee95d5284f75080d507ca2d116c007": ("tailrocks", "termpane", "46cc0977d8ee95d5284f75080d507ca2d116c007"),
}
if lock_role == "trusted-seed":
    expected["git+https://github.com/tailrocks/tuiscotti?rev=d8d54155947f6303a41dfd6079b53c0abc16f29a#d8d54155947f6303a41dfd6079b53c0abc16f29a"] = ("tailrocks", "tuiscotti", "d8d54155947f6303a41dfd6079b53c0abc16f29a")
elif lock_role == "actual-snapshot":
    expected["git+https://github.com/tailrocks/tuiscotti?rev=92ef2d9ee082b35f68e066c5129fedbf07675a80#92ef2d9ee082b35f68e066c5129fedbf07675a80"] = ("tailrocks", "tuiscotti", "92ef2d9ee082b35f68e066c5129fedbf07675a80")
else:
    fail("Cargo lock has an unsupported reviewed source role")
redirect_statuses = {301, 302, 303, 307, 308}
maximum_redirects = 3
maximum_header_bytes = 65536
maximum_body_bytes = 8 * 1024 * 1024

try:
    import tomllib
    with open(lock_path, "rb") as source:
        lock = tomllib.load(source)
except (OSError, tomllib.TOMLDecodeError):
    fail("reviewed Cargo lock is unavailable for Git source qualification")
locked_sources = {package.get("source") for package in lock.get("package", []) if isinstance(package, dict) and isinstance(package.get("source"), str) and package["source"].startswith("git+")}
if locked_sources != set(expected):
    fail("reviewed Cargo lock Git source set does not match its allowlist")

def validate_url(url, expected_url):
    try:
        parsed = urlsplit(url)
        port = parsed.port
    except ValueError:
        fail("trusted Git source metadata URL is malformed")
    expected_parts = urlsplit(expected_url)
    if parsed.scheme != "https" or parsed.netloc != "api.github.com" or parsed.hostname != "api.github.com" or port not in (None, 443) or parsed.username is not None or parsed.password is not None or parsed.path != expected_parts.path or parsed.query != expected_parts.query or parsed.fragment or "\\" in url:
        fail("trusted Git source metadata URL is outside the official API path")
    if any(ord(character) < 32 or ord(character) == 127 for character in url):
        fail("trusted Git source metadata URL contains control characters")

def read_headers(path):
    raw = path.read_bytes()
    if not raw or len(raw) > maximum_header_bytes:
        fail("trusted Git source response headers have an invalid size")
    lines = raw.decode("iso-8859-1").replace("\r\n", "\n").split("\n")
    status_lines = [line for line in lines if line.startswith("HTTP/")]
    if len(status_lines) != 1:
        fail("trusted Git source response has ambiguous status headers")
    match = re.fullmatch(r"HTTP/[0-9]+(?:\.[0-9]+)?[ \t]+([1-5][0-9]{2})(?:[ \t].*)?", status_lines[0])
    if match is None:
        fail("trusted Git source response has malformed status headers")
    locations = []
    for line in lines[lines.index(status_lines[0]) + 1:]:
        if not line:
            continue
        if line[0] in " \t" or ":" not in line:
            fail("trusted Git source response has malformed headers")
        name, value = line.split(":", 1)
        if name.lower() == "location":
            location = value.strip(" \t")
            if not location or any(ord(character) < 32 or ord(character) == 127 for character in location):
                fail("trusted Git source redirect location is malformed")
            locations.append(location)
    if len(locations) > 1:
        fail("trusted Git source response has ambiguous redirect locations")
    return int(match.group(1)), locations

def terminate(process):
    if process.poll() is None:
        process.terminate()
        try:
            process.wait(timeout=1)
        except subprocess.TimeoutExpired:
            process.kill()
            process.wait(timeout=1)

def fetch(url):
    current_url = url
    seen = set()
    for redirect_count in range(maximum_redirects + 1):
        validate_url(current_url, url)
        if current_url in seen:
            fail("trusted Git source metadata redirect loop detected")
        seen.add(current_url)
        with tempfile.TemporaryDirectory(prefix="git-source-metadata-", dir=os.environ["TMPDIR"]) as temporary:
            header_path = Path(temporary) / "headers"
            command = [
                curl,
                "--disable",
                "--proto",
                "=https",
                "--silent",
                "--show-error",
                "--connect-timeout",
                "10",
                "--max-time",
                "30",
                "--max-filesize",
                str(maximum_body_bytes),
                "--dump-header",
                str(header_path),
                "--header",
                "Accept: application/vnd.github+json",
                "--header",
                "X-GitHub-Api-Version: 2022-11-28",
                current_url,
            ]
            environment = {name: os.environ[name] for name in ("HOME", "PATH", "TMPDIR", "LANG", "LC_ALL")}
            try:
                process = subprocess.Popen(command, env=environment, stdin=subprocess.DEVNULL, stdout=subprocess.PIPE, stderr=subprocess.DEVNULL)
            except OSError:
                fail("cannot start the trusted caller curl executable for Git source metadata")
            selector = selectors.DefaultSelector()
            selector.register(process.stdout, selectors.EVENT_READ)
            payload = bytearray()
            deadline = time.monotonic() + 32
            try:
                while True:
                    if header_path.exists() and header_path.stat().st_size > maximum_header_bytes:
                        terminate(process)
                        fail("trusted Git source response headers exceed the size limit")
                    remaining = deadline - time.monotonic()
                    if remaining <= 0:
                        terminate(process)
                        fail("trusted Git source metadata request exceeded the time limit")
                    ready = selector.select(min(remaining, 0.1))
                    if ready:
                        chunk = os.read(process.stdout.fileno(), min(65536, maximum_body_bytes + 1 - len(payload)))
                        if not chunk:
                            break
                        payload.extend(chunk)
                        if len(payload) > maximum_body_bytes:
                            terminate(process)
                            fail("trusted Git source response exceeds the size limit")
                try:
                    process.wait(timeout=max(0.1, deadline - time.monotonic()))
                except subprocess.TimeoutExpired:
                    terminate(process)
                    fail("trusted Git source metadata request exceeded the time limit")
            finally:
                selector.close()
                if process.poll() is None:
                    terminate(process)
            if process.returncode != 0:
                fail("trusted Git source metadata request failed")
            if not header_path.is_file() or header_path.stat().st_size > maximum_header_bytes:
                fail("trusted Git source response headers have an invalid size")
            status, locations = read_headers(header_path)
            if status in redirect_statuses:
                if not locations:
                    fail("trusted Git source redirect has no location")
                if redirect_count == maximum_redirects:
                    fail("trusted Git source exceeded the redirect limit")
                try:
                    target = urljoin(current_url, locations[0])
                except ValueError:
                    fail("trusted Git source redirect URL is malformed")
                validate_url(target, url)
                current_url = target
                continue
            if status != 200:
                fail("trusted Git source metadata returned an unexpected HTTP status")
            if not payload:
                fail("trusted Git source metadata response is empty")
            return bytes(payload)
    fail("trusted Git source exceeded the redirect limit")

def qualify_repo(source, identity):
    owner, repository, commit = identity
    commit_url = f"https://api.github.com/repos/{owner}/{repository}/git/commits/{commit}"
    try:
        commit_data = json.loads(fetch(commit_url))
    except (UnicodeError, json.JSONDecodeError):
        fail("trusted Git source commit metadata is malformed")
    if type(commit_data) is not dict or commit_data.get("sha") != commit:
        fail("trusted Git source commit identity does not match the lock")
    tree_data = commit_data.get("tree")
    tree_sha = tree_data.get("sha") if type(tree_data) is dict else None
    if type(tree_sha) is not str or re.fullmatch(r"[0-9a-f]{40}", tree_sha) is None:
        fail("trusted Git source commit has no valid tree identity")
    tree_url = f"https://api.github.com/repos/{owner}/{repository}/git/trees/{tree_sha}?recursive=1"
    try:
        tree = json.loads(fetch(tree_url))
    except (UnicodeError, json.JSONDecodeError):
        fail("trusted Git source tree metadata is malformed")
    if type(tree) is not dict or tree.get("sha") != tree_sha or tree.get("truncated") is not False or type(tree.get("tree")) is not list:
        fail("trusted Git source tree metadata is incomplete or ambiguous")
    seen = set()
    for entry in tree["tree"]:
        if type(entry) is not dict or type(entry.get("path")) is not str or type(entry.get("mode")) is not str or type(entry.get("type")) is not str or type(entry.get("sha")) is not str:
            fail("trusted Git source tree entry is malformed")
        path = entry["path"]
        components = path.split("/")
        if not path or path.startswith("/") or "\\" in path or any(component in {"", ".", "..", ".git"} for component in components) or any(ord(character) < 32 or ord(character) == 127 for character in path):
            fail("trusted Git source tree contains an unsafe path")
        if path in seen:
            fail("trusted Git source tree contains a duplicate path")
        seen.add(path)
        if path == ".gitmodules" or path.endswith("/.gitmodules"):
            fail("trusted Git source tree contains submodule metadata")
        if entry["mode"] == "160000" or entry["type"] == "commit":
            fail("trusted Git source tree contains a submodule")
        if (entry["mode"], entry["type"]) not in {("040000", "tree"), ("100644", "blob"), ("100755", "blob"), ("120000", "blob")} or re.fullmatch(r"[0-9a-f]{40}", entry["sha"]) is None:
            fail("trusted Git source tree contains an unsupported object entry")

for source in sorted(locked_sources):
    qualify_repo(source, expected[source])
PY
}

validate_cargo_metadata_output() {
  "$python_command" -I -S - "$1" "$2" "$3" "$4" "$5" <<'PY'
import hashlib
import json
import os
import sys
from pathlib import Path

metadata_path, root_value, cargo_home_value, require_git_sources, manifest_value = sys.argv[1:]
root = Path(root_value).resolve(strict=True)
cargo_home = Path(cargo_home_value).resolve(strict=True)
manifest_input = Path(manifest_value).resolve(strict=True)
termpane_source = "git+https://github.com/tailrocks/termpane?rev=46cc0977d8ee95d5284f75080d507ca2d116c007#46cc0977d8ee95d5284f75080d507ca2d116c007"
if require_git_sources == "true":
    # The trusted seed remains frozen at its independently reviewed identity.
    expected_lock_sha256 = "54035a5337e01cd95129b464ffbd0db45e4b0c989a699742efa1f0ab6329046d"
    tuiscotti_source = "git+https://github.com/tailrocks/tuiscotti?rev=d8d54155947f6303a41dfd6079b53c0abc16f29a#d8d54155947f6303a41dfd6079b53c0abc16f29a"
else:
    expected_lock_sha256 = "c3d0a169604e33af6f0288d4be3f8dbdb69c08db73c748d97f44e802142640bf"
    tuiscotti_source = "git+https://github.com/tailrocks/tuiscotti?rev=92ef2d9ee082b35f68e066c5129fedbf07675a80#92ef2d9ee082b35f68e066c5129fedbf07675a80"
expected_git = {termpane_source, tuiscotti_source}
registry_source = "registry+https://github.com/rust-lang/crates.io-index"

def reject(message):
    raise SystemExit(f"generated-workflows: {message}")

def inside(path, parent):
    try:
        return os.path.commonpath((str(path), str(parent))) == str(parent)
    except ValueError:
        return False

try:
    with open(metadata_path, "rb") as source:
        metadata = json.load(source)
except (OSError, UnicodeError, json.JSONDecodeError):
    reject("Cargo metadata output is missing or malformed")
if type(metadata) is not dict or type(metadata.get("packages")) is not list or type(metadata.get("workspace_members")) is not list or type(metadata.get("workspace_root")) is not str:
    reject("Cargo metadata output is incomplete")
try:
    workspace_root = Path(metadata["workspace_root"]).resolve(strict=True)
except OSError:
    reject("Cargo metadata workspace root is unavailable")
if not inside(workspace_root, root):
    reject("Cargo metadata discovered a workspace outside the qualified tree")
lock_path = workspace_root / "Cargo.lock"
if lock_path.is_symlink() or not lock_path.is_file() or hashlib.sha256(lock_path.read_bytes()).hexdigest() != expected_lock_sha256:
    reject("Cargo metadata workspace does not use the reviewed Cargo.lock")
packages = metadata["packages"]
if not packages:
    reject("Cargo metadata discovered no packages")
package_ids = set()
workspace_package_ids = set(metadata["workspace_members"])
package_manifests = set()
seen_git_sources = set()
seen_registry = False
for package in packages:
    if type(package) is not dict or type(package.get("id")) is not str or type(package.get("manifest_path")) is not str:
        reject("Cargo metadata contains an invalid package record")
    package_ids.add(package["id"])
    source = package.get("source")
    try:
        manifest = Path(package["manifest_path"]).resolve(strict=True)
    except OSError:
        reject("Cargo metadata package manifest is unavailable")
    package_manifests.add(manifest)
    if source is None:
        if not inside(manifest, root):
            reject("Cargo metadata local package leaves the qualified tree")
    elif source == registry_source:
        if not inside(manifest, cargo_home):
            reject("Cargo metadata registry package leaves the private Cargo home")
        seen_registry = True
    elif source in expected_git:
        if not inside(manifest, cargo_home):
            reject("Cargo metadata Git package leaves the private Cargo home")
        seen_git_sources.add(source)
    else:
        reject("Cargo metadata discovered an unsupported registry or Git source")
if not workspace_package_ids or not workspace_package_ids.issubset(package_ids):
    reject("Cargo metadata workspace membership is incomplete")
if manifest_input != workspace_root / "Cargo.toml" and manifest_input not in package_manifests:
    reject("Cargo metadata omitted a discovered manifest")
for package in packages:
    if package["id"] in workspace_package_ids:
        if package.get("source") is not None:
            reject("Cargo metadata workspace member is not local to the qualified tree")
        if not inside(Path(package["manifest_path"]).resolve(strict=True), workspace_root):
            reject("Cargo metadata workspace member leaves its workspace root")
if require_git_sources == "true" and (seen_git_sources != expected_git or not seen_registry):
    reject("trusted seed metadata omitted a reviewed Git or crates.io source")
PY
}

qualify_cargo_workspaces() {
  local root=$1
  local cargo_executable=$2
  local cargo_home=$3
  local home_dir=$4
  local rustup_home=$5
  local safe_path=$6
  local safe_git_config=$7
  local runtime_tmp=$8
  local rust_pin=$9
  local label=${10}
  local require_git_sources=${11}
  local manifest_list="$runtime_tmp/$label-cargo-manifests"
  local manifest
  local metadata_path
  local error_path
  local workspace_count=0
  local manifest_count=0
  local seen_manifest_list="$runtime_tmp/$label-qualified-manifests"
  local -a metadata_environment=(
    "HOME=$home_dir"
    "PATH=$safe_path"
    "TMPDIR=$runtime_tmp"
    "CARGO_HOME=$cargo_home"
    "RUSTUP_HOME=$rustup_home"
    "RUSTUP_TOOLCHAIN=$rust_pin"
    "CARGO_NET_OFFLINE=true"
    "CARGO_REGISTRIES_CRATES_IO_PROTOCOL=sparse"
    "GIT_CONFIG_GLOBAL=$safe_git_config"
    GIT_CONFIG_NOSYSTEM=1
    GIT_CONFIG_SYSTEM=/dev/null
    GIT_NO_REPLACE_OBJECTS=1
    GIT_NO_LAZY_FETCH=1
    GIT_OPTIONAL_LOCKS=0
    GIT_TERMINAL_PROMPT=0
    LANG=C
    LC_ALL=C
  )

  if ! find "$root" -type d -name .git -prune -o -type f -name Cargo.toml -print0 > "$manifest_list"; then
    printf 'generated-workflows: cannot enumerate %s Cargo manifests\n' "$label" >&2
    return 1
  fi
  : > "$seen_manifest_list" || return $?
  while IFS= read -r -d '' manifest || [[ -n "$manifest" ]]; do
    manifest_count=$((manifest_count + 1))
    metadata_path="$runtime_tmp/$label-metadata-$manifest_count.json"
    error_path="$runtime_tmp/$label-metadata-$manifest_count.log"
    if ! (cd "$root" && env -i "${metadata_environment[@]}" "$cargo_executable" metadata --locked --offline --all-features --format-version 1 --manifest-path "$manifest") > "$metadata_path" 2> "$error_path"; then
      printf 'generated-workflows: offline Cargo metadata failed for %s manifest %s\n' "$label" "${manifest#"$root"/}" >&2
      "$python_command" -I -S - "$error_path" <<'PY' >&2
from pathlib import Path
import sys

payload = Path(sys.argv[1]).read_bytes()[-8192:]
print(payload.decode("utf-8", "backslashreplace").encode("unicode_escape").decode("ascii"))
PY
      return 1
    fi
    validate_cargo_metadata_output "$metadata_path" "$root" "$cargo_home" "$require_git_sources" "$manifest" || return $?
    printf '%s\0' "$manifest" >> "$seen_manifest_list" || return $?
    workspace_count=$((workspace_count + 1))
  done < "$manifest_list"
  if [[ "$manifest_count" -eq 0 || "$workspace_count" -ne "$manifest_count" ]]; then
    printf 'generated-workflows: %s Cargo workspace discovery was incomplete\n' "$label" >&2
    return 1
  fi
}

fetch_trusted_seed_dependencies() {
  local seed=$1
  local snapshot=$2
  local cargo_executable=$3
  local cargo_home=$4
  local home_dir=$5
  local rustup_home=$6
  local safe_path=$7
  local safe_git_config=$8
  local runtime_tmp=$9
  local rust_pin=${10}
  local seed_inventory=${11}
  local cargo_log="$runtime_tmp/trusted-cargo-fetch.log"
  local snapshot_cargo_log="$runtime_tmp/snapshot-cargo-fetch.log"
  local -a cargo_environment=(
    "HOME=$home_dir"
    "PATH=$safe_path"
    "TMPDIR=$runtime_tmp"
    "CARGO_HOME=$cargo_home"
    "RUSTUP_HOME=$rustup_home"
    "RUSTUP_TOOLCHAIN=$rust_pin"
    CARGO_NET_OFFLINE=false
    CARGO_REGISTRIES_CRATES_IO_PROTOCOL=sparse
    GIT_CONFIG_NOSYSTEM=1
    GIT_CONFIG_SYSTEM=/dev/null
    GIT_NO_REPLACE_OBJECTS=1
    GIT_NO_LAZY_FETCH=1
    GIT_OPTIONAL_LOCKS=0
    GIT_TERMINAL_PROMPT=0
    LANG=C
    LC_ALL=C
  )

  validate_cargo_inputs "$seed" "$cargo_home" "$trusted_lock_sha256" - || return $?
  validate_trusted_lock_sources "$seed/Cargo.lock" || return $?
  if ! (cd "$seed" && env -i "${cargo_environment[@]}" "$cargo_executable" fetch --locked --manifest-path "$seed/Cargo.toml") > "$cargo_log" 2>&1; then
    printf 'generated-workflows: authenticated Cargo could not fetch the reviewed trusted seed\n' >&2
    "$python_command" -I -S - "$cargo_log" <<'PY' >&2
from pathlib import Path
import sys

payload = Path(sys.argv[1]).read_bytes()[-8192:]
print(payload.decode("utf-8", "backslashreplace").encode("unicode_escape").decode("ascii"))
PY
    return 1
  fi
  validate_cargo_inputs "$snapshot" "$cargo_home" "$snapshot_lock_sha256" "$snapshot_manifests_sha256" || return $?
  if ! (cd "$snapshot" && env -i "${cargo_environment[@]}" "$cargo_executable" fetch --locked --manifest-path "$snapshot/Cargo.toml") > "$snapshot_cargo_log" 2>&1; then
    printf 'generated-workflows: authenticated Cargo could not fetch the reviewed snapshot dependencies\n' >&2
    "$python_command" -I -S - "$snapshot_cargo_log" <<'PY' >&2
from pathlib import Path
import sys

payload = Path(sys.argv[1]).read_bytes()[-8192:]
print(payload.decode("utf-8", "backslashreplace").encode("unicode_escape").decode("ascii"))
PY
    return 1
  fi
  qualify_cargo_workspaces "$seed" "$cargo_executable" "$cargo_home" "$home_dir" "$rustup_home" "$safe_path" "$safe_git_config" "$runtime_tmp" "$rust_pin" trusted-seed true || return $?
  qualify_cargo_workspaces "$snapshot" "$cargo_executable" "$cargo_home" "$home_dir" "$rustup_home" "$safe_path" "$safe_git_config" "$runtime_tmp" "$rust_pin" actual-snapshot false || return $?
  local after_inventory
  after_inventory=$(trusted_seed_inventory "$seed") || return $?
  if [[ "$after_inventory" != "$seed_inventory" ]]; then
    printf 'generated-workflows: trusted seed changed during Cargo fetch or metadata qualification\n' >&2
    return 1
  fi
}

install_rustup_bootstrap() {
  local rust_host=$1
  local bootstrap="$downloads_dir/rustup-init-$rust_host"
  local -a bootstrap_environment=(
    "HOME=$home_dir"
    "PATH=$safe_path"
    "TMPDIR=$runtime_tmp"
    LANG=C
    LC_ALL=C
  )
  env -i "${bootstrap_environment[@]}" "$python_command" -I -S - "$rust_host" "$bootstrap" "$curl_command" "$safe_path" "$runtime_tmp" <<'PY' || return 1
import hashlib
import os
import re
import selectors
import subprocess
import sys
import tempfile
import time
from pathlib import Path
from urllib.parse import urljoin, urlsplit

host, destination, curl, safe_path, runtime_tmp = sys.argv[1:]
if host not in {"aarch64-apple-darwin", "x86_64-unknown-linux-gnu"}:
    raise SystemExit("generated-workflows: rustup bootstrap target is unsupported")
base_path = f"/rustup/dist/{host}"
bootstrap_path = base_path + "/rustup-init"
checksum_path = bootstrap_path + ".sha256"
redirect_statuses = {301, 302, 303, 307, 308}
maximum_redirects = 3
maximum_header_bytes = 65536

def validate_url(url, expected_path):
    try:
        parsed = urlsplit(url)
        port = parsed.port
    except ValueError:
        raise SystemExit("generated-workflows: rustup bootstrap redirect URL is malformed")
    if parsed.scheme != "https" or parsed.hostname != "static.rust-lang.org" or port not in (None, 443) or parsed.username is not None or parsed.password is not None or parsed.path != expected_path or "?" in url or "#" in url or "\\" in url:
        raise SystemExit("generated-workflows: rustup bootstrap URL is outside the approved official distribution path")
    if any(ord(character) < 32 or ord(character) == 127 for character in url):
        raise SystemExit("generated-workflows: rustup bootstrap redirect URL contains control characters")

def response_headers(path):
    raw = path.read_bytes()
    if not raw or len(raw) > maximum_header_bytes:
        raise SystemExit("generated-workflows: rustup bootstrap response headers have an invalid size")
    try:
        lines = raw.decode("iso-8859-1").replace("\r\n", "\n").split("\n")
    except UnicodeError:
        raise SystemExit("generated-workflows: rustup bootstrap response headers are malformed")
    status_lines = [line for line in lines if line.startswith("HTTP/")]
    if len(status_lines) != 1:
        raise SystemExit("generated-workflows: rustup bootstrap response has ambiguous status headers")
    match = re.fullmatch(r"HTTP/[0-9]+(?:\.[0-9]+)?[ \t]+([1-5][0-9]{2})(?:[ \t].*)?", status_lines[0])
    if match is None:
        raise SystemExit("generated-workflows: rustup bootstrap response has malformed status headers")
    locations = []
    for line in lines[lines.index(status_lines[0]) + 1:]:
        if not line:
            continue
        if line[0] in " \t" or ":" not in line:
            raise SystemExit("generated-workflows: rustup bootstrap response has malformed headers")
        name, value = line.split(":", 1)
        if name.lower() == "location":
            location = value.strip(" \t")
            if not location or any(ord(character) < 32 or ord(character) == 127 for character in location):
                raise SystemExit("generated-workflows: rustup bootstrap redirect location is malformed")
            locations.append(location)
    if len(locations) > 1:
        raise SystemExit("generated-workflows: rustup bootstrap response has ambiguous redirect locations")
    return int(match.group(1)), locations

def terminate(process):
    if process.poll() is None:
        process.terminate()
        try:
            process.wait(timeout=1)
        except subprocess.TimeoutExpired:
            process.kill()
            process.wait(timeout=1)

def fetch(initial_url, expected_path, maximum_bytes):
    current_url = initial_url
    seen = set()
    for redirect_count in range(maximum_redirects + 1):
        validate_url(current_url, expected_path)
        if current_url in seen:
            raise SystemExit("generated-workflows: rustup bootstrap redirect loop detected")
        seen.add(current_url)
        with tempfile.TemporaryDirectory(prefix="rustup-fetch-", dir=runtime_tmp) as temporary:
            header_path = Path(temporary) / "headers"
            command = [
                curl,
                "--disable",
                "--proto",
                "=https",
                "--silent",
                "--show-error",
                "--connect-timeout",
                "10",
                "--max-time",
                "30",
                "--max-filesize",
                str(maximum_bytes),
                "--dump-header",
                str(header_path),
                current_url,
            ]
            environment = {
                "HOME": os.environ["HOME"],
                "PATH": safe_path,
                "TMPDIR": runtime_tmp,
                "LANG": "C",
                "LC_ALL": "C",
            }
            try:
                process = subprocess.Popen(command, env=environment, stdin=subprocess.DEVNULL, stdout=subprocess.PIPE, stderr=subprocess.DEVNULL)
            except OSError:
                raise SystemExit("generated-workflows: cannot start the trusted caller curl executable")
            selector = selectors.DefaultSelector()
            selector.register(process.stdout, selectors.EVENT_READ)
            payload = bytearray()
            deadline = time.monotonic() + 32
            try:
                while True:
                    if header_path.exists() and header_path.stat().st_size > maximum_header_bytes:
                        terminate(process)
                        raise SystemExit("generated-workflows: rustup bootstrap response headers exceed the size limit")
                    remaining = deadline - time.monotonic()
                    if remaining <= 0:
                        terminate(process)
                        raise SystemExit("generated-workflows: rustup bootstrap request exceeded the time limit")
                    ready = selector.select(min(remaining, 0.1))
                    if ready:
                        chunk = os.read(process.stdout.fileno(), min(65536, maximum_bytes + 1 - len(payload)))
                        if not chunk:
                            break
                        payload.extend(chunk)
                        if len(payload) > maximum_bytes:
                            terminate(process)
                            raise SystemExit("generated-workflows: rustup bootstrap response exceeds the size limit")
                try:
                    process.wait(timeout=max(0.1, deadline - time.monotonic()))
                except subprocess.TimeoutExpired:
                    terminate(process)
                    raise SystemExit("generated-workflows: rustup bootstrap request exceeded the time limit")
            finally:
                selector.close()
                if process.poll() is None:
                    terminate(process)
            if process.returncode != 0:
                raise SystemExit("generated-workflows: cannot retrieve official Rustup bootstrap")
            if not header_path.is_file() or header_path.stat().st_size > maximum_header_bytes:
                raise SystemExit("generated-workflows: rustup bootstrap response headers have an invalid size")
            status, locations = response_headers(header_path)
            if status in redirect_statuses:
                if not locations:
                    raise SystemExit("generated-workflows: rustup bootstrap redirect has no location")
                if redirect_count == maximum_redirects:
                    raise SystemExit("generated-workflows: rustup bootstrap exceeded the redirect limit")
                try:
                    target = urljoin(current_url, locations[0])
                except ValueError:
                    raise SystemExit("generated-workflows: rustup bootstrap redirect URL is malformed")
                validate_url(target, expected_path)
                current_url = target
                continue
            if status != 200:
                raise SystemExit("generated-workflows: rustup bootstrap response has an unexpected status")
            if not payload:
                raise SystemExit("generated-workflows: rustup bootstrap response is empty")
            return bytes(payload)
    raise SystemExit("generated-workflows: rustup bootstrap exceeded the redirect limit")

checksum_url = "https://static.rust-lang.org" + checksum_path
binary_url = "https://static.rust-lang.org" + bootstrap_path
checksum_bytes = fetch(checksum_url, checksum_path, 4096)
try:
    checksum_text = checksum_bytes.decode("ascii").strip()
except UnicodeError:
    raise SystemExit("generated-workflows: official Rustup checksum manifest is malformed")
match = re.fullmatch(r"([0-9a-f]{64})[ \t]+\*?(?:\./)?rustup-init", checksum_text)
if match is None:
    raise SystemExit("generated-workflows: official Rustup checksum manifest is malformed")
binary = fetch(binary_url, bootstrap_path, 64 * 1024 * 1024)
if hashlib.sha256(binary).hexdigest() != match.group(1):
    raise SystemExit("generated-workflows: rustup bootstrap checksum does not match the official manifest")
destination_path = Path(destination)
destination_path.write_bytes(binary)
destination_path.chmod(0o700)
print(f"rustup-init sha256:{match.group(1)}")
PY
  env -i \
    "HOME=$home_dir" \
    "PATH=$safe_path" \
    "TMPDIR=$runtime_tmp" \
    "CARGO_HOME=$cargo_home" \
    "RUSTUP_HOME=$rustup_home" \
    "RUSTUP_DIST_SERVER=https://static.rust-lang.org" \
    "RUSTUP_UPDATE_ROOT=https://static.rust-lang.org/rustup" \
    LANG=C \
    LC_ALL=C \
    "$bootstrap" -y --no-modify-path --default-host "$rust_host" --default-toolchain none > "$downloads_dir/rustup-bootstrap.log" 2>&1 || {
      printf 'generated-workflows: authenticated Rustup bootstrap failed\n' >&2
      if [[ -f "$downloads_dir/rustup-bootstrap.log" ]]; then
        "$python_command" -I -S - "$downloads_dir/rustup-bootstrap.log" <<'PY' >&2
from pathlib import Path
import sys

payload = Path(sys.argv[1]).read_bytes()[-8192:]
print(payload.decode("utf-8", "backslashreplace").encode("unicode_escape").decode("ascii"))
PY
      fi
      return 1
    }
  if [[ ! -x "$cargo_home/bin/rustup" || ! -f "$rustup_home/settings.toml" ]]; then
    printf 'generated-workflows: authenticated Rustup bootstrap did not create private state\n' >&2
    return 1
  fi
}

run_generator() {
  local snapshot=$1
  local preview=$2
  local label=$3
  local tool_state_dir="$temporary_root/$label-tool-state"
  local state_dir="$tool_state_dir/mise-state"
  local config_dir="$tool_state_dir/mise-config"
  local data_dir="$tool_state_dir/mise-data"
  local cache_dir="$tool_state_dir/mise-cache"
  local downloads_dir="$tool_state_dir/mise-downloads"
  local home_dir="$tool_state_dir/home"
  local runtime_tmp="$tool_state_dir/runtime-tmp"
  local xdg_config="$tool_state_dir/xdg-config"
  local xdg_data="$tool_state_dir/xdg-data"
  local xdg_cache="$tool_state_dir/xdg-cache"
  local cargo_home="$tool_state_dir/cargo-home"
  local rustup_home="$tool_state_dir/rustup-home"
  local trusted_seed="$tool_state_dir/trusted-seed"
  local trusted_reader="$tool_state_dir/trusted-source-reader.git"
  local trusted_entries="$tool_state_dir/trusted-seed-tree-entries"
  local private_config="$config_dir/mise.toml"
  local private_lock="$config_dir/mise.lock"
  local safe_git_config="$config_dir/gitconfig"
  local safe_path
  local generator_path
  local platform
  local rust_host
  local selection
  local tool_pin
  local rust_pin
  local jactionlint_pin
  local shellcheck_pin
  local tool_bin
  local tool_dir
  local rust_dir
  local jactionlint_dir
  local shellcheck_dir
  local tool_executable
  local rust_cargo_executable
  local rustc_executable
  local jactionlint_executable
  local shellcheck_executable
  local rust_sysroot
  local rustc_version
  local cargo_version
  local jactionlint_version_output
  local jactionlint_reported_version
  local shellcheck_version_output
  local shellcheck_reported_version
  local trusted_seed_inventory_value
  local -a mise_environment
  local -a generator_environment

  mkdir -p "$state_dir" "$config_dir" "$data_dir" "$cache_dir" "$downloads_dir" "$home_dir" "$runtime_tmp" "$xdg_config" "$xdg_data" "$xdg_cache" "$cargo_home" "$rustup_home" || return $?
  : > "$config_dir/global.toml" || return $?
  : > "$config_dir/system.toml" || return $?
  : > "$safe_git_config" || return $?

  (
    if [[ ! -f "$snapshot/mise.toml" || -L "$snapshot/mise.toml" ]]; then
      printf 'generated-workflows: missing regular snapshot config mise.toml\n' >&2
      exit 1
    fi
    selection=$(read_snapshot_tool_selection "$snapshot") || {
      printf 'generated-workflows: cannot read reviewed snapshot tool data\n' >&2
      exit 1
    }
    tool_pin=$(sed -n '1p' <<< "$selection") || exit $?
    rust_pin=$(sed -n '2p' <<< "$selection") || exit $?
    jactionlint_pin=$(sed -n '3p' <<< "$selection") || exit $?
    shellcheck_pin=$(sed -n '4p' <<< "$selection") || exit $?
    tool_bin=velnor-actions
    case "$(uname -s):$(uname -m)" in
      Linux:x86_64|Linux:amd64) platform=linux-x64 ;;
      Darwin:arm64|Darwin:aarch64) platform=macos-arm64 ;;
      *)
        printf 'generated-workflows: unsupported host platform for authenticated tools\n' >&2
        exit 1
        ;;
    esac
    case "$platform" in
      macos-arm64) rust_host=aarch64-apple-darwin ;;
      linux-x64) rust_host=x86_64-unknown-linux-gnu ;;
    esac
    validate_cargo_inputs "$snapshot" "$cargo_home" "$snapshot_lock_sha256" "$snapshot_manifests_sha256" || exit $?
    prepare_trusted_seed "$trusted_seed" "$trusted_reader" "$trusted_entries" "$selection" "$cargo_home" || exit $?
    trusted_seed_inventory_value=$(trusted_seed_inventory "$trusted_seed") || exit $?

    cat > "$private_config" <<EOF || exit $?
[tools]
"github:tailrocks/velnor-new" = { version = "$tool_pin", bin = "$tool_bin", github_attestations = true }
jactionlint = "$jactionlint_pin"
shellcheck = "$shellcheck_pin"
rust = { version = "$rust_pin", profile = "minimal", components = ["clippy", "rustfmt"], targets = ["$rust_host"] }
EOF
    chmod 600 "$private_config" || exit $?

    safe_path="${mise_command%/*}:${git_command%/*}:$cargo_home/bin:/usr/bin:/bin:/usr/sbin:/sbin"
    mise_environment=(
      "HOME=$home_dir"
      "PATH=$safe_path"
      "TMPDIR=$runtime_tmp"
      "XDG_CONFIG_HOME=$xdg_config"
      "XDG_DATA_HOME=$xdg_data"
      "XDG_CACHE_HOME=$xdg_cache"
      "CARGO_HOME=$cargo_home"
      "RUSTUP_HOME=$rustup_home"
      "RUSTUP_DIST_SERVER=https://static.rust-lang.org"
      "RUSTUP_UPDATE_ROOT=https://static.rust-lang.org/rustup"
      "MISE_CARGO_HOME=$cargo_home"
      "MISE_RUSTUP_HOME=$rustup_home"
      "MISE_RUST_DEFAULT_HOST=$rust_host"
      "LANG=C"
      "LC_ALL=C"
      "GIT_CONFIG_GLOBAL=$safe_git_config"
      GIT_CONFIG_NOSYSTEM=1
      GIT_NO_REPLACE_OBJECTS=1
      GIT_OPTIONAL_LOCKS=0
      "MISE_CONFIG_DIR=$config_dir"
      "MISE_CONFIG_FILE=$private_config"
      "MISE_GLOBAL_CONFIG_FILE=$config_dir/global.toml"
      "MISE_SYSTEM_CONFIG_FILE=$config_dir/system.toml"
      "MISE_STATE_DIR=$state_dir"
      "MISE_DATA_DIR=$data_dir"
      "MISE_CACHE_DIR=$cache_dir"
      "MISE_DOWNLOADS_DIR=$downloads_dir"
      "MISE_ENABLE_TOOLS=github:tailrocks/velnor-new,rust,jactionlint,shellcheck"
      MISE_ENV=
      MISE_IDIOMATIC_VERSION_FILE_ENABLE_TOOLS=
      MISE_LOCKFILE=1
      MISE_LOCKED_VERIFY_PROVENANCE=1
      MISE_GITHUB_ATTESTATIONS=1
      MISE_GITHUB_GITHUB_ATTESTATIONS=1
      MISE_PARANOID=1
      MISE_PROVENANCE_API_FAILURES_FATAL=1
      MISE_NO_ENV=1
      MISE_NO_HOOKS=1
      MISE_AUTO_INSTALL=0
      MISE_OVERRIDE_CONFIG_FILENAMES=mise.toml
      "MISE_CEILING_PATHS=$temporary_root"
      "MISE_TRUSTED_CONFIG_PATHS=$private_config"
    )

    install_rustup_bootstrap "$rust_host" || exit $?

    env -i "${mise_environment[@]}" "$mise_command" --cd "$config_dir" lock --platform "$platform" </dev/null || {
      printf 'generated-workflows: cannot resolve authenticated private tools for Velnor %s\n' "$tool_pin" >&2
      exit 1
    }
    if [[ ! -f "$private_lock" || -L "$private_lock" ]]; then
      printf 'generated-workflows: authenticated private tool lock was not created\n' >&2
      exit 1
    fi
    validate_private_lock "$private_lock" "$tool_pin" "$rust_pin" "$jactionlint_pin" "$shellcheck_pin" "$platform" "$rust_host" || exit $?

    env -i "${mise_environment[@]}" "$mise_command" --locked --cd "$config_dir" install </dev/null || {
      printf 'generated-workflows: authenticated private tool installation failed\n' >&2
      exit 1
    }

    tool_dir=$(env -i "${mise_environment[@]}" "$mise_command" --cd "$config_dir" where github:tailrocks/velnor-new) || {
      printf 'generated-workflows: authenticated Velnor is not installed\n' >&2
      exit 1
    }
    if [[ "${tool_dir##*/}" != "$tool_pin" ]]; then
      printf 'generated-workflows: installed Velnor does not match snapshot pin %s\n' "$tool_pin" >&2
      exit 1
    fi
    validate_private_tool_path tool-root "$tool_state_dir" "$data_dir" "$cargo_home" "$rustup_home" "$tool_dir" '' github:tailrocks/velnor-new "$tool_pin" || exit $?
    rust_dir=$(env -i "${mise_environment[@]}" "$mise_command" --cd "$config_dir" where rust) || {
      printf 'generated-workflows: authenticated Rust toolchain is not installed\n' >&2
      exit 1
    }
    validate_private_tool_path rust-root "$tool_state_dir" "$data_dir" "$cargo_home" "$rustup_home" "$rust_dir" '' rust "$rust_pin" || exit $?
    jactionlint_dir=$(env -i "${mise_environment[@]}" "$mise_command" --cd "$config_dir" where jactionlint) || {
      printf 'generated-workflows: authenticated jactionlint is not installed\n' >&2
      exit 1
    }
    if [[ "${jactionlint_dir##*/}" != "$jactionlint_pin" ]]; then
      printf 'generated-workflows: installed jactionlint does not match reviewed pin %s\n' "$jactionlint_pin" >&2
      exit 1
    fi
    validate_private_tool_path tool-root "$tool_state_dir" "$data_dir" "$cargo_home" "$rustup_home" "$jactionlint_dir" '' jactionlint "$jactionlint_pin" || exit $?
    shellcheck_dir=$(env -i "${mise_environment[@]}" "$mise_command" --cd "$config_dir" where shellcheck) || {
      printf 'generated-workflows: authenticated shellcheck is not installed\n' >&2
      exit 1
    }
    if [[ "${shellcheck_dir##*/}" != "$shellcheck_pin" ]]; then
      printf 'generated-workflows: installed shellcheck does not match reviewed pin %s\n' "$shellcheck_pin" >&2
      exit 1
    fi
    validate_private_tool_path tool-root "$tool_state_dir" "$data_dir" "$cargo_home" "$rustup_home" "$shellcheck_dir" '' shellcheck "$shellcheck_pin" || exit $?
    tool_executable=$(env -i "${mise_environment[@]}" "$mise_command" --cd "$config_dir" which "$tool_bin") || {
      printf 'generated-workflows: authenticated Velnor executable did not resolve through Mise\n' >&2
      exit 1
    }
    validate_private_tool_path tool-executable "$tool_state_dir" "$data_dir" "$cargo_home" "$rustup_home" "$tool_dir" "$tool_executable" "$tool_bin" "$tool_pin" || exit $?
    rust_cargo_executable=$(env -i "${mise_environment[@]}" "$mise_command" --cd "$config_dir" which cargo) || {
      printf 'generated-workflows: pinned Rust cargo executable did not resolve through Mise\n' >&2
      exit 1
    }
    validate_private_tool_path rust-executable "$tool_state_dir" "$data_dir" "$cargo_home" "$rustup_home" "$rust_dir" "$rust_cargo_executable" cargo "$rust_pin" || exit $?
    rustc_executable=$(env -i "${mise_environment[@]}" "$mise_command" --cd "$config_dir" which rustc) || {
      printf 'generated-workflows: pinned Rust rustc executable did not resolve through Mise\n' >&2
      exit 1
    }
    validate_private_tool_path rust-executable "$tool_state_dir" "$data_dir" "$cargo_home" "$rustup_home" "$rust_dir" "$rustc_executable" rustc "$rust_pin" || exit $?
    jactionlint_executable=$(env -i "${mise_environment[@]}" "$mise_command" --cd "$config_dir" which jactionlint) || {
      printf 'generated-workflows: pinned jactionlint executable did not resolve through Mise\n' >&2
      exit 1
    }
    validate_private_tool_path tool-executable "$tool_state_dir" "$data_dir" "$cargo_home" "$rustup_home" "$jactionlint_dir" "$jactionlint_executable" jactionlint "$jactionlint_pin" || exit $?
    shellcheck_executable=$(env -i "${mise_environment[@]}" "$mise_command" --cd "$config_dir" which shellcheck) || {
      printf 'generated-workflows: pinned shellcheck executable did not resolve through Mise\n' >&2
      exit 1
    }
    validate_private_tool_path tool-executable "$tool_state_dir" "$data_dir" "$cargo_home" "$rustup_home" "$shellcheck_dir" "$shellcheck_executable" shellcheck "$shellcheck_pin" || exit $?
    cd "$snapshot" || exit $?
    rustc_version=$(env -i "${mise_environment[@]}" "$rustc_executable" --version) || {
      printf 'generated-workflows: pinned Rust rustc failed at %s\n' "$rustc_executable" >&2
      exit 1
    }
    case "$rustc_version" in
      "rustc $rust_pin"|"rustc $rust_pin "*) ;;
      *)
        printf 'generated-workflows: rustc at %s reported an unexpected version\n' "$rustc_executable" >&2
        exit 1
        ;;
    esac
    cargo_version=$(env -i "${mise_environment[@]}" "$rust_cargo_executable" --version) || {
      printf 'generated-workflows: pinned Rust cargo failed at %s\n' "$rust_cargo_executable" >&2
      exit 1
    }
    case "$cargo_version" in
      "cargo $rust_pin"|"cargo $rust_pin "*) ;;
      *)
        printf 'generated-workflows: cargo at %s reported an unexpected version\n' "$rust_cargo_executable" >&2
        exit 1
        ;;
    esac
    rust_sysroot=$(env -i "${mise_environment[@]}" "$rustc_executable" --print sysroot) || {
      printf 'generated-workflows: pinned Rust rustc could not report its sysroot at %s\n' "$rustc_executable" >&2
      exit 1
    }
    validate_private_tool_path rust-sysroot "$tool_state_dir" "$data_dir" "$cargo_home" "$rustup_home" "$rustup_home/toolchains/$rust_pin-$rust_host" "$rust_sysroot" "$rust_host" "$rust_pin" || exit $?
    jactionlint_version_output=$(env -i "${mise_environment[@]}" "$jactionlint_executable" --version) || {
      printf 'generated-workflows: pinned jactionlint failed at %s\n' "$jactionlint_executable" >&2
      exit 1
    }
    jactionlint_reported_version=${jactionlint_version_output%%$'\n'*}
    if [[ "$jactionlint_reported_version" != "$jactionlint_pin" ]]; then
      printf 'generated-workflows: jactionlint at %s reported an unexpected version\n' "$jactionlint_executable" >&2
      exit 1
    fi
    shellcheck_version_output=$(env -i "${mise_environment[@]}" "$shellcheck_executable" --version) || {
      printf 'generated-workflows: pinned shellcheck failed at %s\n' "$shellcheck_executable" >&2
      exit 1
    }
    shellcheck_reported_version=$(sed -n 's/^version: //p' <<< "$shellcheck_version_output")
    if [[ "$shellcheck_reported_version" != "$shellcheck_pin" ]]; then
      printf 'generated-workflows: shellcheck at %s reported an unexpected version\n' "$shellcheck_executable" >&2
      exit 1
    fi

    validate_trusted_git_source_trees "$trusted_seed/Cargo.lock" trusted-seed "$home_dir" "$runtime_tmp" "$safe_path" || exit $?
    validate_trusted_git_source_trees "$snapshot/Cargo.lock" actual-snapshot "$home_dir" "$runtime_tmp" "$safe_path" || exit $?
    fetch_trusted_seed_dependencies "$trusted_seed" "$snapshot" "$rust_cargo_executable" "$cargo_home" "$home_dir" "$rustup_home" "$safe_path" "$safe_git_config" "$runtime_tmp" "$rust_pin" "$trusted_seed_inventory_value" || exit $?

    generator_path="${tool_executable%/*}:${rust_cargo_executable%/*}:${jactionlint_executable%/*}:${shellcheck_executable%/*}:$safe_path"
    generator_environment=(
      "${mise_environment[@]}"
      "PATH=$generator_path"
      CARGO_NET_OFFLINE=true
      CARGO_REGISTRIES_CRATES_IO_PROTOCOL=sparse
      "RUSTUP_TOOLCHAIN=$rust_pin"
    )
    env -i "${generator_environment[@]}" "$tool_executable" generate --output-dir "$preview" </dev/null || exit $?
    validate_snapshot_symlinks "$snapshot" || exit $?
    validate_snapshot_symlinks "$preview" || exit $?

    local comparison_status
    if git -C "$snapshot" diff --no-index --quiet --no-ext-diff --no-textconv -- "$snapshot/.github" "$preview/.github"; then
      exit 0
    else
      comparison_status=$?
    fi

    if [[ "$comparison_status" -eq 1 ]]; then
      printf 'generated-workflows: drift in %s snapshot\n' "$label" >&2
      exit 1
    fi

    exit "$comparison_status"
  )
}

check_tree() {
  local tree=$1
  local label=$2
  local snapshot="$temporary_root/$label"
  local preview="$temporary_root/$label-preview"
  local entries="$temporary_root/$label-tree-entries"

  initialize_snapshot_reader || return $?
  mkdir -p "$snapshot" "$preview" || return $?
  git_in_snapshot_context init --object-format="$object_format" --quiet "$snapshot" || return $?
  mkdir -p "$snapshot/.git/objects/info" || return $?
  printf '%s\n%s\n%s\n' "$index_tree_objects" "$verified_blob_objects" "$objects" > "$snapshot/.git/objects/info/alternates" || return $?
  while IFS= read -r alternate || [[ -n "$alternate" ]]; do
    if [[ "$alternate" != /* ]]; then
      printf 'generated-workflows: source alternate object directory is not absolute\n' >&2
      return 1
    fi
    printf '%s\n' "$alternate" >> "$snapshot/.git/objects/info/alternates" || return $?
  done < "$source_alternate_dirs"
  bounded_tree_listing snapshot '' "$tree" "$entries" || return $?
  validate_snapshot_tree_entries "$entries" || return $?
  preflight_tree_blob_sizes snapshot '' "$entries" true || return $?
  hydrate_blobs snapshot '' "$entries" "$label" "selected snapshot" || return $?
  preflight_tree_blob_sizes snapshot '' "$entries" false || return $?
  git_in_snapshot_context -C "$snapshot" read-tree "$tree" || return $?
  materialize_tree "$snapshot" "$entries" || return $?
  validate_snapshot_symlinks "$snapshot" || return $?
  find "$snapshot" -type f -exec chmod a-w {} + || return $?
  find "$snapshot" -type d -exec chmod a-w {} + || return $?
  run_generator "$snapshot" "$preview" "$label"
}

resolve_push_tree() {
  local local_oid=$1
  local remote_ref=$2
  local peeled_oid
  local peeled_type
  local commit

  initialize_snapshot_reader || return $?
  case "$remote_ref" in
    refs/heads/*)
      commit=$(git_in_private_snapshot_reader rev-parse --verify "${local_oid}^{commit}") || {
        printf 'generated-workflows: cannot resolve pushed commit %s for %s\n' "$local_oid" "$remote_ref" >&2
        return 1
      }
      ;;
    refs/tags/*)
      peeled_oid=$(git_in_private_snapshot_reader rev-parse --verify "${local_oid}^{}") || {
        printf 'generated-workflows: cannot peel pushed tag object %s for %s\n' "$local_oid" "$remote_ref" >&2
        return 1
      }
      peeled_type=$(git_in_private_snapshot_reader cat-file -t "$peeled_oid") || {
        printf 'generated-workflows: cannot resolve peeled tag object %s for %s\n' "$peeled_oid" "$remote_ref" >&2
        return 1
      }
      if [[ "$peeled_type" != commit ]]; then
        return 2
      fi
      commit=$peeled_oid
      ;;
    *)
      return 2
      ;;
  esac

  local tree
  tree=$(git_in_private_snapshot_reader rev-parse --verify "${commit}^{tree}") || {
    printf 'generated-workflows: cannot resolve pushed tree for %s\n' "$remote_ref" >&2
    return 1
  }
  printf '%s\n' "$tree"
}

check_push_records() {
  local record=
  local local_ref
  local local_oid
  local remote_ref
  local remote_oid
  local extra
  local tree
  local checked=0
  local resolve_status

  while IFS= read -r record || [[ -n "$record" ]]; do
    IFS=$' \t' read -r local_ref local_oid remote_ref remote_oid extra <<< "$record"

    if [[ -z "${local_ref:-}" || -z "${local_oid:-}" || -z "${remote_ref:-}" || -z "${remote_oid:-}" || -n "${extra:-}" ]]; then
      printf 'generated-workflows: invalid pre-push record\n' >&2
      return 2
    fi

    if [[ ${#local_oid} -ne $object_length || ! "$local_oid" =~ ^[0-9a-f]+$ || ${#remote_oid} -ne $object_length || ! "$remote_oid" =~ ^[0-9a-f]+$ ]]; then
      printf 'generated-workflows: invalid object ID in pre-push record\n' >&2
      return 2
    fi

    if [[ "$local_oid" =~ ^0+$ ]]; then
      continue
    fi

    case "$remote_ref" in
      refs/heads/*|refs/tags/*) ;;
      *) continue ;;
    esac

    if tree=$(resolve_push_tree "$local_oid" "$remote_ref"); then
      :
    else
      resolve_status=$?
      if [[ "$resolve_status" -eq 2 ]]; then
        continue
      fi
      return "$resolve_status"
    fi

    checked=$((checked + 1))
    check_tree "$tree" "push-$checked" || return $?
  done
}

case "${1:-}" in
  pre-commit)
    if [[ $# -ne 1 ]]; then
      printf 'generated-workflows: expected pre-commit mode\n' >&2
      exit 2
    fi
    initialize_snapshot_reader || exit $?
    private_index="$temporary_root/effective-index"
    if [[ -f "$effective_index" ]]; then
      cp -p "$effective_index" "$private_index" || exit $?
      shared_index=$(git_with_private_tree_objects "$effective_index" rev-parse --shared-index-path) || {
        printf 'generated-workflows: cannot resolve the effective split-index base\n' >&2
        exit 1
      }
      if [[ -n "$shared_index" ]]; then
        if [[ ! -f "$shared_index" || -L "$shared_index" ]]; then
          printf 'generated-workflows: effective split-index base is unavailable\n' >&2
          exit 1
        fi
        cp -p "$shared_index" "$temporary_root/${shared_index##*/}" || exit $?
      fi
    else
      git_with_private_tree_objects "$private_index" read-tree --empty || exit $?
    fi
    tree=$(git_with_private_tree_objects "$private_index" write-tree --missing-ok) || {
      printf 'generated-workflows: cannot materialize the staged index\n' >&2
      exit 1
    }
    check_tree "$tree" index
    ;;
  pre-push)
    if [[ $# -ne 1 ]]; then
      printf 'generated-workflows: expected pre-push mode\n' >&2
      exit 2
    fi
    check_push_records
    ;;
  *)
    printf 'generated-workflows: expected pre-commit or pre-push mode\n' >&2
    exit 2
    ;;
esac
