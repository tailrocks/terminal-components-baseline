#!/usr/bin/env bash
# SPDX-FileCopyrightText: 2026 Alexey Zhokhov
# SPDX-License-Identifier: Apache-2.0
set -euo pipefail

repo_root=$(cd "$(dirname "$0")/.." && pwd -P)
e2e_focus=${BASELINE_GUARD_E2E_FOCUS:-all}
cold_install=${BASELINE_GUARD_COLD_INSTALL:-0}
public_mutant=${BASELINE_GUARD_PUBLIC_MUTANT:-}
case "$e2e_focus" in
  all|install|git-path-spaces|relative-git-path) ;;
  *) printf 'generated-workflows: unsupported real-toolchain focus %s\n' "$e2e_focus" >&2; exit 2 ;;
esac
case "$cold_install" in
  0|1) ;;
  *) printf 'generated-workflows: BASELINE_GUARD_COLD_INSTALL must be 0 or 1\n' >&2; exit 2 ;;
esac
case "$public_mutant" in
  ''|relative-git-path) ;;
  *) printf 'generated-workflows: unsupported public mutant %s\n' "$public_mutant" >&2; exit 2 ;;
esac
if [[ -n "$public_mutant" && ( "$public_mutant" != relative-git-path || "$e2e_focus" != relative-git-path || "$cold_install" != 0 ) ]]; then
  printf 'generated-workflows: the relative Git path mutant requires a warm relative-git-path run\n' >&2
  exit 2
fi
git_command=$(command -v git)
if [[ "$git_command" != /* ]]; then
  git_command="$(pwd -P)/$git_command"
fi
case "$(uname -s):$(uname -m)" in
  Darwin:arm64|Darwin:aarch64) hash_tool=/usr/bin/shasum; platform=macos-arm64 ;;
  Linux:x86_64|Linux:amd64) hash_tool=/usr/bin/sha256sum; platform=linux-x64 ;;
  *) printf 'generated-workflows: unsupported host for the real-toolchain qualification\n' >&2; exit 2 ;;
esac
sha256_file() {
  if [[ "$hash_tool" == /usr/bin/shasum ]]; then
    "$hash_tool" -a 256 "$1" | /usr/bin/awk '{print $1}'
  else
    "$hash_tool" "$1" | /usr/bin/awk '{print $1}'
  fi
}
rekey_mutant_tool_cache() {
  local cache_root=$1
  local original_checker=$2
  local mutant_checker=$3
  local info_file="$test_root/mutant-cache-info"
  local source_generation
  local target_generation
  local target_key
  python3 -I -S - "$cache_root" "$original_checker" "$mutant_checker" > "$info_file" <<'PY'
import hashlib
import sys
from pathlib import Path

cache_root = Path(sys.argv[1])
original_checker, mutant_checker = sys.argv[2:]
for generation in sorted(cache_root.iterdir()):
    manifest = generation / "manifest"
    if not generation.is_dir() or generation.is_symlink() or not manifest.is_file() or manifest.is_symlink():
        continue
    fields = {}
    for line in manifest.read_text().splitlines():
        key, separator, value = line.partition("=")
        if not separator or key in fields:
            raise SystemExit("generated-workflows: warm cache manifest is malformed")
        fields[key] = value
    if fields.get("checker_sha256") != original_checker:
        continue
    names = (
        "platform", "velnor", "rust", "jactionlint", "shellcheck", "seed_commit",
        "trusted_lock_sha256", "mise_version", "mise_sha256", "python_version",
        "python_artifact_sha256", "python_lock_sha256", "python_mise_sha256",
        "config_sha256", "inventory_helper_sha256",
    )
    try:
        values = [mutant_checker, "1"] + [fields[name] for name in names]
    except KeyError as error:
        raise SystemExit("generated-workflows: warm cache manifest is incomplete") from error
    key = hashlib.sha256("\n".join(values).encode("ascii")).hexdigest()
    print(generation)
    print(key)
    raise SystemExit(0)
raise SystemExit("generated-workflows: no warm cache matches the reviewed checker")
PY
  source_generation=$(/usr/bin/sed -n '1p' "$info_file")
  target_key=$(/usr/bin/sed -n '2p' "$info_file")
  if [[ -z "$source_generation" || ! "$target_key" =~ ^[0-9a-f]{64}$ ]]; then
    printf 'generated-workflows: cannot select the warm cache for the public mutant\n' >&2
    return 1
  fi
  target_generation="$cache_root/$target_key"
  case "$(uname -s)" in
    Darwin) /bin/cp -cRp "$source_generation" "$target_generation" ;;
    Linux) /bin/cp -a --reflink=auto "$source_generation" "$target_generation" ;;
    *) printf 'generated-workflows: unsupported host for warm mutant cache setup\n' >&2; return 2 ;;
  esac
  chmod u+w "$target_generation/manifest"
  python3 -I -S - "$target_generation/manifest" "$target_key" "$mutant_checker" <<'PY'
import sys
from pathlib import Path

path = Path(sys.argv[1])
target_key, mutant_checker = sys.argv[2:]
fields = {}
for line in path.read_text().splitlines():
    key, separator, value = line.partition("=")
    if not separator or key in fields:
        raise SystemExit("generated-workflows: copied warm cache manifest is malformed")
    fields[key] = value
fields["key"] = target_key
fields["checker_sha256"] = mutant_checker
path.write_text("".join(f"{key}={value}\n" for key, value in fields.items()))
PY
  chmod 400 "$target_generation/manifest"
  tool_cache_generation=$target_generation
}
source_common=$("$git_command" -C "$repo_root" rev-parse --path-format=absolute --git-common-dir)
source_common=$(cd "$source_common" && pwd -P)
guard_install_root=${BASELINE_GUARD_INSTALL_ROOT:-$source_common/jackin-generated-workflow-guard}
if [[ "$cold_install" -eq 0 && ( -L "$guard_install_root" || ! -d "$guard_install_root" ) ]]; then
  printf 'generated-workflows: run bash scripts/install-generated-workflow-guard.sh before this real-toolchain qualification\n' >&2
  exit 2
fi

test_root=$(mktemp -d "${TMPDIR:-/tmp}/generated-workflow-e2e.XXXXXX")
evidence_base=${BASELINE_GUARD_EVIDENCE_DIR:-}
network_enforcement=not-available
cleanup_e2e() {
  local evidence_run=
  local name
  if [[ -n "$evidence_base" ]]; then
    mkdir -p "$evidence_base"
    if [[ -L "$evidence_base" || ! -d "$evidence_base" ]]; then
      printf 'generated-workflows: evidence directory is unsafe\n' >&2
      return 1
    fi
    evidence_run=$(mktemp -d "$evidence_base/qualification.XXXXXX") || return $?
    for name in "$test_root"/*.log "$test_root"/*.stdout; do
      if [[ -f "$name" && ! -L "$name" ]]; then
        cp -p "$name" "$evidence_run/"
      fi
    done
    if [[ -f "$test_root/network-deny.sb" ]]; then
      cp -p "$test_root/network-deny.sb" "$evidence_run/"
    fi
    if [[ -f "$test_root/network-control.result" ]]; then
      cp -p "$test_root/network-control.result" "$evidence_run/"
    fi
    mkdir "$evidence_run/cache"
    if [[ -n "${installation:-}" && -n "${checker_digest:-}" && -f "$installation/versions/$checker_digest/manifest" ]]; then
      cp -p "$installation/versions/$checker_digest/manifest" "$evidence_run/cache/installed-checker.manifest"
    fi
    if [[ -n "${installation:-}" && -f "$installation/cache/seed/a7413118476128a1c9b4f290277eb91ddf64ea7a/manifest" ]]; then
      cp -p "$installation/cache/seed/a7413118476128a1c9b4f290277eb91ddf64ea7a/manifest" "$evidence_run/cache/seed.manifest"
      cp -p "$installation/cache/seed/a7413118476128a1c9b4f290277eb91ddf64ea7a/objects.txt" "$evidence_run/cache/seed.objects"
    fi
    if [[ -z "${tool_cache_generation:-}" && -n "${installation:-}" && -d "$installation/cache/tools" ]]; then
      tool_cache_generation=$(find "$installation/cache/tools" -mindepth 1 -maxdepth 1 -type d ! -name '.stage*' -print -quit)
    fi
    if [[ -n "${tool_cache_generation:-}" && -f "$tool_cache_generation/manifest" ]]; then
      cp -p "$tool_cache_generation/manifest" "$evidence_run/cache/tools.manifest"
      cp -p "$tool_cache_generation/tool-state/mise-config/mise.toml" "$evidence_run/cache/tools-mise.toml"
      cp -p "$tool_cache_generation/tool-state/mise-config/mise.lock" "$evidence_run/cache/tools-mise.lock"
    fi
    if [[ -n "${python_artifact_dir:-}" && -f "$python_artifact_dir/manifest" ]]; then
      cp -p "$python_artifact_dir/manifest" "$evidence_run/cache/python.manifest"
      cp -p "$python_artifact_dir/mise.toml" "$evidence_run/cache/python-mise.toml"
      cp -p "$python_artifact_dir/mise.lock" "$evidence_run/cache/python-mise.lock"
      cp -p "$python_artifact_dir/inventory" "$evidence_run/cache/python.inventory"
    fi
    evidence_source_head=unavailable
    evidence_seed_state=unavailable
    if [[ -n "${repository:-}" && -d "$repository" ]]; then
      evidence_source_head=$("$git_command" -C "$repository" rev-parse HEAD 2>/dev/null || true)
      if "$git_command" -C "$repository" cat-file -e 'a7413118476128a1c9b4f290277eb91ddf64ea7a^{commit}' 2>/dev/null; then
        evidence_seed_state=present
      else
        evidence_seed_state=absent
      fi
    fi
    {
      printf 'source_head=%s\n' "$evidence_source_head"
      printf 'approved_seed_in_shallow_source=%s\n' "$evidence_seed_state"
      printf 'checker_sha256=%s\n' "${checker_digest:-unavailable}"
      printf 'platform=%s\n' "${platform:-unavailable}"
      printf 'network_enforcement=%s\n' "$network_enforcement"
    } > "$evidence_run/qualification.txt"
    printf 'generated-workflows: preserved real-toolchain evidence at %s\n' "$evidence_run"
  fi
  find "$test_root" -type d -exec chmod u+rwx {} + || {
    printf 'generated-workflows: cannot restore owned temporary directory permissions for cleanup\n' >&2
    return 1
  }
  find "$test_root" -depth -delete || {
    printf 'generated-workflows: cannot remove the owned temporary test tree\n' >&2
    return 1
  }
  if [[ -e "$test_root" || -L "$test_root" ]]; then
    printf 'generated-workflows: temporary test root remains after cleanup\n' >&2
    return 1
  fi
}
trap cleanup_e2e EXIT
repository="$test_root/repository with spaces"
remote="$test_root/remote.git"
home_dir="$test_root/home"
hooks_path="$test_root/custom hooks"
attacker_marker="$test_root/attacker executed"
mkdir -p "$repository" "$home_dir"
: > "$test_root/case-results.log"
"$git_command" init --quiet --initial-branch=main "$repository"
"$git_command" -C "$repository" fetch --quiet --depth=1 --no-tags "file://$repo_root" "$("$git_command" -C "$repo_root" rev-parse HEAD)"
"$git_command" -C "$repository" checkout --quiet -B main FETCH_HEAD
if "$git_command" -C "$repository" cat-file -e 'a7413118476128a1c9b4f290277eb91ddf64ea7a^{commit}' 2>/dev/null; then
  printf 'generated-workflows: shallow fixture unexpectedly contains the approved seed commit\n' >&2
  exit 1
fi
"$git_command" -C "$repository" config user.name 'Generated workflow qualification'
"$git_command" -C "$repository" config user.email 'generated-workflow@example.invalid'
"$git_command" -C "$repository" config core.hooksPath "$hooks_path"

for input in check-generated-workflows.sh generated-workflow-guard-config.toml python-inventory.sh trusted-seed.commit trusted-seed-metadata.pack trusted-seed-metadata.objects python-mise.toml python-mise.macos-arm64.lock.toml python-mise.linux-x64.lock.toml install-generated-workflow-guard.sh; do
  cp -p "$repo_root/scripts/$input" "$repository/scripts/$input"
done
cp -p "$repository/scripts/check-generated-workflows.sh" "$test_root/checkout-checker.original"
original_checker_sha256=$(sha256_file "$repository/scripts/check-generated-workflows.sh")
if [[ "$public_mutant" == relative-git-path ]]; then
  (cd "$repository" && /usr/bin/patch --quiet -p1 < "$repo_root/tests/mutants/relative-git-path.patch")
  mutant_checker_sha256=$(sha256_file "$repository/scripts/check-generated-workflows.sh")
  /usr/bin/sed "s/^reviewed_checker_sha256=.*/reviewed_checker_sha256=$mutant_checker_sha256/" "$repository/scripts/install-generated-workflow-guard.sh" > "$test_root/mutant-installer"
  mv "$test_root/mutant-installer" "$repository/scripts/install-generated-workflow-guard.sh"
fi
common_dir=$("$git_command" -C "$repository" rev-parse --path-format=absolute --git-common-dir)
common_dir=$(cd "$common_dir" && pwd -P)
installation="$common_dir/jackin-generated-workflow-guard"
case "$(uname -s)" in
  Darwin)
    if [[ "$cold_install" -eq 0 ]]; then /bin/cp -cRp "$guard_install_root" "$installation"; fi
    ;;
  Linux)
    if [[ "$cold_install" -eq 0 ]]; then /bin/cp -a --reflink=auto "$guard_install_root" "$installation"; fi
    ;;
  *) printf 'generated-workflows: unsupported host for the real-toolchain qualification\n' >&2; exit 2 ;;
esac
if [[ "$public_mutant" == relative-git-path ]]; then
  rekey_mutant_tool_cache "$installation/cache/tools" "$original_checker_sha256" "$mutant_checker_sha256"
fi
"$git_command" init --quiet --bare "$remote"
source_head=$("$git_command" -C "$repo_root" rev-parse HEAD)
"$git_command" -C "$remote" fetch --quiet "file://$repo_root" "$source_head"
"$git_command" -C "$remote" update-ref refs/heads/main FETCH_HEAD
"$git_command" -C "$repository" remote add origin "$remote"
if ! (cd "$repository" && HOME="$home_dir" /bin/bash scripts/install-generated-workflow-guard.sh) > "$test_root/setup.log" 2>&1; then
  cat "$test_root/setup.log" >&2
  printf 'generated-workflows: real installed checker setup failed\n' >&2
  exit 1
fi
printf 'private installed checker setup: PASS\n' >> "$test_root/case-results.log"

checker_digest=$(sha256_file "$repository/scripts/check-generated-workflows.sh")
installed_manifest="$installation/versions/$checker_digest/manifest"
manifest_value() { /usr/bin/sed -n "s/^$1=//p" "$installed_manifest"; }
if [[ "$hash_tool" == /usr/bin/shasum ]]; then
  python_generation=$(printf '%s\n' "$platform" "$(manifest_value python_version)" "$(manifest_value python_artifact_sha256)" "$(manifest_value python_lock_sha256)" "$(manifest_value python_mise_sha256)" "$(manifest_value inventory_helper_sha256)" "$(manifest_value mise_version)" "$(manifest_value mise_sha256)" | "$hash_tool" -a 256 | /usr/bin/awk '{print $1}')
else
  python_generation=$(printf '%s\n' "$platform" "$(manifest_value python_version)" "$(manifest_value python_artifact_sha256)" "$(manifest_value python_lock_sha256)" "$(manifest_value python_mise_sha256)" "$(manifest_value inventory_helper_sha256)" "$(manifest_value mise_version)" "$(manifest_value mise_sha256)" | "$hash_tool" | /usr/bin/awk '{print $1}')
fi
python_artifact_dir="$installation/artifacts/python/3.15.2/$platform-$python_generation"
trusted_checker="$installation/versions/$checker_digest/check-generated-workflows.sh"
if [[ ! -f "$trusted_checker" || -L "$trusted_checker" ]]; then
  printf 'generated-workflows: installed trusted checker is unavailable after setup\n' >&2
  exit 1
fi
if [[ "$e2e_focus" == install ]]; then
  printf 'cold or warm public installer and real Velnor qualification: PASS\n' >> "$test_root/case-results.log"
  exit 0
fi

relative_git_directory='relative git tools'
mkdir "$repository/$relative_git_directory"
ln -s "$git_command" "$repository/$relative_git_directory/git"
cat > "$test_root/staged git sentinel" <<EOF
#!/bin/bash
/bin/touch '$attacker_marker'
exit 93
EOF
chmod 755 "$test_root/staged git sentinel"
sentinel_oid=$("$git_command" -C "$repository" hash-object -w --stdin < "$test_root/staged git sentinel")
"$git_command" -C "$repository" update-index --add --cacheinfo "100755,$sentinel_oid,$relative_git_directory/git"
relative_index=$("$git_command" -C "$repository" rev-parse --path-format=absolute --git-path index)
relative_index_before=$(sha256_file "$relative_index")
relative_link_before=$(readlink "$repository/$relative_git_directory/git")
relative_objects_before=$("$git_command" -C "$repository" cat-file --batch-all-objects --batch-check='%(objectname)' | /usr/bin/sort)
relative_checker_status=0
if (cd "$repository" && HOME="$home_dir" PATH="$relative_git_directory:/usr/bin:/bin:/usr/sbin:/sbin" /bin/bash "$trusted_checker" pre-commit) > "$test_root/relative-git-match.log" 2>&1; then
  relative_checker_status=0
else
  relative_checker_status=$?
fi
if [[ "$public_mutant" == relative-git-path ]]; then
  if [[ "$relative_checker_status" -eq 0 || ! -e "$attacker_marker" ]]; then
    cat "$test_root/relative-git-match.log" >&2
    printf 'generated-workflows: relative Git path mutant did not execute the staged sentinel at final comparison\n' >&2
    exit 1
  fi
  printf 'relative Git path mutant: DETECTED at staged final-comparison sentinel\n' >> "$test_root/case-results.log"
  printf 'relative Git path mutant: DETECTED at staged final-comparison sentinel\n'
  exit 0
fi
if [[ "$relative_checker_status" -ne 0 ]]; then
  cat "$test_root/relative-git-match.log" >&2
  printf 'generated-workflows: installed checker failed a matching snapshot with a relative caller PATH\n' >&2
  exit 1
fi
if [[ -e "$attacker_marker" ]]; then
  printf 'generated-workflows: staged git sentinel executed during a matching comparison\n' >&2
  exit 1
fi
if [[ "$(sha256_file "$relative_index")" != "$relative_index_before" || "$(readlink "$repository/$relative_git_directory/git")" != "$relative_link_before" || "$("$git_command" -C "$repository" cat-file --batch-all-objects --batch-check='%(objectname)' | /usr/bin/sort)" != "$relative_objects_before" ]]; then
  printf 'generated-workflows: relative Git path comparison changed source index, worktree link, or Git objects\n' >&2
  exit 1
fi
printf 'relative PATH staged executable Git sentinel reaches matching real generator comparison: PASS, not executed\n' >> "$test_root/case-results.log"
printf 'relative PATH staged executable Git sentinel reaches matching real generator comparison: PASS, not executed\n'
if [[ "$e2e_focus" == relative-git-path ]]; then
  exit 0
fi
if ! (cd "$repository/docs" && HOME="$home_dir" GIT_INDEX_FILE=../.git/index /bin/bash "$trusted_checker" pre-commit) > "$test_root/relative-index-match.log" 2>&1; then
  cat "$test_root/relative-index-match.log" >&2
  printf 'generated-workflows: relative effective index was not resolved from its caller directory\n' >&2
  exit 1
fi
printf 'relative effective index from subdirectory: PASS\n' >> "$test_root/case-results.log"
space_git_dir="$test_root/git tools"
mkdir "$space_git_dir"
ln -s "$git_command" "$space_git_dir/git"
if ! (cd "$repository" && HOME="$home_dir" PATH="$space_git_dir:/usr/bin:/bin:/usr/sbin:/sbin" /bin/bash "$trusted_checker" pre-commit) > "$test_root/git-path-spaces.log" 2>&1; then
  cat "$test_root/git-path-spaces.log" >&2
  printf 'generated-workflows: installed checker rejected a caller Git executable path containing spaces\n' >&2
  exit 1
fi
printf 'installed checker with Git executable path containing spaces: PASS\n' >> "$test_root/case-results.log"
if [[ "$e2e_focus" == git-path-spaces ]]; then
  exit 0
fi
if [[ "$(uname -s)" == Darwin && -x /usr/bin/sandbox-exec ]]; then
  network_profile='(version 1)(allow default)(deny network*)'
  printf '%s\n' "$network_profile" > "$test_root/network-deny.sb"
  if /usr/bin/sandbox-exec -p "$network_profile" /usr/bin/env -i PATH=/usr/bin:/bin HOME=/tmp /usr/bin/curl --disable --noproxy '*' --connect-timeout 2 --max-time 4 --silent --show-error https://1.1.1.1/ > "$test_root/network-control.stdout" 2> "$test_root/network-control.log"; then
    printf 'generated-workflows: network-denied curl positive control unexpectedly connected\n' >&2
    exit 1
  else
    network_control_status=$?
  fi
  printf 'curl_exit=%s\n' "$network_control_status" > "$test_root/network-control.result"
  network_enforcement=macos-sandbox-exec-deny-network
  for warm_run in 1 2; do
    if ! (cd "$repository" && HOME="$home_dir" /usr/bin/sandbox-exec -p "$network_profile" /bin/bash "$trusted_checker" pre-commit) > "$test_root/offline-warm-$warm_run.log" 2>&1; then
      cat "$test_root/offline-warm-$warm_run.log" >&2
      printf 'generated-workflows: warmed installed checker failed with network access denied\n' >&2
      exit 1
    fi
  done
  printf 'two warm installed pre-commit runs under network-denied sandbox: PASS\n' >> "$test_root/case-results.log"
fi
printf 'name: intentional drift\n' > "$repository/.github/workflows/intentional-drift.yml"
"$git_command" -C "$repository" add -- .github/workflows/intentional-drift.yml
if (cd "$repository" && HOME="$home_dir" PATH="$relative_git_directory:/usr/bin:/bin:/usr/sbin:/sbin" /bin/bash "$trusted_checker" pre-commit) > "$test_root/relative-git-drift.log" 2>&1; then
  printf 'generated-workflows: installed checker accepted drift with a relative caller PATH\n' >&2
  exit 1
fi
if ! /usr/bin/grep -F -q 'generated-workflows: drift in index snapshot' "$test_root/relative-git-drift.log" || [[ -e "$attacker_marker" ]]; then
  cat "$test_root/relative-git-drift.log" >&2
  printf 'generated-workflows: relative PATH drift case failed before the real generator comparison\n' >&2
  exit 1
fi
printf 'relative PATH drift reached real generator comparison: REJECTED\n' >> "$test_root/case-results.log"
"$git_command" -C "$repository" reset --quiet HEAD -- .github/workflows/intentional-drift.yml
rm "$repository/.github/workflows/intentional-drift.yml"

if ! HOME="$home_dir" "$git_command" -C "$repository" commit --allow-empty --quiet -m generated-workflow-match; then
  printf 'generated-workflows: real Velnor rejected matching generated workflows\n' >&2
  exit 1
fi
printf 'matching real commit hook: PASS\n' >> "$test_root/case-results.log"
push_main_and_verify() {
  local description=$1
  local push_log=$2
  if ! HOME="$home_dir" "$git_command" -C "$repository" push --quiet origin main >"$push_log" 2>&1; then
    cat "$push_log" >&2
    if /usr/bin/grep -E -q 'shallow update not allowed|deny updating a hidden ref|unpacker error' "$push_log"; then
      printf 'generated-workflows: receiver rejected %s (pre-push hook result is recorded separately)\n' "$description" >&2
    else
      printf 'generated-workflows: pre-push hook or transport rejected %s\n' "$description" >&2
    fi
    return 1
  fi
  local client_head
  local receiver_head
  client_head=$("$git_command" -C "$repository" rev-parse HEAD)
  receiver_head=$("$git_command" -C "$remote" rev-parse refs/heads/main)
  if [[ "$receiver_head" != "$client_head" ]]; then
    printf 'generated-workflows: receiver ref %s does not match client HEAD\n' "$receiver_head" >&2
    return 1
  fi
}
if ! push_main_and_verify 'matching generated workflows' "$test_root/matching-push.log"; then
  printf 'generated-workflows: matching pre-push hook and receiver update did not both succeed\n' >&2
  exit 1
fi
printf 'matching real pre-push hook: PASS\n' >> "$test_root/case-results.log"

cat > "$repository/scripts/branch-hk-sentinel.sh" <<EOF
#!/bin/bash
/bin/touch '$attacker_marker'
EOF
chmod 755 "$repository/scripts/branch-hk-sentinel.sh"
cat > "$repository/hk.pkl" <<'EOF'
amends "package://github.com/jdx/hk/releases/download/v2.11.1/hk@2.11.1#/Config.pkl"
hooks {
  ["pre-commit"] {
    steps {
      ["hostile"] {
        check = "bash scripts/branch-hk-sentinel.sh"
      }
    }
  }
}
EOF
cp "$repository/hk.pkl" "$repository/hk.local.pkl"
mkdir -p "$home_dir/.config/hk"
cp "$repository/hk.pkl" "$home_dir/.config/hk/hk.pkl"
cat > "$repository/scripts/check-generated-workflows.sh" <<EOF
#!/bin/bash
/bin/touch '$attacker_marker'
exit 94
EOF
chmod 755 "$repository/scripts/check-generated-workflows.sh"
"$git_command" -C "$repository" add -- scripts/branch-hk-sentinel.sh hk.pkl hk.local.pkl scripts/check-generated-workflows.sh
if ! HOME="$home_dir" "$git_command" -C "$repository" commit --allow-empty --quiet -m hostile-checkout-configuration; then
  printf 'generated-workflows: installed checker rejected matching hostile checkout and user hk configuration\n' >&2
  exit 1
fi
if [[ -e "$attacker_marker" ]]; then
  printf 'generated-workflows: checkout or user hk configuration executed a sentinel\n' >&2
  exit 1
fi
if ! push_main_and_verify 'hostile checkout and user hk configuration' "$test_root/hostile-push.log"; then
  printf 'generated-workflows: hostile checkout pre-push hook and receiver update did not both succeed\n' >&2
  exit 1
fi
if [[ -e "$attacker_marker" ]]; then
  printf 'generated-workflows: checkout or user hk configuration executed during push\n' >&2
  exit 1
fi

printf 'selected change\n' > "$repository/docs/selected with spaces.md"
"$git_command" -C "$repository" add -- 'docs/selected with spaces.md'
printf 'name: staged invalid workflow\n' > "$repository/.github/workflows/staged-invalid.yml"
"$git_command" -C "$repository" add -- .github/workflows/staged-invalid.yml
if ! HOME="$home_dir" "$git_command" -C "$repository" commit --only --quiet -m commit-only-selected -- 'docs/selected with spaces.md'; then
  printf 'generated-workflows: commit --only validated the ordinary index instead of its effective selected index\n' >&2
  exit 1
fi
if [[ "$("$git_command" -C "$repository" diff --cached --name-only)" != .github/workflows/staged-invalid.yml ]]; then
  printf 'generated-workflows: commit --only changed an unrelated staged workflow\n' >&2
  exit 1
fi
printf 'commit --only selected valid path with unrelated staged drift: PASS\n' >> "$test_root/case-results.log"
"$git_command" -C "$repository" reset --quiet HEAD -- .github/workflows/staged-invalid.yml
rm "$repository/.github/workflows/staged-invalid.yml"

"$git_command" -C "$repository" add -- .github/workflows/ci.yml
printf 'name: selected invalid workflow\n' > "$repository/.github/workflows/ci.yml"
if HOME="$home_dir" "$git_command" -C "$repository" commit --only --quiet -m commit-only-invalid -- .github/workflows/ci.yml > "$test_root/commit-only-invalid.log" 2>&1; then
  printf 'generated-workflows: commit --only accepted an invalid selected snapshot\n' >&2
  exit 1
fi
if ! /usr/bin/grep -F -q 'generated-workflows: drift in index snapshot' "$test_root/commit-only-invalid.log"; then
  cat "$test_root/commit-only-invalid.log" >&2
  printf 'generated-workflows: invalid commit --only case failed before generator comparison\n' >&2
  exit 1
fi
printf 'commit --only selected invalid workflow: REJECTED by real generator comparison\n' >> "$test_root/case-results.log"
"$git_command" -C "$repository" reset --quiet HEAD -- .github/workflows/ci.yml
"$git_command" -C "$repository" checkout --quiet -- .github/workflows/ci.yml

missing_index="$test_root/missing explicit index"
if (cd "$repository" && HOME="$home_dir" GIT_INDEX_FILE="$missing_index" /bin/bash "$trusted_checker" pre-commit) > "$test_root/missing-index.log" 2>&1; then
  printf 'generated-workflows: missing explicit effective index was accepted\n' >&2
  exit 1
fi
if ! /usr/bin/grep -F -q 'effective index is missing or not a regular file' "$test_root/missing-index.log"; then
  cat "$test_root/missing-index.log" >&2
  printf 'generated-workflows: missing explicit index failed at an unexpected boundary\n' >&2
  exit 1
fi
mkdir "$test_root/invalid explicit index"
if (cd "$repository" && HOME="$home_dir" GIT_INDEX_FILE="$test_root/invalid explicit index" /bin/bash "$trusted_checker" pre-commit) > "$test_root/invalid-index.log" 2>&1; then
  printf 'generated-workflows: non-file explicit effective index was accepted\n' >&2
  exit 1
fi
if ! /usr/bin/grep -F -q 'effective index is missing or not a regular file' "$test_root/invalid-index.log"; then
  cat "$test_root/invalid-index.log" >&2
  printf 'generated-workflows: invalid explicit index failed at an unexpected boundary\n' >&2
  exit 1
fi

"$git_command" -C "$repository" config core.splitIndex true
"$git_command" -C "$repository" update-index --split-index
linked_repository="$test_root/linked worktree"
"$git_command" -C "$repository" worktree add --quiet -b linked-qualification "$linked_repository" main
linked_git_directory=$("$git_command" -C "$linked_repository" rev-parse --path-format=absolute --git-dir)
linked_index=$("$git_command" -C "$linked_repository" rev-parse --path-format=absolute --git-path index)
linked_shared=$(GIT_INDEX_FILE="$linked_index" "$git_command" -C "$linked_repository" rev-parse --shared-index-path)
if [[ -z "$linked_shared" ]]; then
  printf 'generated-workflows: linked worktree fixture did not create a split index\n' >&2
  exit 1
fi
if [[ "$linked_shared" != /* ]]; then
  linked_shared="$linked_repository/$linked_shared"
fi
linked_index_before=$(sha256_file "$linked_index")
linked_shared_before=$(sha256_file "$linked_shared")
if ! (cd "$linked_repository" && HOME="$home_dir" "$hooks_path/pre-commit"); then
  printf 'generated-workflows: linked worktree split-index hook failed\n' >&2
  exit 1
fi
if [[ "$linked_index_before" != "$(sha256_file "$linked_index")" || "$linked_shared_before" != "$(sha256_file "$linked_shared")" ]]; then
  printf 'generated-workflows: linked worktree split-index hook changed its source index or base\n' >&2
  exit 1
fi
printf 'linked worktree split index unchanged after hook: PASS\n' >> "$test_root/case-results.log"
custom_index="$linked_git_directory/index with spaces"
cp -p "$linked_index" "$custom_index"
custom_shared=$(GIT_INDEX_FILE="$custom_index" "$git_command" -C "$linked_repository" rev-parse --shared-index-path)
if [[ "$custom_shared" != /* ]]; then
  custom_shared="$linked_repository/$custom_shared"
fi
custom_index_before=$(sha256_file "$custom_index")
custom_shared_before=$(sha256_file "$custom_shared")
if ! (cd "$linked_repository" && HOME="$home_dir" GIT_INDEX_FILE="$custom_index" "$hooks_path/pre-commit"); then
  printf 'generated-workflows: custom split-index hook failed\n' >&2
  exit 1
fi
if [[ "$custom_index_before" != "$(sha256_file "$custom_index")" || "$custom_shared_before" != "$(sha256_file "$custom_shared")" ]]; then
  printf 'generated-workflows: custom split-index hook changed its source index or base\n' >&2
  exit 1
fi
printf 'custom split index with spaces unchanged after hook: PASS\n' >> "$test_root/case-results.log"
linked_config=$("$git_command" -C "$linked_repository" rev-parse --path-format=absolute --git-common-dir)/config
linked_worktree_config="$linked_git_directory/config.worktree"
linked_head_before=$("$git_command" -C "$linked_repository" rev-parse HEAD)
linked_refs_before=$("$git_command" -C "$linked_repository" show-ref --head 2>/dev/null || true)
linked_index_before_missing=$(sha256_file "$linked_index")
linked_config_before=$(sha256_file "$linked_config")
if [[ -f "$linked_worktree_config" ]]; then
  linked_worktree_config_before=$(sha256_file "$linked_worktree_config")
else
  linked_worktree_config_before=absent
fi
linked_objects_before=$("$git_command" -C "$linked_repository" cat-file --batch-all-objects --batch-check='%(objectname) %(objecttype) %(objectsize)' | /usr/bin/sort)
linked_shared_before_missing=$(sha256_file "$linked_shared")
mv "$linked_shared" "$test_root/linked shared index backup"
if [[ "$network_enforcement" == macos-sandbox-exec-deny-network ]]; then
  if (cd "$linked_repository" && HOME="$home_dir" /usr/bin/sandbox-exec -p "$network_profile" /bin/bash "$hooks_path/pre-commit") > "$test_root/missing-shared-index.log" 2>&1; then
    printf 'generated-workflows: missing split-index base was accepted\n' >&2
    exit 1
  fi
elif (cd "$linked_repository" && HOME="$home_dir" "$hooks_path/pre-commit") > "$test_root/missing-shared-index.log" 2>&1; then
  printf 'generated-workflows: missing split-index base was accepted\n' >&2
  exit 1
fi
if ! /usr/bin/grep -F -q 'generated-workflows: cannot resolve the effective split-index base' "$test_root/missing-shared-index.log" && ! /usr/bin/grep -F -q 'generated-workflows: effective split-index base is unavailable' "$test_root/missing-shared-index.log"; then
  cat "$test_root/missing-shared-index.log" >&2
  printf 'generated-workflows: missing split-index base was not rejected at source resolution\n' >&2
  exit 1
fi
if [[ -e "$linked_shared" || -L "$linked_shared" || "$(sha256_file "$linked_index")" != "$linked_index_before_missing" || "$(sha256_file "$linked_config")" != "$linked_config_before" || "$("$git_command" -C "$linked_repository" rev-parse HEAD)" != "$linked_head_before" || "$("$git_command" -C "$linked_repository" show-ref --head 2>/dev/null || true)" != "$linked_refs_before" || "$("$git_command" -C "$linked_repository" cat-file --batch-all-objects --batch-check='%(objectname) %(objecttype) %(objectsize)' | /usr/bin/sort)" != "$linked_objects_before" ]]; then
  printf 'generated-workflows: missing split-index refusal changed source index, configuration, refs, or Git objects\n' >&2
  exit 1
fi
if [[ "$linked_worktree_config_before" == absent ]]; then
  if [[ -e "$linked_worktree_config" || -L "$linked_worktree_config" ]]; then
    printf 'generated-workflows: missing split-index refusal created worktree configuration\n' >&2
    exit 1
  fi
elif [[ ! -f "$linked_worktree_config" || "$(sha256_file "$linked_worktree_config")" != "$linked_worktree_config_before" ]]; then
  printf 'generated-workflows: missing split-index refusal changed worktree configuration\n' >&2
  exit 1
fi
printf 'missing split-index base: REJECTED with source state unchanged\n' >> "$test_root/case-results.log"
mv "$test_root/linked shared index backup" "$linked_shared"
if [[ "$(sha256_file "$linked_shared")" != "$linked_shared_before_missing" ]]; then
  printf 'generated-workflows: restored split-index base differs from its original bytes\n' >&2
  exit 1
fi
if ! (cd "$linked_repository" && HOME="$home_dir" "$hooks_path/pre-commit") > "$test_root/restored-shared-index.log" 2>&1; then
  cat "$test_root/restored-shared-index.log" >&2
  printf 'generated-workflows: restored split-index base did not pass the public hook\n' >&2
  exit 1
fi
if [[ "$(sha256_file "$linked_index")" != "$linked_index_before_missing" || "$(sha256_file "$linked_shared")" != "$linked_shared_before_missing" ]]; then
  printf 'generated-workflows: restored split-index hook changed its source index or base\n' >&2
  exit 1
fi
printf 'restored split-index base: public hook PASS with index and base unchanged\n' >> "$test_root/case-results.log"

printf 'name: intentional drift\n' > "$repository/.github/workflows/intentional-drift.yml"
"$git_command" -C "$repository" add -- .github/workflows/intentional-drift.yml
if HOME="$home_dir" "$git_command" -C "$repository" commit --quiet -m generated-workflow-drift > "$test_root/drift.log" 2>&1; then
  printf 'generated-workflows: real Velnor accepted a generated-workflow drift\n' >&2
  exit 1
fi
if ! /usr/bin/grep -F -q 'generated-workflows: drift in index snapshot' "$test_root/drift.log"; then
  cat "$test_root/drift.log" >&2
  printf 'generated-workflows: drift case failed before the real generator comparison\n' >&2
  exit 1
fi
printf 'staged workflow drift reached real generator comparison: REJECTED\n' >> "$test_root/case-results.log"

tool_cache_generation=$(find "$installation/cache/tools" -mindepth 1 -maxdepth 1 -type d ! -name '.stage*' -print -quit)
if [[ -z "$tool_cache_generation" || ! -f "$tool_cache_generation/tool-state/mise-config/mise.lock" ]]; then
  printf 'generated-workflows: qualified tool cache is unavailable for the integrity case\n' >&2
  exit 1
fi
tool_lock="$tool_cache_generation/tool-state/mise-config/mise.lock"
cp -p "$tool_lock" "$test_root/tool-lock.original"
chmod u+w "$tool_lock"
printf '%s\n' 'invalid = true' >> "$tool_lock"
if (cd "$repository" && HOME="$home_dir" "$hooks_path/pre-commit") > "$test_root/corrupt-tool-cache.log" 2>&1; then
  printf 'generated-workflows: corrupted persistent tool cache was accepted\n' >&2
  exit 1
fi
if ! /usr/bin/grep -F -q 'qualified tool cache payload changed; refusing to execute it' "$test_root/corrupt-tool-cache.log"; then
  cat "$test_root/corrupt-tool-cache.log" >&2
  printf 'generated-workflows: corrupt tool cache failed at an unexpected boundary\n' >&2
  exit 1
fi
printf 'corrupt persistent tool cache: REJECTED before execution\n' >> "$test_root/case-results.log"
cp -p "$test_root/tool-lock.original" "$tool_lock"

python_inventory="$python_artifact_dir/inventory"
cp -p "$python_inventory" "$test_root/python-inventory.original"
chmod u+w "$python_inventory"
printf '%s\n' 'invalid inventory' >> "$python_inventory"
if (cd "$repository" && HOME="$home_dir" "$hooks_path/pre-commit") > "$test_root/corrupt-python-inventory.log" 2>&1; then
  printf 'generated-workflows: corrupted private Python inventory was accepted\n' >&2
  exit 1
fi
if ! /usr/bin/grep -F -q 'pinned Python 3.15.2 or its inventory is missing or changed' "$test_root/corrupt-python-inventory.log"; then
  cat "$test_root/corrupt-python-inventory.log" >&2
  printf 'generated-workflows: corrupt Python inventory failed at an unexpected boundary\n' >&2
  exit 1
fi
printf 'corrupt private Python inventory: REJECTED before interpreter execution\n' >> "$test_root/case-results.log"
cp -p "$test_root/python-inventory.original" "$python_inventory"

trusted_config="$installation/versions/$checker_digest/config.toml"
cp -p "$trusted_config" "$test_root/trusted-config.original"
chmod u+w "$trusted_config"
printf '%s' 'tampered=true' >> "$trusted_config"
if (cd "$repository" && HOME="$home_dir" "$hooks_path/pre-commit") > "$test_root/tampered-config.log" 2>&1; then
  printf 'generated-workflows: altered installed checker configuration was accepted\n' >&2
  exit 1
fi
if ! /usr/bin/grep -F -q 'installed checker artifacts failed integrity validation' "$test_root/tampered-config.log"; then
  cat "$test_root/tampered-config.log" >&2
  printf 'generated-workflows: altered installed configuration failed at an unexpected boundary\n' >&2
  exit 1
fi
printf 'tampered installed configuration: REJECTED before checker execution\n' >> "$test_root/case-results.log"
cp -p "$test_root/trusted-config.original" "$trusted_config"

cp -p "$trusted_checker" "$test_root/trusted-checker.untampered"
chmod u+w "$trusted_checker"
printf '%s' 'tampered checker' >> "$trusted_checker"
if (cd "$repository" && HOME="$home_dir" "$hooks_path/pre-commit") > "$test_root/tampered-checker.log" 2>&1; then
  printf 'generated-workflows: altered installed checker was accepted\n' >&2
  exit 1
fi
if ! /usr/bin/grep -F -q 'trusted checker failed its launcher integrity check' "$test_root/tampered-checker.log"; then
  cat "$test_root/tampered-checker.log" >&2
  printf 'generated-workflows: altered installed checker failed at an unexpected boundary\n' >&2
  exit 1
fi
printf 'tampered installed checker: REJECTED by launcher digest\n' >> "$test_root/case-results.log"
cp -p "$test_root/trusted-checker.untampered" "$trusted_checker"

checker_version_dir=${trusted_checker%/*}
case "$(uname -s)" in
  Darwin)
    checker_version_mode=$(/usr/bin/stat -f '%Lp' "$checker_version_dir")
    checker_version_mode_now() { /usr/bin/stat -f '%Lp' "$checker_version_dir"; }
    ;;
  Linux)
    checker_version_mode=$(/usr/bin/stat -c '%a' "$checker_version_dir")
    checker_version_mode_now() { /usr/bin/stat -c '%a' "$checker_version_dir"; }
    ;;
esac
chmod u+w "$checker_version_dir"
mv "$trusted_checker" "$test_root/trusted-checker.original"
ln -s "$test_root/trusted-checker.original" "$trusted_checker"
chmod "$checker_version_mode" "$checker_version_dir"
if [[ "$(checker_version_mode_now)" != "$checker_version_mode" || ! -L "$trusted_checker" ]]; then
  printf 'generated-workflows: symlink fixture did not preserve the installed version directory mode\n' >&2
  exit 1
fi
if (cd "$repository" && HOME="$home_dir" "$hooks_path/pre-commit") > "$test_root/symlink-checker.log" 2>&1; then
  printf 'generated-workflows: symlinked installed checker was accepted\n' >&2
  exit 1
fi
if ! /usr/bin/grep -F -q 'trusted checker installation is unsafe' "$test_root/symlink-checker.log"; then
  cat "$test_root/symlink-checker.log" >&2
  printf 'generated-workflows: symlinked installed checker failed at an unexpected boundary\n' >&2
  exit 1
fi
printf 'symlinked installed checker: REJECTED by launcher boundary\n' >> "$test_root/case-results.log"
chmod u+w "$checker_version_dir"
rm "$trusted_checker"
mv "$test_root/trusted-checker.original" "$trusted_checker"
chmod "$checker_version_mode" "$checker_version_dir"
if [[ "$(checker_version_mode_now)" != "$checker_version_mode" || -L "$trusted_checker" ]]; then
  printf 'generated-workflows: restored checker fixture did not preserve the installed version directory mode\n' >&2
  exit 1
fi

cp -p "$test_root/checkout-checker.original" "$repository/scripts/check-generated-workflows.sh"
foreign_hooks="$test_root/foreign hooks"
mkdir "$foreign_hooks"
printf '#!/bin/sh\nexit 0\n' > "$foreign_hooks/pre-commit"
chmod 755 "$foreign_hooks/pre-commit"
foreign_hook_before=$(sha256_file "$foreign_hooks/pre-commit")
"$git_command" -C "$repository" config core.hooksPath "$foreign_hooks"
if (cd "$repository" && HOME="$home_dir" /bin/bash scripts/install-generated-workflow-guard.sh) > "$test_root/foreign-hook-install.log" 2>&1; then
  printf 'generated-workflows: installer replaced a foreign pre-commit hook\n' >&2
  exit 1
fi
if ! /usr/bin/grep -F -q 'refusing to replace an existing hook' "$test_root/foreign-hook-install.log" || [[ "$foreign_hook_before" != "$(sha256_file "$foreign_hooks/pre-commit")" ]]; then
  cat "$test_root/foreign-hook-install.log" >&2
  printf 'generated-workflows: foreign pre-commit hook was not preserved after explicit refusal\n' >&2
  exit 1
fi
printf 'foreign pre-commit hook: installer refused and preserved bytes\n' >> "$test_root/case-results.log"
printf 'PASS real installed checker accepts matching generated workflows and rejects drift\n'
