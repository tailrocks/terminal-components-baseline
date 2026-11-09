#!/usr/bin/env bash
# SPDX-FileCopyrightText: 2026 Alexey Zhokhov
# SPDX-License-Identifier: Apache-2.0
set +x +a
set -euo pipefail

action=${1:?}
repo=${2:-/work/repository}
case_dir=${3:-/work/evidence}
uid=$(/usr/bin/id -u)
old_head=d29d80ee1bf3da923a0ed98e3514a2e9a5289698
old_installer_sha=669ef9abe5b24a2f16f6e21bd5e5074ef7f9dc3a70b395b015da0b0cf3816c6e
old_checker_sha=9d2fb0526f89cd58257579876786efeaacc3f88119f2f8cb782b8b0873d5c1eb
candidate_installer_sha=b5478a9c28f143558bc1b500fd3f801ed4a7765cbb9595fdec3683643b7ba836
candidate_checker_sha=b5a9002a5e67f4c770500620c33f10d412eaf7d6876ac896647e7dc546f48eea
seed_lock_sha=54035a5337e01cd95129b464ffbd0db45e4b0c989a699742efa1f0ab6329046d
bundle_sha=a142fde18873c0df9312ce33560de6f60b54d3ffd9cf8b44941a3b949d624b1f
candidate_root=/work/candidate
metadata=/work/foreign-owner-metadata
proof_root=$(cd "$(dirname "$0")" && pwd -P)
source "$proof_root/managed-upgrade-foreign-owner-observer-runtime.sh"

fail() {
  printf 'managed-upgrade-foreign-owner: %s\n' "$1" >&2
  exit 1
}

sha256_file() {
  /usr/bin/sha256sum "$1" | /usr/bin/awk '{print $1}'
}

capture_child() {
  observer_capture_child "$@"
}

captured_child_status() {
  /usr/bin/python3 - "$1" <<'PY'
import json
import sys

record = json.load(open(sys.argv[1], encoding="utf-8"))
if record.get("log_redacted_before_first_write") is not True or record.get("secret_output_detected") is not False or record.get("child_argv_contains_secret") is not False or record.get("capture_rejected") is not False:
    raise SystemExit("managed-upgrade-foreign-owner: child capture did not satisfy the secret boundary")
print(record["child_exit_status"])
PY
}

raw_child_status() {
  /usr/bin/python3 - "$1" <<'PY'
import json
import sys

print(json.load(open(sys.argv[1], encoding="utf-8"))["child_exit_status"])
PY
}

captured_output_is_clean() {
  /usr/bin/python3 - "$1" <<'PY'
import json
import sys

record = json.load(open(sys.argv[1], encoding="utf-8"))
raise SystemExit(0 if record.get("log_redacted_before_first_write") is True and record.get("secret_output_detected") is False and record.get("child_argv_contains_secret") is False and record.get("capture_rejected") is False else 1)
PY
}

refusal_oracle() {
  local kind=$1
  local candidate_status=$2
  local selected=$3
  local new_version=$4
  local relative=$5
  if [[ "$candidate_status" -eq 0 ]]; then
    printf 'ASSERTION FAILED: unchanged foreign-owner refusal oracle observed candidate acceptance; candidate_installer_status=0\n' > "$case_dir/$kind/refusal-oracle.failure.txt"
    return 42
  fi
  /usr/bin/grep -F -q 'refusing to replace an existing hook' "$case_dir/$kind/candidate-installer.log" || fail "candidate refusal did not reach the existing-hook generation validation boundary"
  /usr/bin/cmp -s "$case_dir/$kind/before.snapshot" "$case_dir/$kind/after.snapshot" || fail "fixed candidate changed protected state after foreign-owner refusal"
  [[ "$(/usr/bin/stat -c '%u:%g' "$selected")" == 0:0 ]] || fail "fixed refusal changed the selected foreign owner"
  [[ ! -e "$new_version" && ! -L "$new_version" ]] || fail "fixed refusal published the candidate checker generation"
  [[ "$kind" == negative ]] || return 43
  printf 'oracle=foreign-owner-refusal-and-full-protected-state-preservation\ncandidate_status=%s\nselected_relative_path=%s\nselected_owner=0:0\n' "$candidate_status" "$relative" > "$case_dir/$kind/refusal-oracle.success.txt"
}

observer_preflight() {
  /bin/bash "$proof_root/managed-upgrade-foreign-owner-observer-preflight.sh" "$1"
}

observer_assert() {
  /usr/bin/python3 "$proof_root/managed-upgrade-foreign-owner-observer-preflight.py" "$1" --assert || fail "independent native exec observer capability preflight failed; bounded raw diagnostics were retained"
}

common_directory() {
  local value
  value=$(/usr/bin/git -C "$repo" rev-parse --path-format=absolute --git-common-dir) || return 1
  (cd "$value" && pwd -P)
}

installation_root() {
  printf '%s/jackin-generated-workflow-guard\n' "$(common_directory)"
}

hooks_directory() {
  local value
  value=$(/usr/bin/git -C "$repo" rev-parse --path-format=absolute --git-path hooks) || return 1
  (cd "$value" && pwd -P)
}

read_prior_paths() {
  /usr/bin/python3 - "$(installation_root)" <<'PY'
import hashlib
import sys
from pathlib import Path

root = Path(sys.argv[1])
old_hash = "9d2fb0526f89cd58257579876786efeaacc3f88119f2f8cb782b8b0873d5c1eb"
manifest = root / "versions" / old_hash / "manifest"
fields = {}
for line in manifest.read_text(encoding="utf-8").splitlines():
    key, separator, value = line.partition("=")
    if not separator or key in fields:
        raise SystemExit("managed-upgrade-foreign-owner: prior manifest is malformed")
    fields[key] = value
if len(fields) != 16 or "trusted_seed_cargo_lock_sha256" in fields:
    raise SystemExit("managed-upgrade-foreign-owner: prior manifest is not the genuine 16-field format")
if fields.get("checker_sha256") != old_hash:
    raise SystemExit("managed-upgrade-foreign-owner: prior checker manifest binding is wrong")
generation_values = (
    "linux-x64",
    fields["python_version"],
    fields["python_artifact_sha256"],
    fields["python_lock_sha256"],
    fields["python_mise_sha256"],
    fields["inventory_helper_sha256"],
    fields["mise_version"],
    fields["mise_sha256"],
)
generation = hashlib.sha256("\n".join(generation_values).encode()).hexdigest()
version = root / "versions" / old_hash
python_root = root / "artifacts" / "python" / fields["python_version"]
python_generation = python_root / f"linux-x64-{generation}"
runtime = python_generation / "runtime"
inventory = python_generation / "inventory"
if not version.is_dir() or not runtime.is_dir() or not inventory.is_file():
    raise SystemExit("managed-upgrade-foreign-owner: prior generation or Linux runtime is absent")
print("\t".join(map(str, (version, python_generation, runtime, inventory))))
PY
}

select_stdlib_member() {
  local inventory=$1
  /usr/bin/python3 - "$inventory" <<'PY'
import os
import stat
import sys

inventory = sys.argv[1]
for line in open(inventory, encoding="utf-8"):
    fields = line.rstrip("\n").split("\t")
    if len(fields) != 4 or fields[0] != "file":
        continue
    mode = int(fields[1], 8)
    relative = fields[3]
    if not relative.startswith("lib/python3.15/") or not relative.endswith(".py") or mode & 0o111 or not mode & 0o004:
        continue
    if relative.startswith("/") or ".." in relative.split("/") or any(ord(char) < 32 for char in relative):
        raise SystemExit("managed-upgrade-foreign-owner: selected path is unsafe")
    print(relative)
    break
else:
    raise SystemExit("managed-upgrade-foreign-owner: no non-executable stdlib source member exists")
PY
}

snapshot_state() {
  local output=$1
  local root
  local install
  local hooks
  local common
  local new_generation
  install=$(installation_root)
  hooks=$(hooks_directory)
  common=$(common_directory)
  local old_version old_runtime
  old_version=$(cat "$metadata/old-version-dir")
  old_runtime=$(cat "$metadata/old-runtime")
  new_generation="$install/versions/$candidate_checker_sha"
  : > "$output"
  for root in \
    "$install" \
    "$common" \
    "$repo" \
    "/home/verifier" \
    "$old_version" \
    "$old_runtime" \
    "$hooks" \
    "$install/unrelated-preserved-sentinel" \
    "$new_generation" \
    "$install/install.lock" \
    "$install/install.advisory.lock"; do
    printf 'subject=%s\n' "$root" >> "$output"
    /usr/bin/python3 "$proof_root/managed-upgrade-foreign-owner-snapshot.py" "$root" >> "$output"
  done
}

render_expected_current_hook() {
  local mode=$1
  local generation_dir=$2
  local expected_install_root=$3
  LC_ALL=C
  export LC_ALL
  printf '#!/bin/bash\nset +x +a\nunset GITHUB_COM_TOKEN\nset -euo pipefail\nLC_ALL=C\nexport LC_ALL\nguard_owner=jackin-generated-workflow-guard\nPATH=/usr/bin:/bin:/usr/sbin:/sbin\nexport PATH\nentry_index_present=0\nentry_index=\nif [[ ${GIT_INDEX_FILE+x} ]]; then\n  entry_index_present=1\n  entry_index=$GIT_INDEX_FILE\nfi\n'
  printf 'checker=%q\nmode=%q\ntrusted_install_root=%q\ntrusted_version_dir=%q\nhash_tool=%q\n' "$generation_dir/check-generated-workflows.sh" "$mode" "$expected_install_root" "$generation_dir" /usr/bin/sha256sum
  cat <<'HOOK'
sha256_file() {
  if [[ "$hash_tool" == /usr/bin/shasum ]]; then
    "$hash_tool" -a 256 "$1" | /usr/bin/awk '{print $1}'
  else
    "$hash_tool" "$1" | /usr/bin/awk '{print $1}'
  fi
}
if [[ -L "$trusted_install_root" || ! -d "$trusted_install_root" || -L "$trusted_install_root/versions" || ! -d "$trusted_install_root/versions" || -L "$trusted_version_dir" || ! -d "$trusted_version_dir" || "$checker" != "$trusted_version_dir/check-generated-workflows.sh" || -L "$checker" || ! -f "$checker" ]]; then
  printf 'generated-workflows: trusted checker installation is unsafe\n' >&2
  exit 1
fi
if [[ "$(sha256_file "$checker")" != "${trusted_version_dir##*/}" ]]; then
  printf 'generated-workflows: trusted checker failed its launcher integrity check\n' >&2
  exit 1
fi
current_user=$(/usr/bin/id -u)
for trusted_directory in "$trusted_install_root" "$trusted_install_root/versions" "$trusted_version_dir"; do
  if [[ "$(uname -s)" == Darwin ]]; then
    trusted_owner=$(/usr/bin/stat -f '%u' "$trusted_directory")
    trusted_mode=$(/usr/bin/stat -f '%Lp' "$trusted_directory")
  else
    trusted_owner=$(/usr/bin/stat -c '%u' "$trusted_directory")
    trusted_mode=$(/usr/bin/stat -c '%a' "$trusted_directory")
  fi
  if [[ "$trusted_owner" != "$current_user" ]] || (( (8#$trusted_mode & 077) != 0 )); then
    printf 'generated-workflows: trusted checker directory owner or permissions are unsafe\n' >&2
    exit 1
  fi
done
if [[ "$(uname -s)" == Darwin ]]; then
  checker_owner=$(/usr/bin/stat -f '%u' "$checker")
  checker_mode=$(/usr/bin/stat -f '%Lp' "$checker")
else
  checker_owner=$(/usr/bin/stat -c '%u' "$checker")
  checker_mode=$(/usr/bin/stat -c '%a' "$checker")
fi
if [[ "$checker_owner" != "$current_user" ]] || (( (8#$checker_mode & 022) != 0 )); then
  printf 'generated-workflows: trusted checker owner or permissions are unsafe\n' >&2
  exit 1
fi
if [[ "$entry_index_present" -eq 1 ]]; then
  exec /usr/bin/env -i PATH=/usr/bin:/bin:/usr/sbin:/sbin HOME=/tmp TMPDIR=/tmp LANG=C LC_ALL=C GIT_CONFIG_NOSYSTEM=1 GIT_CONFIG_GLOBAL=/dev/null GIT_CONFIG_SYSTEM=/dev/null GIT_NO_REPLACE_OBJECTS=1 GIT_NO_LAZY_FETCH=1 "GIT_INDEX_FILE=$entry_index" /bin/bash "$checker" "$mode" "$@"
else
  exec /usr/bin/env -i PATH=/usr/bin:/bin:/usr/sbin:/sbin HOME=/tmp TMPDIR=/tmp LANG=C LC_ALL=C GIT_CONFIG_NOSYSTEM=1 GIT_CONFIG_GLOBAL=/dev/null GIT_CONFIG_SYSTEM=/dev/null GIT_NO_REPLACE_OBJECTS=1 GIT_NO_LAZY_FETCH=1 /bin/bash "$checker" "$mode" "$@"
fi
HOOK
}

run_prepare() {
  [[ "$uid" == 1000 ]] || fail "old generation preparation must run as UID 1000"
  [[ "$(uname -s)/$(uname -m)" == Linux/x86_64 ]] || fail "fixture platform is not Linux amd64"
  [[ ! -e "$repo" && ! -L "$repo" ]] || fail "repository path already exists"
  local setup_token=${GITHUB_COM_TOKEN-}
  unset GITHUB_COM_TOKEN GITHUB_TOKEN GH_TOKEN
  mkdir -p "$(dirname "$repo")" "$case_dir/prepared" "$metadata"
  [[ "$(sha256_file /work/baseline.bundle)" == "$bundle_sha" ]] || fail "immutable d29 bundle hash differs from its frozen input"
  /usr/bin/git init --quiet --initial-branch=main "$repo"
  /usr/bin/git -C "$repo" fetch --quiet --no-tags /work/baseline.bundle refs/remotes/origin/feature/openshell-native
  /usr/bin/git -C "$repo" checkout --quiet --detach "$old_head"
  [[ "$(/usr/bin/git -C "$repo" rev-parse HEAD)" == "$old_head" ]] || fail "old Git source is not the immutable d29 revision"
  [[ "$(/usr/bin/git -C "$repo" ls-tree "$old_head" -- scripts/install-generated-workflow-guard.sh | /usr/bin/awk '{print $1}')" == 100755 ]] || fail "old installer tracked mode differs from d29"
  [[ "$(/usr/bin/git -C "$repo" ls-tree "$old_head" -- scripts/check-generated-workflows.sh | /usr/bin/awk '{print $1}')" == 100644 ]] || fail "old checker tracked mode differs from d29"
  local old_input old_mode actual_mode old_blob actual_blob
  for old_input in \
    scripts/check-generated-workflows.sh \
    scripts/generated-workflow-guard-config.toml \
    scripts/python-inventory.sh \
    scripts/trusted-seed.commit \
    scripts/trusted-seed-metadata.pack \
    scripts/trusted-seed-metadata.objects \
    scripts/python-mise.toml \
    scripts/python-mise.macos-arm64.lock.toml \
    scripts/python-mise.linux-x64.lock.toml \
    scripts/install-generated-workflow-guard.sh; do
    old_mode=$(/usr/bin/git -C "$repo" ls-tree "$old_head" -- "$old_input" | /usr/bin/awk '{print $1}')
    case "$old_mode" in
      100755) actual_mode=755 ;;
      100644) actual_mode=644 ;;
      *) fail "d29 input has unsupported tracked mode: $old_input" ;;
    esac
    [[ "$(/usr/bin/stat -c '%a' "$repo/$old_input")" == "$actual_mode" ]] || fail "d29 checkout did not preserve the original tracked mode: $old_input"
    old_blob=$(/usr/bin/git -C "$repo" rev-parse "$old_head:$old_input")
    actual_blob=$(/usr/bin/git -C "$repo" hash-object "$repo/$old_input")
    [[ "$actual_blob" == "$old_blob" ]] || fail "d29 checkout did not preserve the original Git blob: $old_input"
  done
  [[ "$(sha256_file "$repo/scripts/install-generated-workflow-guard.sh")" == "$old_installer_sha" ]] || fail "old installer bytes differ from d29"
  [[ "$(sha256_file "$repo/scripts/check-generated-workflows.sh")" == "$old_checker_sha" ]] || fail "old checker bytes differ from d29"
  local install
  install=$(installation_root)
  local capture_status=0
  if GITHUB_COM_TOKEN="$setup_token" capture_child "$case_dir/prepared/old-installer.status.json" "$case_dir/prepared/old-installer.log" "$repo" /bin/bash scripts/install-generated-workflow-guard.sh; then
    :
  else
    capture_status=$?
    fail "real d29 installer output violated the secret-safe capture boundary (capture status $capture_status)"
  fi
  local status
  status=$(captured_child_status "$case_dir/prepared/old-installer.status.json")
  unset setup_token
  [[ "$status" == 0 ]] || fail "real d29 installer failed with child status $status"
  local old_version="$install/versions/$old_checker_sha"
  [[ -d "$old_version" && ! -L "$old_version" ]] || fail "real d29 generation was not installed"
  [[ "$(/usr/bin/awk 'END { print NR }' "$old_version/manifest")" == 16 ]] || fail "real old generation is not the 16-field contract"
  [[ ! -e "$old_version/trusted-seed-cargo.lock" && ! -L "$old_version/trusted-seed-cargo.lock" ]] || fail "real old generation unexpectedly contains the new lock artifact"
  local hooks
  hooks=$(hooks_directory)
  for hook in pre-commit pre-push; do
    [[ -f "$hooks/$hook" && ! -L "$hooks/$hook" ]] || fail "old installer did not create regular $hook"
    [[ "$(/usr/bin/stat -c '%a' "$hooks/$hook")" == 700 ]] || fail "old installer did not create canonical $hook mode"
    [[ "$(/usr/bin/stat -c '%u' "$hooks/$hook")" == 1000 ]] || fail "old installer did not create UID 1000 $hook"
    /usr/bin/grep -F -x -q "checker=$install/versions/$old_checker_sha/check-generated-workflows.sh" "$hooks/$hook" || fail "old $hook does not name the actual d29 checker path"
  done
  [[ "$(sha256_file "$hooks/pre-commit")" != "$(sha256_file "$hooks/pre-push")" ]] || fail "old canonical hook modes unexpectedly have identical bytes"
  printf 'preserve this unrelated install payload\n' > "$install/unrelated-preserved-sentinel"
  chmod 600 "$install/unrelated-preserved-sentinel"
  /usr/bin/python3 - "$candidate_root" "$repo" <<'PY'
import hashlib
import json
import os
import shutil
import stat
import sys
from pathlib import Path

source = Path(sys.argv[1])
destination = Path(sys.argv[2])
manifest = json.loads(Path("/work/input-manifest.json").read_text(encoding="utf-8"))
expected_files = manifest["candidate_files"]
paths = (
    "Cargo.toml",
    "Cargo.lock",
    "apps/jackin/Cargo.toml",
    "scripts/check-generated-workflows.sh",
    "scripts/generated-workflow-guard-config.toml",
    "scripts/install-generated-workflow-guard.sh",
    "scripts/python-inventory.sh",
    "scripts/python-mise.toml",
    "scripts/python-mise.linux-x64.lock.toml",
    "scripts/python-mise.macos-arm64.lock.toml",
    "scripts/trusted-seed.commit",
    "scripts/trusted-seed-metadata.pack",
    "scripts/trusted-seed-metadata.objects",
    "scripts/trusted-seed-cargo.lock",
)
for relative in paths:
    original = source / relative
    target = destination / relative
    info = original.lstat()
    if not stat.S_ISREG(info.st_mode) or info.st_nlink != 1:
        raise SystemExit(f"managed-upgrade-foreign-owner: candidate input is not a singly linked regular file: {relative}")
    if expected_files.get(relative, {}).get("sha256") != hashlib.sha256(original.read_bytes()).hexdigest():
        raise SystemExit(f"managed-upgrade-foreign-owner: candidate input differs from the frozen manifest: {relative}")
    if expected_files[relative].get("mode") != format(stat.S_IMODE(info.st_mode), "04o"):
        raise SystemExit(f"managed-upgrade-foreign-owner: candidate mode differs from the frozen manifest: {relative}")
    target.parent.mkdir(parents=True, exist_ok=True)
    shutil.copyfile(original, target)
    os.chmod(target, stat.S_IMODE(info.st_mode))
    if hashlib.sha256(original.read_bytes()).digest() != hashlib.sha256(target.read_bytes()).digest():
        raise SystemExit(f"managed-upgrade-foreign-owner: candidate input copy changed bytes: {relative}")
PY
  [[ "$(sha256_file "$repo/scripts/install-generated-workflow-guard.sh")" == "$candidate_installer_sha" ]] || fail "current candidate installer hash differs from the frozen input"
  [[ "$(sha256_file "$repo/scripts/check-generated-workflows.sh")" == "$candidate_checker_sha" ]] || fail "current candidate checker hash differs from the frozen input"
  [[ "$(sha256_file "$repo/scripts/trusted-seed-cargo.lock")" == "$seed_lock_sha" ]] || fail "current seed lock hash differs from the frozen input"
  [[ "$(/usr/bin/stat -c '%s' "$repo/scripts/trusted-seed-cargo.lock")" == 69918 ]] || fail "current seed lock size differs from the frozen input"
  local paths
  paths=$(read_prior_paths)
  local old_python_generation old_runtime old_inventory
  IFS=$'\t' read -r old_version old_python_generation old_runtime old_inventory <<< "$paths"
  [[ "$(sha256_file "$repo/scripts/python-inventory.sh")" == "$(/usr/bin/sed -n 's/^inventory_helper_sha256=//p' "$old_version/manifest")" ]] || fail "current trusted inventory helper differs from the prior generation's reviewed pin"
  /bin/bash "$repo/scripts/python-inventory.sh" check "$old_runtime" "$old_inventory" > "$case_dir/prepared/inventory-before-owner-change.log" 2>&1 || fail "current inventory helper rejected the intact old runtime"
  printf 'inventory_check_before_owner_change=0\n' > "$case_dir/prepared/inventory-status.txt"
  select_stdlib_member "$old_inventory" > "$metadata/runtime-relative-path"
  [[ -s "$metadata/runtime-relative-path" ]] || fail "selected runtime member path is empty"
  /usr/bin/stat -c '%u:%g:%a:%h:%F' "$old_runtime/$(cat "$metadata/runtime-relative-path")" > "$metadata/runtime-member-before.txt"
  sha256_file "$old_runtime/$(cat "$metadata/runtime-relative-path")" > "$metadata/runtime-member-sha256.txt"
  printf '%s\n' "$repo" > "$metadata/repository-path"
  printf '%s\n' "$install" > "$metadata/install-root"
  printf '%s\n' "$old_version" > "$metadata/old-version-dir"
  printf '%s\n' "$old_python_generation" > "$metadata/old-python-generation"
  printf '%s\n' "$old_runtime" > "$metadata/old-runtime"
  printf '%s\n' "$old_inventory" > "$metadata/old-inventory"
  printf 'old_head=%s\nold_installer_sha256=%s\nold_checker_sha256=%s\ncandidate_installer_sha256=%s\ncandidate_checker_sha256=%s\ntrusted_seed_cargo_lock_sha256=%s\nuid=%s\nplatform=%s/%s\n' "$old_head" "$old_installer_sha" "$old_checker_sha" "$candidate_installer_sha" "$candidate_checker_sha" "$seed_lock_sha" "$uid" "$(uname -s)" "$(uname -m)" > "$case_dir/prepared/inputs.txt"
  printf 'case=old-generation-prepared\nrepository=%s\ninstaller_status=0\n' "$repo" > "$case_dir/prepared/fixture-witness.txt"
}

run_owner_chown() {
  [[ "$uid" == 0 ]] || fail "foreign owner setup must run as BuildKit root"
  local install old_version old_python_generation old_runtime old_inventory relative selected
  install=$(cat "$metadata/install-root")
  old_version=$(cat "$metadata/old-version-dir")
  old_python_generation=$(cat "$metadata/old-python-generation")
  old_runtime=$(cat "$metadata/old-runtime")
  old_inventory=$(cat "$metadata/old-inventory")
  relative=$(cat "$metadata/runtime-relative-path")
  selected="$old_runtime/$relative"
  mkdir -p "$case_dir/owner-setup"
  [[ "$(sha256_file "$repo/scripts/python-inventory.sh")" == "$(/usr/bin/sed -n 's/^inventory_helper_sha256=//p' "$old_version/manifest")" ]] || fail "foreign owner fixture does not use the currently trusted inventory helper"
  /bin/bash "$repo/scripts/python-inventory.sh" check "$old_runtime" "$old_inventory" > "$case_dir/owner-setup/inventory-before.log" 2>&1 || fail "current inventory helper rejected the intact old runtime before chown"
  [[ "$(/usr/bin/stat -c '%u' "$selected")" == 1000 ]] || fail "selected runtime member was not initially owned by UID 1000"
  [[ "$(/usr/bin/stat -c '%g' "$selected")" == 1000 ]] || fail "selected runtime member was not initially in GID 1000"
  [[ -f "$selected" && ! -L "$selected" ]] || fail "selected runtime member is not a regular file"
  [[ "$(/usr/bin/stat -c '%h' "$selected")" == 1 ]] || fail "selected runtime member is not singly linked"
  local selected_mode selected_hash
  selected_mode=$(/usr/bin/stat -c '%a' "$selected")
  (( (8#$selected_mode & 0111) == 0 )) || fail "selected runtime member is executable"
  (( (8#$selected_mode & 0004) != 0 )) || fail "selected runtime member would not be readable by UID 1000 after root ownership changes"
  selected_hash=$(sha256_file "$selected")
  snapshot_state "$case_dir/owner-setup/before-chown.snapshot"
  /usr/bin/chown 0:0 "$selected"
  [[ "$(/usr/bin/stat -c '%u:%g:%a:%h:%F' "$selected")" == "0:0:$selected_mode:1:regular file" ]] || fail "chown changed selected file metadata beyond owner and group"
  [[ "$(sha256_file "$selected")" == "$selected_hash" ]] || fail "chown changed selected file bytes"
  /bin/bash "$repo/scripts/python-inventory.sh" check "$old_runtime" "$old_inventory" > "$case_dir/owner-setup/inventory-after.log" 2>&1 || fail "current inventory helper rejected the foreign-owned runtime"
  /usr/bin/runuser -u verifier -- /usr/bin/cat "$selected" > /dev/null || fail "UID 1000 cannot traverse to and read the foreign-owned runtime member"
  printf 'inventory_check_before_chown=0\ninventory_check_after_chown=0\nuid_1000_read_after_chown=0\n' > "$case_dir/owner-setup/fixture-status.txt"
  snapshot_state "$case_dir/owner-setup/after-chown.snapshot"
  /usr/bin/python3 - "$case_dir/owner-setup/before-chown.snapshot" "$case_dir/owner-setup/after-chown.snapshot" "$selected" <<'PY'
import json
import os
import sys

def read(path):
    output = {}
    subject = None
    for line in open(path, encoding="utf-8"):
        line = line.rstrip("\n")
        if line.startswith("subject="):
            subject = line.partition("=")[2]
            output[subject] = []
        elif line:
            output[subject].append(json.loads(line))
    return output

before = read(sys.argv[1])
after = read(sys.argv[2])
selected = os.path.abspath(sys.argv[3])
if before.keys() != after.keys():
    raise SystemExit("managed-upgrade-foreign-owner: root chown changed the fixture path set")
allowed = {"uid", "gid", "ctime_ns"}
found = False
for subject in before:
    left = before[subject]
    right = after[subject]
    if len(left) != len(right):
        raise SystemExit(f"managed-upgrade-foreign-owner: root chown changed entry count under {subject}")
    for first, second in zip(left, right):
        if first["path"] != second["path"]:
            raise SystemExit(f"managed-upgrade-foreign-owner: root chown changed paths under {subject}")
        if os.path.normpath(os.path.join(subject, first["path"])) == os.path.normpath(selected):
            found = True
            for key in first.keys() | second.keys():
                if key in allowed:
                    continue
                if first.get(key) != second.get(key):
                    raise SystemExit("managed-upgrade-foreign-owner: root chown changed selected member metadata other than UID/GID")
            if (first["uid"], first["gid"], second["uid"], second["gid"]) != (1000, 1000, 0, 0):
                raise SystemExit("managed-upgrade-foreign-owner: selected member ownership transition is not UID/GID 1000 to 0")
        elif first != second:
            raise SystemExit(f"managed-upgrade-foreign-owner: root chown changed unrelated state under {subject}")
if not found:
    raise SystemExit("managed-upgrade-foreign-owner: selected member is absent from the runtime snapshot")
PY
  printf 'selected_relative_path=%s\nselected_path=%s\nselected_sha256=%s\nselected_mode=%s\nselected_owner_before=1000:1000\nselected_owner_after=0:0\ncurrent_inventory_check_before=0\ncurrent_inventory_check_after=0\n' "$relative" "$selected" "$selected_hash" "$selected_mode" > "$case_dir/owner-setup/foreign-owner-fixture.txt"
}

check_current_overlay() {
  [[ "$uid" == 1000 ]] || fail "candidate installer must run as UID 1000"
  [[ "$(uname -s)/$(uname -m)" == Linux/x86_64 ]] || fail "candidate run is not Linux amd64"
  [[ "$(/usr/bin/git -C "$repo" rev-parse HEAD)" == "$old_head" ]] || fail "test repository no longer identifies the real d29 fixture"
  [[ "$(sha256_file "$repo/scripts/install-generated-workflow-guard.sh")" == "$candidate_installer_sha" ]] || fail "candidate installer source changed outside the approved private mutant"
  [[ "$(sha256_file "$repo/scripts/check-generated-workflows.sh")" == "$candidate_checker_sha" ]] || fail "candidate checker source changed"
  [[ "$(sha256_file "$repo/scripts/trusted-seed-cargo.lock")" == "$seed_lock_sha" ]] || fail "candidate seed lock source changed"
}

run_candidate_case() {
  local kind=$1
  [[ "$kind" == positive || "$kind" == negative || "$kind" == mutant ]] || fail "unsupported candidate case"
  mkdir -p "$case_dir/$kind"
  local setup_token=${GITHUB_COM_TOKEN-}
  unset GITHUB_COM_TOKEN GITHUB_TOKEN GH_TOKEN
  local install old_version old_python_generation old_runtime old_inventory relative selected hooks common new_version
  install=$(cat "$metadata/install-root")
  old_version=$(cat "$metadata/old-version-dir")
  old_python_generation=$(cat "$metadata/old-python-generation")
  old_runtime=$(cat "$metadata/old-runtime")
  old_inventory=$(cat "$metadata/old-inventory")
  relative=$(cat "$metadata/runtime-relative-path")
  selected="$old_runtime/$relative"
  hooks=$(hooks_directory)
  common=$(common_directory)
  new_version="$install/versions/$candidate_checker_sha"
  check_current_overlay
  [[ "$(/usr/bin/awk 'END { print NR }' "$old_version/manifest")" == 16 ]] || fail "old manifest no longer satisfies the exact 16-field contract"
  [[ ! -e "$old_version/trusted-seed-cargo.lock" && ! -L "$old_version/trusted-seed-cargo.lock" ]] || fail "old generation unexpectedly acquired the new seed lock artifact"
  [[ "$(/usr/bin/cat "$install/unrelated-preserved-sentinel")" == 'preserve this unrelated install payload' ]] || fail "sentinel does not match its captured setup value"
  [[ -f "$selected" && ! -L "$selected" && "$(sha256_file "$selected")" == "$(cat "$metadata/runtime-member-sha256.txt")" ]] || fail "selected runtime member bytes or type changed"
  local member_mode
  member_mode=$(/usr/bin/stat -c '%a' "$selected")
  (( (8#$member_mode & 0111) == 0 )) || fail "selected runtime member became executable"
  [[ "$(/usr/bin/stat -c '%h' "$selected")" == 1 ]] || fail "selected runtime member link count changed"
  /bin/bash "$repo/scripts/python-inventory.sh" check "$old_runtime" "$old_inventory" > "$case_dir/$kind/inventory-before.log" 2>&1 || fail "current inventory helper rejected the runtime before candidate invocation"
  printf 'inventory_check_before_candidate=0\n' > "$case_dir/$kind/inventory-status.txt"
  if [[ "$kind" == negative || "$kind" == mutant ]]; then
    [[ "$(/usr/bin/stat -c '%u:%g' "$selected")" == 0:0 ]] || fail "negative fixture lacks exactly the root-owned selected member"
  else
    [[ "$(/usr/bin/stat -c '%u:%g' "$selected")" == 1000:1000 ]] || fail "positive fixture member is not owned by UID/GID 1000"
    /usr/bin/python3 - "$old_runtime" <<'PY'
import os
import sys

root = sys.argv[1]
def fail_walk(error):
    raise error

for directory, names, files in os.walk(root, followlinks=False, onerror=fail_walk):
    for name in names + files:
        path = os.path.join(directory, name)
        metadata = os.lstat(path)
        if metadata.st_uid != 1000 or metadata.st_gid != 1000:
            raise SystemExit("managed-upgrade-foreign-owner: intact positive runtime contains a non-1000 owner or group")
PY
  fi
  if [[ "$kind" == mutant ]]; then
    /usr/bin/python3 - "$repo/scripts/install-generated-workflow-guard.sh" <<'PY'
import sys
from pathlib import Path

path = Path(sys.argv[1])
source = path.read_text(encoding="utf-8")
needle = '      [[ "$owner" == "$current_uid" ]] || return 1\n'
if source.count(needle) != 1:
    raise SystemExit("managed-upgrade-foreign-owner: approved ownership guard mutation anchor is not unique")
path.write_text(source.replace(needle, "      :\n", 1), encoding="utf-8")
PY
    if /usr/bin/diff -u /work/candidate/scripts/install-generated-workflow-guard.sh "$repo/scripts/install-generated-workflow-guard.sh" > "$case_dir/mutant/installer-owner-check-removal.patch"; then
      fail "approved private owner-check mutant produced no source delta"
    else
      [[ "$?" == 1 ]] || fail "could not record the private guard mutant diff"
    fi
    [[ "$(/usr/bin/grep -c '^[+-][^+-]' "$case_dir/mutant/installer-owner-check-removal.patch")" == 2 ]] || fail "private mutant changed more than the batch owner equality check"
    /usr/bin/cp "$repo/scripts/install-generated-workflow-guard.sh" "$case_dir/mutant/mutated-installer.sh"
    sha256_file "$repo/scripts/install-generated-workflow-guard.sh" > "$case_dir/mutant/mutated-installer.sha256"
  fi
  /bin/bash "$repo/scripts/python-inventory.sh" check "$old_runtime" "$old_inventory" > "$case_dir/$kind/inventory-after-fixture.log" 2>&1 || fail "current inventory helper does not pass the exact candidate fixture"
  printf 'inventory_check_after_fixture=0\n' >> "$case_dir/$kind/inventory-status.txt"
  [[ ! -e "$new_version" && ! -L "$new_version" ]] || fail "candidate checker generation already exists before its first operation"
  sha256_file "$common/config" > "$case_dir/$kind/git-config-before.sha256"
  sha256_file "$common/index" > "$case_dir/$kind/git-index-before.sha256"
  /usr/bin/python3 "$proof_root/managed-upgrade-foreign-owner-snapshot.py" "$old_version" > "$case_dir/$kind/old-generation.before.snapshot"
  /usr/bin/python3 "$proof_root/managed-upgrade-foreign-owner-snapshot.py" "$old_python_generation" > "$case_dir/$kind/old-python-generation.before.snapshot"
  /usr/bin/python3 "$proof_root/managed-upgrade-foreign-owner-snapshot.py" "$install/unrelated-preserved-sentinel" > "$case_dir/$kind/sentinel.before.snapshot"
  snapshot_state "$case_dir/$kind/before.snapshot"
  local capture_status=0
  local candidate_status=0
  local observer_log="$case_dir/$kind/execution-observer.log"
  observer_initialize || fail "the independently qualified native ARM64 exec observer is unavailable"
  observer_prepare_trace /dev/stderr || fail "the shared bounded observer argv could not be prepared"
  if GITHUB_COM_TOKEN="$setup_token" capture_child "$case_dir/$kind/candidate-status.json" "$case_dir/$kind/candidate-installer.log" "$repo" "${observer_trace_argv[@]}" /bin/bash scripts/install-generated-workflow-guard.sh; then
    capture_status=0
  else
    capture_status=$?
  fi
  if [[ -f "$case_dir/$kind/candidate-status.json" && ! -L "$case_dir/$kind/candidate-status.json" ]]; then
    candidate_status=$(raw_child_status "$case_dir/$kind/candidate-status.json")
    printf 'candidate_installer_status=%s\ncapture_status=%s\nuid=%s\nplatform=%s/%s\nold_generation_path=%s\nold_python_runtime_path=%s\nold_checker_path=%s\nold_inventory_helper_path=%s\npre_commit_hook_path=%s\npre_push_hook_path=%s\n' "$candidate_status" "$capture_status" "$uid" "$(uname -s)" "$(uname -m)" "$old_version" "$old_runtime" "$old_version/check-generated-workflows.sh" "$old_version/python-inventory.sh" "$hooks/pre-commit" "$hooks/pre-push" > "$case_dir/$kind/candidate-status.txt"
  else
    fail "secret-safe candidate capture did not retain a status record"
  fi
  [[ "$capture_status" == 0 ]] && captured_output_is_clean "$case_dir/$kind/candidate-status.json" || fail "candidate output violated the secret-safe capture boundary (capture status $capture_status; child status $candidate_status)"
  /usr/bin/python3 "$proof_root/managed-upgrade-foreign-owner-execution-observer.py" "$case_dir/$kind/candidate-installer.log" "$observer_log" "$old_version" "$old_runtime" "$old_version/check-generated-workflows.sh" "$old_version/python-inventory.sh" "$hooks/pre-commit" "$hooks/pre-push"
  unset setup_token
  snapshot_state "$case_dir/$kind/after.snapshot"
  if [[ "$kind" == negative ]]; then
    refusal_oracle "$kind" "$candidate_status" "$selected" "$new_version" "$relative"
    printf 'case=foreign-owned-runtime-refused-and-preserved\ncandidate_status=%s\nselected_relative_path=%s\nselected_sha256=%s\nselected_owner=0:0\n' "$candidate_status" "$relative" "$(cat "$metadata/runtime-member-sha256.txt")" > "$case_dir/$kind/case-witness.txt"
    return 0
  fi
  if [[ "$kind" == mutant ]]; then
    local oracle_status=0
    if refusal_oracle "$kind" "$candidate_status" "$selected" "$new_version" "$relative"; then
      fail "private owner-check mutant unexpectedly satisfied the fixed refusal oracle"
    else
      oracle_status=$?
    fi
    [[ "$oracle_status" == 42 ]] || fail "private owner-check mutant failed outside the unchanged refusal oracle (status $oracle_status)"
    /usr/bin/cmp -s "$case_dir/$kind/before.snapshot" "$case_dir/$kind/after.snapshot" && fail "private owner-check mutant unexpectedly preserved protected state after acceptance"
    printf 'case=causal-owner-check-mutant-accepted-and-unchanged-oracle-failed\ncandidate_status=%s\noracle_status=%s\nselected_relative_path=%s\nselected_owner=0:0\n' "$candidate_status" "$oracle_status" "$relative" > "$case_dir/$kind/case-witness.txt"
    printf 'expected_assertion=unchanged-foreign-owner-refusal-oracle-rejected-mutant-acceptance\nprotected_state_changed=yes\n' > "$case_dir/$kind/mutant-diagnostic.txt"
    return 0
  fi
  [[ "$candidate_status" -eq 0 ]] || {
    cat "$case_dir/$kind/candidate-installer.log" >&2
    fail "candidate refused the intact UID 1000 old generation with status $candidate_status"
  }
  [[ -f "$new_version/manifest" && ! -L "$new_version/manifest" ]] || fail "positive upgrade did not publish the current generation manifest"
  [[ "$(/usr/bin/awk 'END { print NR }' "$new_version/manifest")" == 17 ]] || fail "current generation does not satisfy the exact 17-field contract"
  /usr/bin/grep -F -x -q "trusted_seed_cargo_lock_sha256=$seed_lock_sha" "$new_version/manifest" || fail "current generation manifest does not bind the reviewed seed lock"
  [[ "$(sha256_file "$new_version/trusted-seed-cargo.lock")" == "$seed_lock_sha" ]] || fail "current generation seed lock bytes differ from the reviewed artifact"
  [[ "$(sha256_file "$old_version/check-generated-workflows.sh")" == "$old_checker_sha" ]] || fail "positive upgrade changed prior checker bytes"
  /usr/bin/python3 "$proof_root/managed-upgrade-foreign-owner-snapshot.py" "$old_version" > "$case_dir/$kind/old-generation.after.snapshot"
  /usr/bin/python3 "$proof_root/managed-upgrade-foreign-owner-snapshot.py" "$old_python_generation" > "$case_dir/$kind/old-python-generation.after.snapshot"
  /usr/bin/python3 "$proof_root/managed-upgrade-foreign-owner-snapshot.py" "$install/unrelated-preserved-sentinel" > "$case_dir/$kind/sentinel.after.snapshot"
  /usr/bin/cmp -s "$case_dir/$kind/old-generation.before.snapshot" "$case_dir/$kind/old-generation.after.snapshot" || fail "positive upgrade changed prior generation files or metadata"
  /usr/bin/cmp -s "$case_dir/$kind/old-python-generation.before.snapshot" "$case_dir/$kind/old-python-generation.after.snapshot" || fail "positive upgrade changed prior Python runtime files or metadata"
  /usr/bin/cmp -s "$case_dir/$kind/sentinel.before.snapshot" "$case_dir/$kind/sentinel.after.snapshot" || fail "positive upgrade changed the captured sentinel bytes or metadata"
  [[ "$(/usr/bin/cat "$install/unrelated-preserved-sentinel")" == 'preserve this unrelated install payload' ]] || fail "positive upgrade changed the sentinel"
  [[ "$(sha256_file "$common/config")" == "$(cat "$case_dir/$kind/git-config-before.sha256")" ]] || fail "positive upgrade changed Git common-dir config"
  [[ "$(sha256_file "$common/index")" == "$(cat "$case_dir/$kind/git-index-before.sha256")" ]] || fail "positive upgrade changed the Git index"
  [[ "$(/usr/bin/stat -c '%a:%u' "$hooks/pre-commit")" == 700:1000 && "$(/usr/bin/stat -c '%a:%u' "$hooks/pre-push")" == 700:1000 ]] || fail "positive upgrade did not create UID 1000 canonical-mode hooks"
  render_expected_current_hook pre-commit "$new_version" "$install" > "$case_dir/$kind/expected-pre-commit"
  render_expected_current_hook pre-push "$new_version" "$install" > "$case_dir/$kind/expected-pre-push"
  /usr/bin/cmp -s "$case_dir/$kind/expected-pre-commit" "$hooks/pre-commit" || fail "positive pre-commit is not the exact canonical current render"
  /usr/bin/cmp -s "$case_dir/$kind/expected-pre-push" "$hooks/pre-push" || fail "positive pre-push is not the exact canonical current render"
  [[ "$(sha256_file "$hooks/pre-commit")" != "$(sha256_file "$hooks/pre-push")" ]] || fail "canonical mode-specific current hooks have identical bytes"
  printf 'case=intact-old-generation-upgraded-to-canonical-current-pair\ncandidate_status=0\nold_checker_sha256=%s\ncurrent_checker_sha256=%s\nold_manifest_fields=16\ncurrent_manifest_fields=17\nuid=%s\nplatform=%s/%s\n' "$old_checker_sha" "$candidate_checker_sha" "$uid" "$(uname -s)" "$(uname -m)" > "$case_dir/$kind/case-witness.txt"
}

case "$action" in
  observer-preflight) observer_preflight "$case_dir" ;;
  observer-assert) observer_assert "$case_dir" ;;
  canary-control) mkdir -p "$case_dir"; /usr/bin/python3 "$proof_root/managed-upgrade-foreign-owner-capture.py" canary-control "$case_dir/canary-control" ;;
  prepare) run_prepare ;;
  root-chown) run_owner_chown ;;
  positive|negative|mutant) run_candidate_case "$action" ;;
  *) fail "unsupported action: $action" ;;
esac
