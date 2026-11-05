#!/usr/bin/env bash
# SPDX-FileCopyrightText: 2026 Alexey Zhokhov
# SPDX-License-Identifier: Apache-2.0
set -euo pipefail

export GIT_NO_REPLACE_OBJECTS=1

repo_root=$(git rev-parse --show-toplevel)
repo_root=$(cd "$repo_root" && pwd -P)
objects=$(git -C "$repo_root" rev-parse --path-format=absolute --git-path objects)
object_format=$(git -C "$repo_root" rev-parse --show-object-format)
local_git_env=$(git -C "$repo_root" rev-parse --local-env-vars)
source_alternate_dirs=${GIT_ALTERNATE_OBJECT_DIRECTORIES-}
mise_command=$(command -v mise) || {
  printf 'generated-workflows: mise is unavailable\n' >&2
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
trap 'find "$temporary_root" -depth -delete' EXIT
temporary_root=$(cd "$temporary_root" && pwd -P) || {
  printf 'generated-workflows: cannot resolve the temporary directory\n' >&2
  exit 1
}

fail_materialization() {
  printf 'generated-workflows: cannot materialize snapshot: %s\n' "$1" >&2
  return 1
}

git_in_snapshot_context() {
  local environment_name
  local -a environment_options=()
  while IFS= read -r environment_name; do
    [[ -z "$environment_name" ]] || environment_options+=(-u "$environment_name")
  done <<< "$local_git_env"
  environment_options+=(GIT_NO_REPLACE_OBJECTS=1)
  if [[ -n "$source_alternate_dirs" ]]; then
    environment_options+=("GIT_ALTERNATE_OBJECT_DIRECTORIES=$source_alternate_dirs")
  fi
  env "${environment_options[@]}" git "$@"
}

create_parent_directories() {
  local root=$1
  local path=$2
  local parent_path
  local current
  local component

  case "$path" in
    /*|.|..|./*|../*|*/./*|*/.|*/../*|*/..|.git|.git/*|*//*|'')
      fail_materialization "unsafe path"
      return 1
      ;;
  esac

  parent_path=${path%/*}
  if [[ "$parent_path" == "$path" ]]; then
    return 0
  fi

  current=$root
  while [[ -n "$parent_path" ]]; do
    case "$parent_path" in
      */*)
        component=${parent_path%%/*}
        parent_path=${parent_path#*/}
        ;;
      *)
        component=$parent_path
        parent_path=
        ;;
    esac
    current="$current/$component"
    if [[ -L "$current" ]]; then
      fail_materialization "path traverses a symlink"
      return 1
    fi
    if [[ -e "$current" ]]; then
      if [[ ! -d "$current" ]]; then
        fail_materialization "path traverses a non-directory"
        return 1
      fi
    else
      mkdir "$current" || return $?
    fi
  done
}

materialize_tree() {
  local snapshot=$1
  local entries=$2
  local entry
  local header
  local path
  local mode
  local type
  local object_id
  local target
  local link_file
  local link_target
  local entry_number=0

  while IFS= read -r -d '' entry || [[ -n "$entry" ]]; do
    header=${entry%%$'\t'*}
    path=${entry#*$'\t'}
    mode=${header%% *}
    header=${header#* }
    type=${header%% *}
    object_id=${header#* }
    entry_number=$((entry_number + 1))

    case "$path" in
      /*|.|..|./*|../*|*/./*|*/.|*/../*|*/..|.git|.git/*|*//*|'')
        fail_materialization "unsafe path"
        return 1
        ;;
    esac

    target="$snapshot/$path"
    create_parent_directories "$snapshot" "$path" || return $?
    if [[ -e "$target" || -L "$target" ]]; then
      fail_materialization "duplicate path"
      return 1
    fi

    case "$mode:$type" in
      100644:blob|100755:blob)
        git -C "$repo_root" cat-file blob "$object_id" > "$target" || return $?
        if [[ "$mode" == 100755 ]]; then
          chmod 755 "$target" || return $?
        else
          chmod 644 "$target" || return $?
        fi
        ;;
      120000:blob)
        link_file="$temporary_root/link-target-$entry_number"
        git -C "$repo_root" cat-file blob "$object_id" > "$link_file" || return $?
        if IFS= read -r -d '' link_target < "$link_file"; then
          fail_materialization "symlink target contains a NUL byte"
          return 1
        fi
        if [[ -z "$link_target" ]]; then
          fail_materialization "symlink target is empty"
          return 1
        fi
        ln -s "$link_target" "$target" || return $?
        ;;
      160000:commit)
        mkdir "$target" || return $?
        ;;
      *)
        fail_materialization "unsupported entry mode $mode"
        return 1
        ;;
    esac
  done < "$entries"
}

run_generator() {
  local snapshot=$1
  local preview=$2
  local label=$3
  local state_dir="$temporary_root/mise-state"
  local config_dir="$temporary_root/mise-config"
  local config_file
  local tool_pin
  local tool_bin
  local tool_dir
  local tool_executable

  mkdir -p "$state_dir" "$config_dir" || return $?
  : > "$config_dir/global.toml" || return $?
  : > "$config_dir/system.toml" || return $?

  (
    local environment_name
    while IFS= read -r environment_name; do
      [[ -z "$environment_name" ]] || unset "$environment_name"
    done <<< "$local_git_env"
    while IFS= read -r environment_name; do
      case "$environment_name" in
        MISE_*_VERSION) unset "$environment_name" ;;
      esac
    done < <(compgen -e)
    if [[ -n "$source_alternate_dirs" ]]; then
      export GIT_ALTERNATE_OBJECT_DIRECTORIES="$source_alternate_dirs"
    fi
    export GIT_NO_REPLACE_OBJECTS=1
    export MISE_CONFIG_DIR="$config_dir"
    export MISE_GLOBAL_CONFIG_FILE="$config_dir/global.toml"
    export MISE_SYSTEM_CONFIG_FILE="$config_dir/system.toml"
    export MISE_STATE_DIR="$state_dir"
    export MISE_PARANOID=1
    export MISE_OVERRIDE_CONFIG_FILENAMES=mise.toml
    export MISE_CEILING_PATHS="$temporary_root"
    export MISE_TRUSTED_CONFIG_PATHS=
    export MISE_ENABLE_TOOLS=github:tailrocks/velnor-new,rust,jactionlint
    export MISE_ENV=
    unset MISE_CONFIG_FILE MISE_GLOBAL_CONFIG_ROOT MISE_DEFAULT_CONFIG_FILENAME MISE_NO_CONFIG MISE_NO_ENV

    for config_file in mise.toml rust-toolchain.toml; do
      if [[ ! -f "$snapshot/$config_file" || -L "$snapshot/$config_file" ]]; then
        printf 'generated-workflows: missing regular snapshot config %s\n' "$config_file" >&2
        exit 1
      fi
    done

    "$mise_command" --cd "$snapshot" trust "$snapshot/mise.toml" </dev/null || exit $?
    "$mise_command" --cd "$snapshot" trust "$snapshot/rust-toolchain.toml" </dev/null || exit $?
    tool_pin=$("$mise_command" --cd "$snapshot" config get --file "$snapshot/mise.toml" 'tools."github:tailrocks/velnor-new".version') || {
      printf 'generated-workflows: cannot read the snapshot-pinned Velnor version\n' >&2
      exit 1
    }
    tool_bin=$("$mise_command" --cd "$snapshot" config get --file "$snapshot/mise.toml" 'tools."github:tailrocks/velnor-new".bin') || {
      printf 'generated-workflows: cannot read the snapshot-pinned Velnor executable name\n' >&2
      exit 1
    }
    if [[ -z "$tool_pin" ]]; then
      printf 'generated-workflows: snapshot does not pin Velnor\n' >&2
      exit 1
    fi
    if [[ -z "$tool_bin" || "$tool_bin" == */* || "$tool_bin" == . || "$tool_bin" == .. ]]; then
      printf 'generated-workflows: snapshot Velnor executable name is invalid\n' >&2
      exit 1
    fi
    "$mise_command" --locked --cd "$snapshot" install </dev/null || exit $?
    tool_dir=$("$mise_command" --cd "$snapshot" where github:tailrocks/velnor-new) || {
      printf 'generated-workflows: snapshot-pinned Velnor is not installed\n' >&2
      exit 1
    }
    if [[ "${tool_dir##*/}" != "$tool_pin" ]]; then
      printf 'generated-workflows: installed Velnor does not match snapshot pin %s\n' "$tool_pin" >&2
      exit 1
    fi
    tool_executable="$tool_dir/$tool_bin"
    if [[ ! -x "$tool_executable" ]]; then
      printf 'generated-workflows: snapshot-pinned Velnor executable is unavailable\n' >&2
      exit 1
    fi
    cd "$snapshot" || exit $?
    "$mise_command" --locked --cd "$snapshot" exec github:tailrocks/velnor-new rust jactionlint -- "$tool_executable" generate --output-dir "$preview" </dev/null || exit $?

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

  mkdir -p "$snapshot" "$preview" || return $?
  git_in_snapshot_context init --object-format="$object_format" --quiet "$snapshot" || return $?
  mkdir -p "$snapshot/.git/objects/info" || return $?
  printf '%s\n' "$objects" > "$snapshot/.git/objects/info/alternates" || return $?
  git -C "$repo_root" ls-tree -r -z --full-tree "$tree" > "$entries" || return $?
  git_in_snapshot_context -C "$snapshot" read-tree "$tree" || return $?
  materialize_tree "$snapshot" "$entries" || return $?
  run_generator "$snapshot" "$preview" "$label"
}

resolve_push_tree() {
  local local_oid=$1
  local remote_ref=$2
  local peeled_oid
  local peeled_type
  local commit

  case "$remote_ref" in
    refs/heads/*)
      commit=$(git -C "$repo_root" rev-parse --verify "${local_oid}^{commit}") || {
        printf 'generated-workflows: cannot resolve pushed commit %s for %s\n' "$local_oid" "$remote_ref" >&2
        return 1
      }
      ;;
    refs/tags/*)
      peeled_oid=$(git -C "$repo_root" rev-parse --verify "${local_oid}^{}") || {
        printf 'generated-workflows: cannot peel pushed tag object %s for %s\n' "$local_oid" "$remote_ref" >&2
        return 1
      }
      peeled_type=$(git -C "$repo_root" cat-file -t "$peeled_oid") || {
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

  git -C "$repo_root" rev-parse --verify "${commit}^{tree}"
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
    tree=$(git -C "$repo_root" write-tree) || {
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
