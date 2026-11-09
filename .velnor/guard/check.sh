#!/usr/bin/env bash
set -euo pipefail
cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.."
bash scripts/install-generated-workflow-guard.sh
repo_root=$(pwd -P)
git_clean() {
  env -i PATH=/usr/bin:/bin:/usr/sbin:/sbin HOME="${HOME:-/tmp}" GIT_NO_REPLACE_OBJECTS=1 GIT_NO_LAZY_FETCH=1 /usr/bin/git -C "$repo_root" "$@"
}
[[ "$(git_clean rev-parse --show-toplevel)" == "$repo_root" ]]
common_raw=$(git_clean rev-parse --path-format=absolute --git-common-dir)
[[ "$common_raw" == /* && -d "$common_raw" && ! -L "$common_raw" ]]
common_dir=$(cd -- "$common_raw" && pwd -P)
[[ "$common_dir" == "$repo_root/.git" ]]
python_parent="$common_dir/jackin-generated-workflow-guard/artifacts/python/3.15.2"
shopt -s nullglob
candidates=("$python_parent"/linux-x64-*)
[[ ${#candidates[@]} -eq 1 ]]
generation=${candidates[0]}
name=${generation##*/}
[[ "$name" =~ ^linux-x64-[0-9a-f]{64}$ && -d "$generation" && ! -L "$generation" ]]
expected_generation=$(printf '%s\n' linux-x64 3.15.2 7b03163a980c607ba8194709d4a4086735a7a9c488cc475b098157306927b36b 5c13cb611f7f461656eeb052684c8d82f17eb5979d65f89b48622a0f60c4a168 4bf669510cff7c1f5d5c7c8edc5f6d496bedf872b635295a607f81680074744d 0541173812097cd6ac4deb065a7e45b6960ddcf26369eb3fd32805a0b66f33eb 2026.11.4 0296d9cba9324106d4e0962bb94a2072902f3658813aeb3b09504947bb8ddc2b | /usr/bin/sha256sum | /usr/bin/awk '{print $1}')
[[ "$name" == "linux-x64-$expected_generation" ]]
runtime="$generation/runtime"
inventory="$generation/inventory"
manifest="$generation/manifest"
[[ -d "$runtime" && ! -L "$runtime" && -f "$inventory" && ! -L "$inventory" && -f "$manifest" && ! -L "$manifest" ]]
inventory_hash=$(/usr/bin/sha256sum "$inventory" | /usr/bin/awk '{print $1}')
expected_manifest=$(printf 'python_version=%s\nbackend=core:python\narchive_sha256=%s\nlock_sha256=%s\nmise_config_sha256=%s\ninventory_helper_sha256=%s\ninventory_sha256=%s' 3.15.2 7b03163a980c607ba8194709d4a4086735a7a9c488cc475b098157306927b36b 5c13cb611f7f461656eeb052684c8d82f17eb5979d65f89b48622a0f60c4a168 4bf669510cff7c1f5d5c7c8edc5f6d496bedf872b635295a607f81680074744d 0541173812097cd6ac4deb065a7e45b6960ddcf26369eb3fd32805a0b66f33eb "$inventory_hash")
[[ "$(/bin/cat "$manifest")" == "$expected_manifest" ]]
bash scripts/python-inventory.sh check "$runtime" "$inventory"
[[ -L "$runtime/bin/python3" && "$(/usr/bin/readlink "$runtime/bin/python3")" == python3.15 && -f "$runtime/bin/python3.15" && ! -L "$runtime/bin/python3.15" && -x "$runtime/bin/python3.15" ]]
python_hash=$(/usr/bin/sha256sum "$runtime/bin/python3.15" | /usr/bin/awk '{print $1}')
/usr/bin/awk -F '\t' -v hash="$python_hash" '$1 == "file" && $3 == hash && $4 == "bin/python3.15" { n++ } END { exit n != 1 }' "$inventory"
"$runtime/bin/python3" -I -S -c 'import sys; sys.exit(sys.version_info[:3] != (3, 15, 2))'
export PATH="$runtime/bin:$PATH"
BASELINE_GUARD_E2E_FOCUS=installer-lock-lifecycle bash tests/check-installed-generated-workflow-guard.sh
BASELINE_GUARD_E2E_FOCUS=installer-repair-mise bash tests/check-installed-generated-workflow-guard.sh
BASELINE_GUARD_E2E_FOCUS=installer-repair-mise-http-failure bash tests/check-installed-generated-workflow-guard.sh
BASELINE_GUARD_E2E_FOCUS=installer-repair-mise-wrong-hash bash tests/check-installed-generated-workflow-guard.sh
BASELINE_GUARD_E2E_FOCUS=installer-repair-python-executable bash tests/check-installed-generated-workflow-guard.sh
BASELINE_GUARD_E2E_FOCUS=installer-repair-python-inventory bash tests/check-installed-generated-workflow-guard.sh
BASELINE_GUARD_E2E_FOCUS=installer-repair-python-missing-inventory bash tests/check-installed-generated-workflow-guard.sh
BASELINE_GUARD_E2E_FOCUS=installer-unsafe-python bash tests/check-installed-generated-workflow-guard.sh
bash scripts/python-inventory.sh check "$runtime" "$inventory"
[[ "$(/usr/bin/sha256sum "$inventory" | /usr/bin/awk '{print $1}')" == "$inventory_hash" && "$(/usr/bin/sha256sum "$runtime/bin/python3.15" | /usr/bin/awk '{print $1}')" == "$python_hash" && -L "$runtime/bin/python3" && "$(/usr/bin/readlink "$runtime/bin/python3")" == python3.15 ]]
