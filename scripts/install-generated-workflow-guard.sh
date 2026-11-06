#!/usr/bin/env bash
# SPDX-FileCopyrightText: 2026 Alexey Zhokhov
# SPDX-License-Identifier: Apache-2.0
set +x +a
set -euo pipefail
LC_ALL=C
export LC_ALL
unset trusted_setup_token
trusted_setup_token=${GITHUB_COM_TOKEN-}
unset GITHUB_COM_TOKEN
if [[ -n "$trusted_setup_token" && ( ! "$trusted_setup_token" =~ ^[A-Za-z0-9_]+$ || ${#trusted_setup_token} -gt 512 ) ]]; then
  printf 'generated-workflows: invalid setup credential format\n' >&2
  exit 1
fi
PATH=/usr/bin:/bin:/usr/sbin:/sbin
export PATH

repo_root=$(cd "$(dirname "$0")/.." && pwd -P)
git_command=/usr/bin/git
if [[ ! -x "$git_command" ]]; then
  printf 'generated-workflows: /usr/bin/git is unavailable\n' >&2
  exit 127
fi
case "$(uname -s):$(uname -m)" in
  Darwin:arm64|Darwin:aarch64) hash_tool=/usr/bin/shasum ;;
  Linux:x86_64|Linux:amd64) hash_tool=/usr/bin/sha256sum ;;
  *) printf 'generated-workflows: unsupported host for hook installation\n' >&2; exit 2 ;;
esac
reviewed_checker_sha256=9d2fb0526f89cd58257579876786efeaacc3f88119f2f8cb782b8b0873d5c1eb
reviewed_config_sha256=2a628986b760cb9563ea6bec758269c36d2c56f8875aafc91e14da7ed039146d
reviewed_python_mise_sha256=44598b7976484ccee3703b7d23367d2e2181457370f36790b4f04306098952a9
reviewed_inventory_helper_sha256=0541173812097cd6ac4deb065a7e45b6960ddcf26369eb3fd32805a0b66f33eb
reviewed_seed_commit=a7413118476128a1c9b4f290277eb91ddf64ea7a
reviewed_seed_tree=5d842e61ade8e6bdbdcdc4081b219ddf968d7f0c
reviewed_metadata_pack_sha256=b1451815278c3ca19a9b06908a99d65a9dd42066fae94a46a2ab457a5e1603d0
reviewed_metadata_object_list_sha256=4f811c3e207c19028e9664b5c3e9afa91ed427d1d58b4849c0f742381b006a9f
install_version=1
mise_version=2026.11.3
python_version=3.15.2
case "$(uname -s):$(uname -m)" in
  Darwin:arm64|Darwin:aarch64)
    platform=macos-arm64
    mise_sha256=6d39119ab914c54ab865204dee9337c71b83a7ed1e730f663c2fc3453e38fe9c
    python_artifact_sha256=1b1b03fca8aa351698403f3d100ee83db7a5d99748fc7707681ac31a6a846ad9
    python_lock_sha256=b8653eea3067e6714f12ceead39b1e613700a5d9567d491320ea255bb6d79e4b
    python_lock_file=python-mise.macos-arm64.lock.toml
    ;;
  Linux:x86_64|Linux:amd64)
    platform=linux-x64
    mise_sha256=c2f7e814cb39f60b7f67a0ff36cb805e94128bf2dff09d175309d0f095b8cc3b
    python_artifact_sha256=7b03163a980c607ba8194709d4a4086735a7a9c488cc475b098157306927b36b
    python_lock_sha256=99a7f2363dbff0fe43632c78bfb3c043d96763632af7a8162f1767505e9719e5
    python_lock_file=python-mise.linux-x64.lock.toml
    ;;
esac

sha256_file() {
  if [[ "$hash_tool" == /usr/bin/shasum ]]; then
    "$hash_tool" -a 256 "$1" | awk '{print $1}'
  else
    "$hash_tool" "$1" | awk '{print $1}'
  fi
}

git_clean() {
  env -i PATH=/usr/bin:/bin:/usr/sbin:/sbin HOME="${HOME:-/tmp}" GIT_NO_REPLACE_OBJECTS=1 GIT_NO_LAZY_FETCH=1 "$git_command" -C "$repo_root" "$@"
}

worktree_root=$(git_clean rev-parse --show-toplevel) || {
  printf 'generated-workflows: run setup from a Git worktree\n' >&2
  exit 1
}
worktree_root=$(cd "$worktree_root" && pwd -P)
common_dir=$(git_clean rev-parse --path-format=absolute --git-common-dir) || exit 1
common_dir=$(cd "$common_dir" && pwd -P)
hooks_path=$(git_clean rev-parse --path-format=absolute --git-path hooks) || exit 1
if [[ "$hooks_path" != /* ]]; then
  hooks_path="$worktree_root/$hooks_path"
fi
hooks_path_parent=$(dirname "$hooks_path")
hooks_path_leaf=$(basename "$hooks_path")
if [[ -d "$hooks_path" ]]; then
  hooks_path_logical=$(cd "$hooks_path" && pwd -L) || exit 1
  hooks_path_physical=$(cd "$hooks_path" && pwd -P) || exit 1
elif [[ -d "$hooks_path_parent" ]]; then
  hooks_path_logical=$(cd "$hooks_path_parent" && pwd -L) || exit 1
  hooks_path_physical=$(cd "$hooks_path_parent" && pwd -P) || exit 1
  hooks_path_logical="$hooks_path_logical/$hooks_path_leaf"
  hooks_path_physical="$hooks_path_physical/$hooks_path_leaf"
else
  printf 'generated-workflows: effective hook directory parent is unavailable\n' >&2
  exit 1
fi
if [[ "$hooks_path_logical" != "$hooks_path_physical" ]]; then
  printf 'generated-workflows: effective hook directory traverses a symlink\n' >&2
  exit 1
fi
hooks_path=$hooks_path_physical
case "$hooks_path/" in
  "$worktree_root/"*)
    case "$hooks_path/" in
      "$common_dir/"*) ;;
      *)
        printf 'generated-workflows: configured core.hooksPath is inside the checkout; use a Git metadata hook directory\n' >&2
        exit 1
        ;;
    esac
    ;;
esac
validate_existing_hook() {
  local target=$1
  local mode
  local owner
  if [[ ! -e "$target" && ! -L "$target" ]]; then
    return 0
  fi
  if [[ -L "$target" || ! -f "$target" ]] || ! /usr/bin/grep -F -q 'guard_owner=jackin-generated-workflow-guard' "$target" || ! /usr/bin/grep -F -q "$install_root/versions/" "$target"; then
    printf 'generated-workflows: refusing to replace an existing hook at %s\n' "$target" >&2
    return 1
  fi
  if [[ "$platform" == macos-arm64 ]]; then
    owner=$(/usr/bin/stat -f '%u' "$target")
    mode=$(/usr/bin/stat -f '%Lp' "$target")
  else
    owner=$(/usr/bin/stat -c '%u' "$target")
    mode=$(/usr/bin/stat -c '%a' "$target")
  fi
  if [[ "$owner" != "$(/usr/bin/id -u)" ]] || (( (8#$mode & 022) != 0 )); then
    printf 'generated-workflows: refusing to replace an unsafe existing hook at %s\n' "$target" >&2
    return 1
  fi
}
for path in "$repo_root/scripts/check-generated-workflows.sh" "$repo_root/scripts/generated-workflow-guard-config.toml" "$repo_root/scripts/python-inventory.sh" "$repo_root/scripts/trusted-seed.commit" "$repo_root/scripts/trusted-seed-metadata.pack" "$repo_root/scripts/trusted-seed-metadata.objects" "$repo_root/scripts/python-mise.toml" "$repo_root/scripts/$python_lock_file"; do
  if [[ -L "$path" || ! -f "$path" ]]; then
    printf 'generated-workflows: reviewed installation input is not a regular file\n' >&2
    exit 1
  fi
done
actual_checker_sha256=$(sha256_file "$repo_root/scripts/check-generated-workflows.sh")
actual_config_sha256=$(sha256_file "$repo_root/scripts/generated-workflow-guard-config.toml")
actual_inventory_helper_sha256=$(sha256_file "$repo_root/scripts/python-inventory.sh")
actual_python_mise_sha256=$(sha256_file "$repo_root/scripts/python-mise.toml")
actual_python_lock_sha256=$(sha256_file "$repo_root/scripts/$python_lock_file")
actual_metadata_pack_sha256=$(sha256_file "$repo_root/scripts/trusted-seed-metadata.pack")
actual_metadata_object_list_sha256=$(sha256_file "$repo_root/scripts/trusted-seed-metadata.objects")
if [[ "$actual_checker_sha256" != "$reviewed_checker_sha256" || "$actual_config_sha256" != "$reviewed_config_sha256" || "$actual_inventory_helper_sha256" != "$reviewed_inventory_helper_sha256" || "$actual_python_mise_sha256" != "$reviewed_python_mise_sha256" || "$actual_python_lock_sha256" != "$python_lock_sha256" || "$actual_metadata_pack_sha256" != "$reviewed_metadata_pack_sha256" || "$actual_metadata_object_list_sha256" != "$reviewed_metadata_object_list_sha256" ]]; then
  printf 'generated-workflows: checkout checker, configuration, or inventory helper differs from the reviewed installer pins\n' >&2
  exit 1
fi
seed_object=$(git_clean hash-object -t commit --stdin < "$repo_root/scripts/trusted-seed.commit") || exit 1
if [[ "$seed_object" != "$reviewed_seed_commit" ]]; then
  printf 'generated-workflows: embedded reviewed seed commit does not match its Git object ID\n' >&2
  exit 1
fi

install_root="$common_dir/jackin-generated-workflow-guard"
versions_dir="$install_root/versions"
cache_root="$install_root/cache"
artifact_root="$install_root/artifacts"
lock_dir="$install_root/install.lock"
validate_existing_hook "$hooks_path/pre-commit"
validate_existing_hook "$hooks_path/pre-push"
umask 077
if [[ -e "$install_root" || -L "$install_root" ]]; then
  if [[ -L "$install_root" || ! -d "$install_root" ]]; then
    printf 'generated-workflows: private install root is unsafe\n' >&2
    exit 1
  fi
  if [[ "$platform" == macos-arm64 ]]; then
    install_owner=$(/usr/bin/stat -f '%u' "$install_root")
    install_mode=$(/usr/bin/stat -f '%Lp' "$install_root")
  else
    install_owner=$(/usr/bin/stat -c '%u' "$install_root")
    install_mode=$(/usr/bin/stat -c '%a' "$install_root")
  fi
  if [[ "$install_owner" != "$(/usr/bin/id -u)" ]]; then
    printf 'generated-workflows: private install root has the wrong owner\n' >&2
    exit 1
  fi
  if (( (8#$install_mode & 077) != 0 )); then
    printf 'generated-workflows: private install root permissions are unsafe\n' >&2
    exit 1
  fi
else
  mkdir "$install_root"
  chmod 700 "$install_root"
fi
if ! mkdir "$lock_dir" 2>/dev/null; then
  printf 'generated-workflows: another trusted checker installation is in progress\n' >&2
  exit 1
fi
cleanup() {
  unset trusted_setup_token
  if [[ -n "${stage_dir:-}" && -d "$stage_dir" ]]; then
    chmod -R u+w "$stage_dir" 2>/dev/null || true
    find "$stage_dir" -depth -delete
  fi
  if [[ -n "${python_stage:-}" && -d "$python_stage" ]]; then
    chmod -R u+w "$python_stage" 2>/dev/null || true
    find "$python_stage" -type d -exec chmod u+w {} + 2>/dev/null || true
    find "$python_stage" -type f -exec chmod u+w {} + 2>/dev/null || true
    find "$python_stage" -depth -delete 2>/dev/null || true
  fi
  if [[ -n "${mise_download:-}" ]]; then
    /bin/rm -f "$mise_download"
  fi
  rmdir "$lock_dir" 2>/dev/null || true
}
trap cleanup EXIT
ensure_private_directory() {
  local path=$1
  if [[ -L "$path" || ( -e "$path" && ! -d "$path" ) ]]; then
    printf 'generated-workflows: private installation directory is unsafe\n' >&2
    return 1
  fi
  if [[ ! -e "$path" ]]; then
    mkdir "$path" || return $?
    chmod 700 "$path" || return $?
  fi
  if [[ "$platform" == macos-arm64 ]]; then
    path_owner=$(/usr/bin/stat -f '%u' "$path")
    path_mode=$(/usr/bin/stat -f '%Lp' "$path")
  else
    path_owner=$(/usr/bin/stat -c '%u' "$path")
    path_mode=$(/usr/bin/stat -c '%a' "$path")
  fi
  if [[ "$path_owner" != "$(/usr/bin/id -u)" ]] || (( (8#$path_mode & 077) != 0 )); then
    printf 'generated-workflows: private installation directory owner or permissions are unsafe\n' >&2
    return 1
  fi
}
ensure_private_directory "$versions_dir"
ensure_private_directory "$cache_root"
ensure_private_directory "$artifact_root"
mise_artifact_dir="$artifact_root/mise/$mise_version/$platform"
if [[ "$hash_tool" == /usr/bin/shasum ]]; then
  python_generation=$(printf '%s\n' "$platform" "$python_version" "$python_artifact_sha256" "$python_lock_sha256" "$reviewed_python_mise_sha256" "$reviewed_inventory_helper_sha256" "$mise_version" "$mise_sha256" | "$hash_tool" -a 256 | /usr/bin/awk '{print $1}')
else
  python_generation=$(printf '%s\n' "$platform" "$python_version" "$python_artifact_sha256" "$python_lock_sha256" "$reviewed_python_mise_sha256" "$reviewed_inventory_helper_sha256" "$mise_version" "$mise_sha256" | "$hash_tool" | /usr/bin/awk '{print $1}')
fi
python_artifact_dir="$artifact_root/python/$python_version/$platform-$python_generation"
ensure_private_directory "$artifact_root/mise"
ensure_private_directory "$artifact_root/mise/$mise_version"
ensure_private_directory "$mise_artifact_dir"
ensure_private_directory "$artifact_root/python"
ensure_private_directory "$artifact_root/python/$python_version"
mise_artifact="$mise_artifact_dir/mise"
if [[ -e "$mise_artifact" || -L "$mise_artifact" ]]; then
  if [[ -L "$mise_artifact" || ! -f "$mise_artifact" || "$(sha256_file "$mise_artifact")" != "$mise_sha256" ]]; then
    printf 'generated-workflows: approved mise artifact is unsafe or has changed\n' >&2
    exit 1
  fi
else
  mise_download="$install_root/.mise-download.$$"
  if ! env -i PATH=/usr/bin:/bin:/usr/sbin:/sbin HOME="${TMPDIR:-/tmp}" TMPDIR="${TMPDIR:-/tmp}" LANG=C LC_ALL=C /usr/bin/curl --disable --proto '=https' --location --fail --silent --show-error --connect-timeout 10 --max-time 180 --max-filesize 268435456 --output "$mise_download" "https://github.com/jdx/mise/releases/download/v$mise_version/mise-v$mise_version-$platform"; then
    printf 'generated-workflows: cannot download the pinned official mise release\n' >&2
    exit 1
  fi
  if [[ "$(sha256_file "$mise_download")" != "$mise_sha256" ]]; then
    printf 'generated-workflows: official mise artifact checksum does not match the reviewed release identity\n' >&2
    exit 1
  fi
  chmod 500 "$mise_download"
  mv "$mise_download" "$mise_artifact"
  mise_download=
fi
python_runtime="$python_artifact_dir/runtime"
python_inventory="$python_artifact_dir/inventory"
python_artifact_manifest="$python_artifact_dir/manifest"
python_mise_config="$python_artifact_dir/mise.toml"
python_mise_lock="$python_artifact_dir/mise.lock"
if [[ -e "$python_artifact_dir" || -L "$python_artifact_dir" ]]; then
  if [[ -L "$python_artifact_dir" || ! -d "$python_artifact_dir" || -L "$python_runtime" || ! -d "$python_runtime" || -L "$python_inventory" || ! -f "$python_inventory" || -L "$python_artifact_manifest" || ! -f "$python_artifact_manifest" || -L "$python_mise_config" || ! -f "$python_mise_config" || -L "$python_mise_lock" || ! -f "$python_mise_lock" ]]; then
    printf 'generated-workflows: pinned Python artifact generation is incomplete\n' >&2
    exit 1
  fi
  if [[ "$(sha256_file "$python_mise_config")" != "$reviewed_python_mise_sha256" || "$(sha256_file "$python_mise_lock")" != "$python_lock_sha256" ]] ||
    ! /usr/bin/grep -F -x -q "archive_sha256=$python_artifact_sha256" "$python_artifact_manifest" ||
    ! /usr/bin/grep -F -x -q "lock_sha256=$python_lock_sha256" "$python_artifact_manifest" ||
    ! /usr/bin/grep -F -x -q "mise_config_sha256=$reviewed_python_mise_sha256" "$python_artifact_manifest" ||
    ! /usr/bin/grep -F -x -q "inventory_helper_sha256=$reviewed_inventory_helper_sha256" "$python_artifact_manifest" ||
    [[ "$(sha256_file "$python_inventory")" != "$(/usr/bin/grep '^inventory_sha256=' "$python_artifact_manifest" | /usr/bin/cut -d= -f2-)" ]]; then
    printf 'generated-workflows: pinned Python artifact manifest failed integrity validation\n' >&2
    exit 1
  fi
  "$repo_root/scripts/python-inventory.sh" check "$python_runtime" "$python_inventory"
else
  python_stage="$artifact_root/python/$python_version/.stage.$$"
  mkdir "$python_stage"
  python_stage=$(cd "$python_stage" && pwd -P)
  python_stage_parent=$(cd "$python_stage/.." && pwd -P)
  chmod 700 "$python_stage"
  cp "$repo_root/scripts/python-mise.toml" "$python_stage/mise.toml"
  cp "$repo_root/scripts/$python_lock_file" "$python_stage/mise.lock"
  chmod 400 "$python_stage/mise.toml" "$python_stage/mise.lock"
  mkdir -p "$python_stage/global" "$python_stage/system" "$python_stage/home" "$python_stage/data" "$python_stage/cache" "$python_stage/tmp"
  : > "$python_stage/global/config.toml"
  : > "$python_stage/system/config.toml"
  python_mise_environment=(
    PATH=/usr/bin:/bin:/usr/sbin:/sbin
    "HOME=$python_stage/home"
    "TMPDIR=$python_stage/tmp"
    "MISE_DATA_DIR=$python_stage/data"
    "MISE_CACHE_DIR=$python_stage/cache"
    "MISE_CONFIG_DIR=$python_stage/global"
    "MISE_GLOBAL_CONFIG_FILE=$python_stage/global/config.toml"
    "MISE_SYSTEM_CONFIG_DIR=$python_stage/system"
    "MISE_SYSTEM_CONFIG_FILE=$python_stage/system/config.toml"
    "MISE_CONFIG_FILE=$python_stage/mise.toml"
    "MISE_TRUSTED_CONFIG_PATHS=$python_stage/mise.toml"
    "MISE_CEILING_PATHS=$python_stage_parent"
    "MISE_OVERRIDE_CONFIG_FILENAMES=mise.toml"
    MISE_NO_ENV=1
    MISE_NO_HOOKS=1
    MISE_AUTO_INSTALL=0
    MISE_LOCKFILE=1
    MISE_PROVENANCE_API_FAILURES_FATAL=1
    LANG=C
    LC_ALL=C
  )
  if [[ "$(sha256_file "$python_stage/mise.toml")" != "$reviewed_python_mise_sha256" || "$(sha256_file "$python_stage/mise.lock")" != "$python_lock_sha256" ]]; then
    printf 'generated-workflows: staged private Python Mise configuration or lock failed integrity validation\n' >&2
    exit 1
  fi
  if ! env -i "${python_mise_environment[@]}" "$mise_artifact" --locked --cd "$python_stage" config ls --json > "$python_stage/config-ls.json" 2> "$python_stage/config-ls.log"; then
    cat "$python_stage/config-ls.log" >&2
    printf 'generated-workflows: Mise could not discover the staged private Python configuration\n' >&2
    exit 1
  fi
  python_config_count=$(/usr/bin/grep -c '"path":' "$python_stage/config-ls.json" || true)
  if [[ "$python_config_count" -ne 3 ]] || ! /usr/bin/grep -F -x -q "    \"path\": \"$python_stage/mise.toml\"," "$python_stage/config-ls.json" || ! /usr/bin/grep -F -x -q "    \"path\": \"$python_stage/global/config.toml\"," "$python_stage/config-ls.json" || ! /usr/bin/grep -F -x -q "    \"path\": \"$python_stage/system/config.toml\"," "$python_stage/config-ls.json"; then
    cat "$python_stage/config-ls.json" >&2
    printf 'generated-workflows: Mise loaded an unexpected Python configuration set\n' >&2
    exit 1
  fi
  /bin/rm -f "$python_stage/config-ls.json" "$python_stage/config-ls.log"
  if ! env -i "${python_mise_environment[@]}" "$mise_artifact" --locked --cd "$python_stage" install python > "$python_stage/install.log" 2>&1; then
    cat "$python_stage/install.log" >&2
    printf 'generated-workflows: Mise could not qualify the locked Python %s artifact\n' "$python_version" >&2
    exit 1
  fi
  if [[ "$(sha256_file "$python_stage/mise.toml")" != "$reviewed_python_mise_sha256" || "$(sha256_file "$python_stage/mise.lock")" != "$python_lock_sha256" ]]; then
    printf 'generated-workflows: private Python Mise configuration or lock changed during qualification\n' >&2
    exit 1
  fi
  mise_python_runtime="$python_stage/data/installs/python/$python_version"
  if [[ -L "$mise_python_runtime" || ! -d "$mise_python_runtime" ]]; then
    printf 'generated-workflows: private Mise Python installation is missing\n' >&2
    exit 1
  fi
  cp -Rp "$mise_python_runtime" "$python_stage/runtime"
  rm -rf "$python_stage/data" "$python_stage/cache" "$python_stage/home" "$python_stage/global" "$python_stage/system" "$python_stage/tmp"
  /bin/rm -f "$python_stage/install.log"
  runtime_python="$python_stage/runtime/bin/python3.15"
  if [[ ! -x "$runtime_python" || -L "$runtime_python" ]] || ! "$runtime_python" -I -S -c 'import sys, tomllib; sys.exit(sys.version_info[:3] != (3, 15, 2))'; then
    printf 'generated-workflows: locked relocatable Python failed its isolated runtime check\n' >&2
    exit 1
  fi
  find "$python_stage/runtime" -type f -exec chmod a-w {} +
  find "$python_stage/runtime" -type d -exec chmod a-w {} +
  "$repo_root/scripts/python-inventory.sh" write "$python_stage/runtime" "$python_stage/inventory"
  python_inventory_sha256=$(sha256_file "$python_stage/inventory")
  printf 'python_version=%s\nbackend=core:python\narchive_sha256=%s\nlock_sha256=%s\nmise_config_sha256=%s\ninventory_helper_sha256=%s\ninventory_sha256=%s\n' "$python_version" "$python_artifact_sha256" "$python_lock_sha256" "$reviewed_python_mise_sha256" "$reviewed_inventory_helper_sha256" "$python_inventory_sha256" > "$python_stage/manifest"
  chmod 400 "$python_stage/mise.toml" "$python_stage/mise.lock" "$python_stage/inventory" "$python_stage/manifest"
  chmod 500 "$python_stage/runtime" "$python_stage"
  mv "$python_stage" "$python_artifact_dir"
  python_stage=
  "$repo_root/scripts/python-inventory.sh" check "$python_runtime" "$python_inventory"
fi
python_inventory_sha256=$(sha256_file "$python_inventory")
python_relative_path=bin/python3.15
if [[ ! -x "$python_runtime/$python_relative_path" || -L "$python_runtime/$python_relative_path" ]]; then
  printf 'generated-workflows: pinned Python executable is missing or symlinked\n' >&2
  exit 1
fi
if ! "$python_runtime/$python_relative_path" -I -S -c 'import sys, tomllib; sys.exit(sys.version_info[:3] != (3, 15, 2))'; then
  printf 'generated-workflows: locked private Python runtime failed after publication\n' >&2
  exit 1
fi
version_dir="$versions_dir/$reviewed_checker_sha256"
if [[ -e "$version_dir" || -L "$version_dir" ]]; then
  if [[ -L "$version_dir" || ! -d "$version_dir" || ! -f "$version_dir/check-generated-workflows.sh" || -L "$version_dir/check-generated-workflows.sh" || ! -f "$version_dir/config.toml" || -L "$version_dir/config.toml" || ! -f "$version_dir/trusted-seed.commit" || -L "$version_dir/trusted-seed.commit" || ! -f "$version_dir/trusted-seed-metadata.pack" || -L "$version_dir/trusted-seed-metadata.pack" || ! -f "$version_dir/trusted-seed-metadata.objects" || -L "$version_dir/trusted-seed-metadata.objects" || ! -f "$version_dir/manifest" || -L "$version_dir/manifest" ]]; then
    printf 'generated-workflows: installed checker generation is unsafe\n' >&2
    exit 1
  fi
  if [[ "$(sha256_file "$version_dir/check-generated-workflows.sh")" != "$reviewed_checker_sha256" || "$(sha256_file "$version_dir/config.toml")" != "$reviewed_config_sha256" || "$(sha256_file "$version_dir/python-inventory.sh")" != "$reviewed_inventory_helper_sha256" || "$(sha256_file "$version_dir/trusted-seed-metadata.pack")" != "$reviewed_metadata_pack_sha256" || "$(sha256_file "$version_dir/trusted-seed-metadata.objects")" != "$reviewed_metadata_object_list_sha256" || "$(git_clean hash-object -t commit --stdin < "$version_dir/trusted-seed.commit")" != "$reviewed_seed_commit" ]]; then
    printf 'generated-workflows: installed checker generation failed integrity validation\n' >&2
    exit 1
  fi
else
  stage_dir="$versions_dir/.stage.$$"
  mkdir "$stage_dir"
  chmod 700 "$stage_dir"
  cp "$repo_root/scripts/check-generated-workflows.sh" "$stage_dir/check-generated-workflows.sh"
  cp "$repo_root/scripts/generated-workflow-guard-config.toml" "$stage_dir/config.toml"
  cp "$repo_root/scripts/python-inventory.sh" "$stage_dir/python-inventory.sh"
  cp "$repo_root/scripts/trusted-seed.commit" "$stage_dir/trusted-seed.commit"
  cp "$repo_root/scripts/trusted-seed-metadata.pack" "$stage_dir/trusted-seed-metadata.pack"
  cp "$repo_root/scripts/trusted-seed-metadata.objects" "$stage_dir/trusted-seed-metadata.objects"
  chmod 400 "$stage_dir/check-generated-workflows.sh" "$stage_dir/config.toml" "$stage_dir/trusted-seed.commit" "$stage_dir/trusted-seed-metadata.pack" "$stage_dir/trusted-seed-metadata.objects"
  chmod 500 "$stage_dir/python-inventory.sh"
  printf 'version=%s\nchecker_version=%s\nchecker_sha256=%s\nconfig_sha256=%s\ninventory_helper_sha256=%s\nseed_commit=%s\nseed_tree=%s\nmetadata_pack_sha256=%s\nmetadata_object_list_sha256=%s\nmise_version=%s\nmise_sha256=%s\npython_version=%s\npython_artifact_sha256=%s\npython_lock_sha256=%s\npython_mise_sha256=%s\ninventory_sha256=%s\n' "$install_version" "$install_version" "$reviewed_checker_sha256" "$reviewed_config_sha256" "$reviewed_inventory_helper_sha256" "$reviewed_seed_commit" "$reviewed_seed_tree" "$reviewed_metadata_pack_sha256" "$reviewed_metadata_object_list_sha256" "$mise_version" "$mise_sha256" "$python_version" "$python_artifact_sha256" "$python_lock_sha256" "$reviewed_python_mise_sha256" "$python_inventory_sha256" > "$stage_dir/manifest"
  chmod 400 "$stage_dir/manifest"
  chmod 500 "$stage_dir"
  if [[ "$(sha256_file "$stage_dir/check-generated-workflows.sh")" != "$reviewed_checker_sha256" || "$(sha256_file "$stage_dir/config.toml")" != "$reviewed_config_sha256" || "$(sha256_file "$stage_dir/trusted-seed-metadata.pack")" != "$reviewed_metadata_pack_sha256" || "$(sha256_file "$stage_dir/trusted-seed-metadata.objects")" != "$reviewed_metadata_object_list_sha256" || "$(git_clean hash-object -t commit --stdin < "$stage_dir/trusted-seed.commit")" != "$reviewed_seed_commit" ]]; then
    printf 'generated-workflows: staged checker generation failed integrity validation\n' >&2
    exit 1
  fi
  mv "$stage_dir" "$version_dir"
  stage_dir=
fi

if [[ "$(sha256_file "$version_dir/check-generated-workflows.sh")" != "$reviewed_checker_sha256" || "$(sha256_file "$version_dir/config.toml")" != "$reviewed_config_sha256" || "$(sha256_file "$version_dir/python-inventory.sh")" != "$reviewed_inventory_helper_sha256" || "$(sha256_file "$version_dir/trusted-seed-metadata.pack")" != "$reviewed_metadata_pack_sha256" || "$(sha256_file "$version_dir/trusted-seed-metadata.objects")" != "$reviewed_metadata_object_list_sha256" || "$(sha256_file "$python_inventory")" != "$python_inventory_sha256" ]]; then
  printf 'generated-workflows: installed checker or runtime artifact failed final integrity validation\n' >&2
  exit 1
fi
if ! /usr/bin/grep -F -x -q "metadata_pack_sha256=$reviewed_metadata_pack_sha256" "$version_dir/manifest" || ! /usr/bin/grep -F -x -q "metadata_object_list_sha256=$reviewed_metadata_object_list_sha256" "$version_dir/manifest" || ! /usr/bin/grep -F -x -q "seed_tree=$reviewed_seed_tree" "$version_dir/manifest" || ! /usr/bin/grep -F -x -q "inventory_sha256=$python_inventory_sha256" "$version_dir/manifest" || ! /usr/bin/grep -F -x -q "python_artifact_sha256=$python_artifact_sha256" "$version_dir/manifest" || ! /usr/bin/grep -F -x -q "python_lock_sha256=$python_lock_sha256" "$version_dir/manifest" || ! /usr/bin/grep -F -x -q "python_mise_sha256=$reviewed_python_mise_sha256" "$version_dir/manifest" || ! /usr/bin/grep -F -x -q "mise_sha256=$mise_sha256" "$version_dir/manifest"; then
  printf 'generated-workflows: installed checker manifest does not bind the qualified runtime artifacts\n' >&2
  exit 1
fi
prepare_environment=(/usr/bin/env -i)
if [[ -n "$trusted_setup_token" ]]; then
  prepare_environment+=(-S 'GITHUB_COM_TOKEN=${GITHUB_COM_TOKEN}')
fi
GITHUB_COM_TOKEN="$trusted_setup_token" "${prepare_environment[@]}" PATH=/usr/bin:/bin:/usr/sbin:/sbin HOME=/tmp TMPDIR=/tmp LANG=C LC_ALL=C GIT_CONFIG_NOSYSTEM=1 GIT_CONFIG_GLOBAL=/dev/null GIT_CONFIG_SYSTEM=/dev/null GIT_NO_REPLACE_OBJECTS=1 GIT_NO_LAZY_FETCH=1 /bin/bash "$version_dir/check-generated-workflows.sh" prepare-cache || {
  printf 'generated-workflows: trusted tool and dependency cache qualification failed; no new hook was installed\n' >&2
  exit 1
}

unset trusted_setup_token

write_hook() {
  local target=$1
  local mode=$2
  local temporary=$3
  validate_existing_hook "$target" || return $?
  {
    printf '#!/bin/bash\nset +x +a\nunset GITHUB_COM_TOKEN\nset -euo pipefail\nLC_ALL=C\nexport LC_ALL\nguard_owner=jackin-generated-workflow-guard\nPATH=/usr/bin:/bin:/usr/sbin:/sbin\nexport PATH\nentry_index_present=0\nentry_index=\nif [[ ${GIT_INDEX_FILE+x} ]]; then\n  entry_index_present=1\n  entry_index=$GIT_INDEX_FILE\nfi\n'
    printf 'checker=%q\nmode=%q\ntrusted_install_root=%q\ntrusted_version_dir=%q\nhash_tool=%q\n' "$version_dir/check-generated-workflows.sh" "$mode" "$install_root" "$version_dir" "$hash_tool"
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
  } > "$temporary"
  chmod 700 "$temporary"
}

if [[ -L "$hooks_path" ]]; then
  printf 'generated-workflows: effective hook directory is a symlink\n' >&2
  exit 1
fi
mkdir -p "$hooks_path"
hooks_dir=$(cd "$hooks_path" && pwd -P)
if [[ -L "$hooks_dir" || ! -d "$hooks_dir" ]]; then
  printf 'generated-workflows: effective hook directory is unsafe\n' >&2
  exit 1
fi
pre_commit_temp="$hooks_dir/.jackin-generated-workflow-guard.$$.pre-commit.new"
pre_push_temp="$hooks_dir/.jackin-generated-workflow-guard.$$.pre-push.new"
write_hook "$hooks_dir/pre-commit" pre-commit "$pre_commit_temp"
write_hook "$hooks_dir/pre-push" pre-push "$pre_push_temp"
pre_commit_backup=
pre_push_backup=
if [[ -f "$hooks_dir/pre-commit" ]]; then
  pre_commit_backup="$hooks_dir/.jackin-generated-workflow-guard.$$.pre-commit.backup"
  cp -p "$hooks_dir/pre-commit" "$pre_commit_backup"
fi
if [[ -f "$hooks_dir/pre-push" ]]; then
  pre_push_backup="$hooks_dir/.jackin-generated-workflow-guard.$$.pre-push.backup"
  cp -p "$hooks_dir/pre-push" "$pre_push_backup"
fi
if ! mv -f "$pre_commit_temp" "$hooks_dir/pre-commit"; then
  printf 'generated-workflows: cannot install the pre-commit hook\n' >&2
  exit 1
fi
if ! mv -f "$pre_push_temp" "$hooks_dir/pre-push"; then
  if [[ -n "$pre_commit_backup" ]]; then
    mv -f "$pre_commit_backup" "$hooks_dir/pre-commit"
  else
    /bin/rm -f "$hooks_dir/pre-commit"
  fi
  printf 'generated-workflows: cannot install the pre-push hook; the previous registration was restored\n' >&2
  exit 1
fi
/bin/rm -f "$pre_commit_backup" "$pre_push_backup"
printf 'generated-workflows: installed trusted checker %s in Git common metadata\n' "$reviewed_checker_sha256"
