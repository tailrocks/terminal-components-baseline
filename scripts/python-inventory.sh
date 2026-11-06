#!/usr/bin/env bash
# SPDX-FileCopyrightText: 2026 Alexey Zhokhov
# SPDX-License-Identifier: Apache-2.0
set -euo pipefail

export LC_ALL=C
mode=${1:?}
root=${2:?}
expected=${3-}
case "$(uname -s):$(uname -m)" in
  Darwin:arm64|Darwin:aarch64) hash_tool=/usr/bin/shasum ;;
  Linux:x86_64|Linux:amd64) hash_tool=/usr/bin/sha256sum ;;
  *) printf 'generated-workflows: unsupported Python inventory host\n' >&2; exit 2 ;;
esac

write_inventory() {
  local output=$1
  if [[ "$root" == *[[:cntrl:]]* ]]; then return 1; fi
  local listing="$output.paths"
  local modes="$output.modes"
  local hashes="$output.hashes"
  local path relative target item kind mode_line hash_line digest expected_path escaped
  local index item_count=0 stat_count=0 hash_count=0
  local -a items kinds stat_paths hash_paths
  items=(sentinel)
  kinds=(sentinel)
  stat_paths=(sentinel)
  hash_paths=(sentinel)
  unset 'items[0]' 'kinds[0]' 'stat_paths[0]' 'hash_paths[0]'
  (cd "$root" && /usr/bin/find . -mindepth 1 -print0 | LC_ALL=C /usr/bin/sort -z) > "$listing"
  : > "$output"
  flush_inventory_batch() {
    if [[ ${stat_count} -gt 0 ]]; then
      if [[ "$hash_tool" == /usr/bin/shasum ]]; then
        /usr/bin/stat -f '%Lp' "${stat_paths[@]}" > "$modes" || return 1
      else
        /usr/bin/stat -c '%a' "${stat_paths[@]}" > "$modes" || return 1
      fi
    else
      : > "$modes"
    fi
    if [[ ${hash_count} -gt 0 ]]; then
      if [[ "$hash_tool" == /usr/bin/shasum ]]; then
        "$hash_tool" -a 256 -- "${hash_paths[@]}" > "$hashes" || return 1
      else
        "$hash_tool" -- "${hash_paths[@]}" > "$hashes" || return 1
      fi
    else
      : > "$hashes"
    fi
    exec 3< "$modes" 4< "$hashes"
    for ((index=0; index<${item_count}; index++)); do
      item=${items[$index]}
      kind=${kinds[$index]}
      if [[ "$kind" == link ]]; then
        target=$(/usr/bin/readlink "$root/$item" && printf '.') || return 1
        target=${target%.}
        target=${target%$'\n'}
        if [[ "$target" == *[[:cntrl:]]* ]]; then
          printf 'generated-workflows: pinned Python contains an unsupported link target\n' >&2
          return 1
        fi
        printf 'link\t%s\t%s\n' "$item" "$target" >> "$output"
      else
        IFS= read -r mode_line <&3 || return 1
        [[ "$mode_line" =~ ^[0-7]+$ ]] || return 1
        if [[ "$kind" == directory ]]; then
          printf 'directory\t%s\t%s\n' "$mode_line" "$item" >> "$output"
        else
          IFS= read -r hash_line <&4 || return 1
          expected_path="$root/$item"
          escaped=${expected_path//\\/\\\\}
          if [[ "$escaped" != "$expected_path" ]]; then
            [[ "$hash_line" == \\* ]] || return 1
            hash_line=${hash_line#\\}
          fi
          digest=${hash_line:0:64}
          [[ "$digest" =~ ^[0-9a-f]{64}$ && "$hash_line" == "$digest  $escaped" ]] || return 1
          printf 'file\t%s\t%s\t%s\n' "$mode_line" "$digest" "$item" >> "$output"
        fi
      fi
    done
    if IFS= read -r mode_line <&3 || IFS= read -r hash_line <&4; then
      return 1
    fi
    exec 3<&- 4<&-
    items=(sentinel); kinds=(sentinel); stat_paths=(sentinel); hash_paths=(sentinel)
    unset 'items[0]' 'kinds[0]' 'stat_paths[0]' 'hash_paths[0]'
    item_count=0; stat_count=0; hash_count=0
  }
  while IFS= read -r -d '' path; do
    relative=${path#./}
    if [[ "$relative" == *[[:cntrl:]]* ]]; then
      printf 'generated-workflows: pinned Python contains an unsupported path\n' >&2
      return 1
    fi
    items+=("$relative"); item_count=$((item_count + 1))
    if [[ -L "$root/$relative" ]]; then
      kinds+=(link)
    elif [[ -d "$root/$relative" ]]; then
      kinds+=(directory); stat_paths+=("$root/$relative"); stat_count=$((stat_count + 1))
    elif [[ -f "$root/$relative" ]]; then
      kinds+=(file); stat_paths+=("$root/$relative"); hash_paths+=("$root/$relative"); stat_count=$((stat_count + 1)); hash_count=$((hash_count + 1))
    else
      printf 'generated-workflows: pinned Python contains a special file\n' >&2
      return 1
    fi
    if [[ ${item_count} -eq 64 ]]; then flush_inventory_batch || return 1; fi
  done < "$listing"
  if [[ ${item_count} -gt 0 ]]; then flush_inventory_batch || return 1; fi
  /bin/rm -f "$listing" "$modes" "$hashes"
}

case "$mode" in
  write)
    if [[ ! -d "$root" || -L "$root" || -z "$expected" ]]; then
      printf 'generated-workflows: pinned Python inventory source is unsafe\n' >&2
      exit 1
    fi
    write_inventory "$expected"
    ;;
  check)
    if [[ ! -d "$root" || -L "$root" || ! -f "$expected" || -L "$expected" ]]; then
      printf 'generated-workflows: pinned Python inventory inputs are unsafe\n' >&2
      exit 1
    fi
    inventory_tmp=${TMPDIR:-/tmp}/jackin-python-inventory.XXXXXX
    actual=$(mktemp "$inventory_tmp") || {
      printf 'generated-workflows: cannot create a private Python inventory check file\n' >&2
      exit 1
    }
    trap '/bin/rm -f "$actual" "$actual.paths" "$actual.modes" "$actual.hashes"' EXIT
    write_inventory "$actual"
    if ! /usr/bin/cmp -s "$actual" "$expected"; then
      printf 'generated-workflows: pinned Python artifact inventory does not match its qualified archive\n' >&2
      exit 1
    fi
    ;;
  *) printf 'usage: %s [write|check] root inventory\n' "$0" >&2; exit 2 ;;
esac
