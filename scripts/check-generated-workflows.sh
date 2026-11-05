#!/usr/bin/env bash
# SPDX-FileCopyrightText: 2026 Alexey Zhokhov
# SPDX-License-Identifier: Apache-2.0
set -euo pipefail

repo_root=$(git rev-parse --show-toplevel)
objects=$(git -C "$repo_root" rev-parse --path-format=absolute --git-path objects)
object_format=$(git -C "$repo_root" rev-parse --show-object-format)
local_git_env=$(git -C "$repo_root" rev-parse --local-env-vars)
source_alternate_dirs=${GIT_ALTERNATE_OBJECT_DIRECTORIES-}

case "$object_format" in
  sha1) object_length=40 ;;
  sha256) object_length=64 ;;
  *) printf 'generated-workflows: unsupported Git object format %s\n' "$object_format" >&2; exit 2 ;;
esac

temporary_root=$(mktemp -d "${TMPDIR:-/tmp}/jackin-generated-workflows.XXXXXX")
trap 'find "$temporary_root" -depth -delete' EXIT

check_tree() {
  local tree=$1
  local label=$2
  local snapshot="$temporary_root/$label"
  local preview="$temporary_root/$label-preview"
  local comparison_status

  (
    local environment_name
    while IFS= read -r environment_name; do
      [[ -z "$environment_name" ]] || unset "$environment_name"
    done <<< "$local_git_env"
    if [[ -n "$source_alternate_dirs" ]]; then
      export GIT_ALTERNATE_OBJECT_DIRECTORIES="$source_alternate_dirs"
    fi

    mkdir -p "$snapshot" "$preview" || exit $?
    git init --object-format="$object_format" --quiet "$snapshot" || exit $?
    mkdir -p "$snapshot/.git/objects/info" || exit $?
    printf '%s\n' "$objects" > "$snapshot/.git/objects/info/alternates" || exit $?
    git -C "$snapshot" read-tree "$tree" || exit $?
    git -C "$snapshot" checkout-index --all || exit $?

    cd "$snapshot" || exit $?
    mise exec -- velnor-actions generate --output-dir "$preview" </dev/null || exit $?

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

check_push_records() {
  local local_ref
  local local_oid
  local remote_ref
  local remote_oid
  local extra
  local tree
  local checked=0

  while IFS=$' \t' read -r local_ref local_oid remote_ref remote_oid extra || [[ -n "${local_ref:-}${local_oid:-}${remote_ref:-}${remote_oid:-}${extra:-}" ]]; do
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

    tree=$(git -C "$repo_root" rev-parse --verify "${local_oid}^{tree}") || {
      printf 'generated-workflows: cannot resolve pushed object %s for %s\n' "$local_oid" "$local_ref" >&2
      return 1
    }

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
