#!/usr/bin/env bash
# SPDX-FileCopyrightText: 2026 Alexey Zhokhov
# SPDX-License-Identifier: Apache-2.0
set -euo pipefail

repo_root=$(cd "$(dirname "$0")/.." && pwd -P)
root_checker="$repo_root/scripts/check-generated-workflows.sh"
checker=${CHECKER_UNDER_TEST:-$root_checker}
real_path=$PATH
mise_path=$(command -v mise)
test_root=$(mktemp -d "${TMPDIR:-/tmp}/baseline snapshot tests.XXXXXX")
trap 'find "$test_root" -depth -delete' EXIT
canonical="$test_root/canonical"
fake_bin="$test_root/bin"
fake_log="$test_root/mise.log"
object_length=$(git -C "$repo_root" rev-parse --show-object-format | awk '{ if ($0 == "sha1") print 40; else if ($0 == "sha256") print 64; else exit 1 }')
zero_oid=$(printf '%0*d' "$object_length" 0)
mkdir -p "$canonical/workflows" "$fake_bin"
printf 'name: generated\n' > "$canonical/workflows/generated.yml"
printf '#!/bin/sh\nexit 0\n' > "$canonical/workflows/run.sh"
chmod 755 "$canonical/workflows/run.sh"
ln -s generated.yml "$canonical/workflows/current"

cat > "$fake_bin/mise" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail
mise_cwd=
while (($#)); do
  case "$1" in
    --locked) shift ;;
    --cd) mise_cwd=$2; shift 2 ;;
    *) operation=$1; shift; break ;;
  esac
done
read_pin() {
  sed -n 's/.*"github:tailrocks\/velnor-new"[[:space:]]*=[[:space:]]*{[[:space:]]*version[[:space:]]*=[[:space:]]*"\([^"]*\)".*/\1/p' "$mise_cwd/mise.toml"
}
case "${operation:-}" in
  trust)
    [[ $# -eq 1 && -f "$1" && ! -L "$1" ]]
    printf 'trust|%s|%s|%s\n' "$1" "${MISE_STATE_DIR-}" "${MISE_PARANOID-}" >> "$FAKE_LOG"
    ;;
  config)
    [[ "${1:-}" == get ]]
    shift
    config_file=
    while (($#)); do
      case "$1" in
        --file) config_file=$2; shift 2 ;;
        *) key=$1; shift ;;
      esac
    done
    case "$key" in
      'tools."github:tailrocks/velnor-new".version')
        version=$(sed -n 's/.*"github:tailrocks\/velnor-new"[[:space:]]*=[[:space:]]*{[[:space:]]*version[[:space:]]*=[[:space:]]*"\([^"]*\)".*/\1/p' "$config_file")
        [[ -n "$version" ]]
        printf 'pin|%s\n' "$version" >> "$FAKE_LOG"
        if [[ -n "${FAKE_EXPECT_PIN-}" && "$version" != "$FAKE_EXPECT_PIN" ]]; then
          exit 1
        fi
        printf '%s\n' "$version"
        ;;
      'tools."github:tailrocks/velnor-new".bin')
        printf 'velnor-actions\n'
        ;;
      *) exit 2 ;;
    esac
    ;;
  install)
    version=$(read_pin)
    [[ -n "$version" ]]
    mkdir -p "$FAKE_INSTALL_DIR/$version"
    printf '#!/bin/sh\nexit 0\n' > "$FAKE_INSTALL_DIR/$version/velnor-actions"
    chmod 755 "$FAKE_INSTALL_DIR/$version/velnor-actions"
    printf 'install|%s\n' "$version" >> "$FAKE_LOG"
    ;;
  where)
    [[ "${1:-}" == github:tailrocks/velnor-new ]]
    printf '%s/%s\n' "$FAKE_INSTALL_DIR" "$(read_pin)"
    ;;
  which)
    version=$(read_pin)
    case "${1:-}" in
      --version) printf '%s\n' "$version" ;;
      velnor-actions)
        if [[ "${2:-}" == --version ]]; then
          printf '%s\n' "$version"
        else
          printf '%s/%s/velnor-actions\n' "$FAKE_INSTALL_DIR" "$version"
        fi
        ;;
      *) exit 2 ;;
    esac
    ;;
  exec)
    output_dir=
    while (($#)); do
      if [[ "$1" == --output-dir ]]; then
        output_dir=$2
        shift 2
      else
        shift
      fi
    done
    [[ -n "$output_dir" ]]
    printf 'exec|%s|%s\n' "$PWD" "$output_dir" >> "$FAKE_LOG"
    [[ "${FAKE_MODE:-copy}" != fail-generator ]]
    mkdir -p "$output_dir/.github"
    cp -pPR "$FAKE_GENERATED_DIR/." "$output_dir/.github/"
    if [[ "${FAKE_MODE:-copy}" == drift ]]; then
      printf 'drift\n' >> "$output_dir/.github/workflows/generated.yml"
    fi
    if [[ -n "${FAKE_CAPTURE_LIST-}" ]]; then
      capture_index=0
      while IFS= read -r -d '' capture_path || [[ -n "$capture_path" ]]; do
        capture_index=$((capture_index + 1))
        cat "$PWD/$capture_path" > "$FAKE_CAPTURE_DIR/$capture_index"
      done < "$FAKE_CAPTURE_LIST"
    fi
    if [[ -n "${FAKE_CAPTURE_EXECUTABLE-}" ]]; then
      if [[ -x "$PWD/$FAKE_CAPTURE_EXECUTABLE" ]]; then
        printf 'executable\n' > "$FAKE_CAPTURE_META"
      else
        printf 'not-executable\n' > "$FAKE_CAPTURE_META"
      fi
    fi
    if [[ -n "${FAKE_CAPTURE_SYMLINK-}" ]]; then
      if [[ -L "$PWD/$FAKE_CAPTURE_SYMLINK" ]]; then
        readlink "$PWD/$FAKE_CAPTURE_SYMLINK" > "$FAKE_CAPTURE_LINK"
      else
        printf 'not-symlink\n' > "$FAKE_CAPTURE_LINK"
      fi
    fi
    ;;
  *)
    printf 'fake-mise: unsupported command %s\n' "${operation:-missing}" >&2
    exit 2
    ;;
esac
EOF
chmod 755 "$fake_bin/mise"

write_project_config() {
  local directory=$1
  local pin=$2
  cat > "$directory/mise.toml" <<EOF
min_version = "2026.11.1"

[tools]
"github:tailrocks/velnor-new" = { version = "$pin", bin = "velnor-actions" }
rust = "1.99.1"
jactionlint = "2.4.0"

[settings]
idiomatic_version_file_enable_tools = ["rust"]
EOF
  printf '[toolchain]\nchannel = "1.99.1"\n' > "$directory/rust-toolchain.toml"
}

new_repo() {
  local name=$1
  local pin=${2:-0.1.74}
  local object_format=${3:-sha1}
  local directory="$test_root/$name"
  mkdir -p "$directory/.velnor" "$directory/docs"
  cp -pPR "$canonical/." "$directory/.github/"
  printf 'schema = 1\n' > "$directory/.velnor/config.toml"
  printf 'fixture\n' > "$directory/docs/keep.md"
  write_project_config "$directory" "$pin"
  git init --object-format="$object_format" --quiet "$directory"
  git -C "$directory" config user.name 'Baseline Checker Test'
  git -C "$directory" config user.email 'baseline-checker@example.invalid'
  git -C "$directory" add -A
  git -C "$directory" commit --quiet -m initial
  printf '%s\n' "$directory"
}

commit_all() {
  local directory=$1
  local message=$2
  git -C "$directory" add -A
  git -C "$directory" commit --quiet -m "$message"
  git -C "$directory" rev-parse HEAD
}

push_record() {
  printf '%s %s %s %s\n' "$1" "$2" "$3" "$4"
}

run_fake_checker() {
  local directory=$1
  local mode=$2
  local records=${3-}
  if [[ "$mode" == pre-push ]]; then
    printf '%s\n' "$records" | (cd "$directory" && PATH="$fake_bin:$real_path" FAKE_GENERATED_DIR="$canonical" FAKE_LOG="$fake_log" FAKE_INSTALL_DIR="$test_root/fake-installed" bash "$checker" "$mode")
  else
    (cd "$directory" && PATH="$fake_bin:$real_path" FAKE_GENERATED_DIR="$canonical" FAKE_LOG="$fake_log" FAKE_INSTALL_DIR="$test_root/fake-installed" bash "$checker" "$mode")
  fi
}

run_fake_checker_with_capture() {
  local directory=$1
  local mode=$2
  local records=$3
  local capture_list=$4
  local capture_dir=$5
  printf '%s\n' "$records" | (cd "$directory" && PATH="$fake_bin:$real_path" FAKE_GENERATED_DIR="$canonical" FAKE_LOG="$fake_log" FAKE_INSTALL_DIR="$test_root/fake-installed" FAKE_CAPTURE_LIST="$capture_list" FAKE_CAPTURE_DIR="$capture_dir" bash "$checker" "$mode")
}

run_empty_push() {
  local directory=$1
  (cd "$directory" && PATH="$fake_bin:$real_path" FAKE_GENERATED_DIR="$canonical" FAKE_LOG="$fake_log" FAKE_INSTALL_DIR="$test_root/fake-installed" bash "$checker" pre-push </dev/null)
}

assert_success() {
  local label=$1
  shift
  if "$@"; then
    printf 'PASS %s\n' "$label"
  else
    local status=$?
    printf 'FAIL %s: exit %s\n' "$label" "$status" >&2
    return 1
  fi
}

assert_failure() {
  local label=$1
  shift
  if "$@"; then
    printf 'FAIL %s: unexpectedly succeeded\n' "$label" >&2
    return 1
  else
    printf 'PASS %s\n' "$label"
  fi
}

assert_equal() {
  local label=$1
  local expected=$2
  local actual=$3
  if [[ "$actual" == "$expected" ]]; then
    printf 'PASS %s\n' "$label"
  else
    printf 'FAIL %s: expected <%s>, got <%s>\n' "$label" "$expected" "$actual" >&2
    return 1
  fi
}

assert_distinct() {
  local label=$1
  local first=$2
  local second=$3
  if [[ "$first" != "$second" ]]; then
    printf 'PASS %s\n' "$label"
  else
    printf 'FAIL %s: values unexpectedly match <%s>\n' "$label" "$first" >&2
    return 1
  fi
}

assert_contains() {
  local label=$1
  local expected=$2
  local file=$3
  if rg -F -q "$expected" "$file"; then
    printf 'PASS %s\n' "$label"
  else
    printf 'FAIL %s: did not find <%s>\n' "$label" "$expected" >&2
    return 1
  fi
}

count_execs() {
  awk -F'|' '$1 == "exec" { count++ } END { print count + 0 }' "$fake_log"
}

test_snapshot_pin() {
  local directory
  local source_branch
  local source_oid
  local alternate_oid
  local feature_record
  directory=$(new_repo snapshot-pin 0.1.74)
  source_branch=$(git -C "$directory" symbolic-ref --short HEAD)
  source_oid=$(git -C "$directory" rev-parse HEAD)
  git -C "$directory" checkout --quiet -b snapshot-tool-pin
  write_project_config "$directory" 0.1.73
  alternate_oid=$(commit_all "$directory" alternate-snapshot-pin)
  git -C "$directory" checkout --quiet "$source_branch"
  [[ "$(git -C "$directory" rev-parse HEAD)" == "$source_oid" ]]
  : > "$fake_log"
  export FAKE_EXPECT_PIN=0.1.73
  feature_record=$(push_record refs/heads/snapshot-tool-pin "$alternate_oid" refs/heads/feature "$zero_oid")
  assert_success 'push snapshot selects its own Velnor pin' run_fake_checker "$directory" pre-push "$feature_record"
  unset FAKE_EXPECT_PIN
  assert_equal 'pin was read from pushed tree' 1 "$(grep -c '^pin|0.1.73$' "$fake_log")"
}

test_auxiliary_refs() {
  local directory
  local clean_oid
  local drift_oid
  local blob_oid
  local notes_record
  local blob_record
  local branch_record
  directory=$(new_repo auxiliary-refs)
  clean_oid=$(git -C "$directory" rev-parse HEAD)
  printf 'changed\n' > "$directory/.github/workflows/generated.yml"
  drift_oid=$(commit_all "$directory" drift)
  blob_oid=$(printf 'auxiliary blob\n' | git -C "$directory" hash-object -w --stdin)
  notes_record=$(push_record refs/notes/commits "$drift_oid" refs/notes/commits "$zero_oid")
  blob_record=$(push_record refs/meta/blob "$blob_oid" refs/meta/blob "$zero_oid")
  : > "$fake_log"
  assert_success 'notes and blob auxiliary refs are skipped' run_fake_checker "$directory" pre-push "$notes_record"$'\n'"$blob_record"
  assert_equal 'auxiliary refs do not run generation' 0 "$(count_execs)"
  branch_record=$(push_record refs/heads/feature "$clean_oid" refs/heads/feature "$zero_oid")
  : > "$fake_log"
  assert_success 'mixed branch and notes records validate only the branch snapshot' run_fake_checker "$directory" pre-push "$branch_record"$'\n'"$notes_record"
  assert_equal 'mixed records generate once' 1 "$(count_execs)"
}

test_push_destinations() {
  local directory
  local clean_oid
  local drift_oid
  local blob_oid
  local tag_oid
  local records
  local missing_oid
  local sha256_directory
  local sha256_zero
  local sha256_clean_oid
  local sha256_tag_oid
  directory=$(new_repo push-destinations)
  assert_success 'empty pre-push input passes' run_empty_push "$directory"
  clean_oid=$(git -C "$directory" rev-parse HEAD)
  records=$(push_record refs/heads/main "$clean_oid" refs/heads/main "$zero_oid")
  assert_success 'clean branch snapshot passes' run_fake_checker "$directory" pre-push "$records"
  records=$(push_record refs/tags/lightweight "$clean_oid" refs/tags/lightweight "$zero_oid")
  assert_success 'lightweight commit tag passes' run_fake_checker "$directory" pre-push "$records"
  git -C "$directory" tag -a annotated -m annotated "$clean_oid"
  tag_oid=$(git -C "$directory" rev-parse refs/tags/annotated)
  records=$(push_record refs/tags/annotated "$tag_oid" refs/tags/annotated "$zero_oid")
  assert_success 'annotated commit tag passes' run_fake_checker "$directory" pre-push "$records"
  blob_oid=$(printf 'non-commit tag\n' | git -C "$directory" hash-object -w --stdin)
  git -C "$directory" update-ref refs/tags/blob-tag "$blob_oid"
  records=$(push_record refs/tags/blob-tag "$blob_oid" refs/tags/blob-tag "$zero_oid")
  : > "$fake_log"
  assert_success 'blob-valued tag is skipped' run_fake_checker "$directory" pre-push "$records"
  assert_equal 'non-commit tag does not run generation' 0 "$(count_execs)"
  printf 'changed\n' > "$directory/.github/workflows/generated.yml"
  drift_oid=$(commit_all "$directory" drift)
  git -C "$directory" update-ref refs/tags/drift-tag "$drift_oid"
  records=$(push_record refs/tags/drift-tag "$drift_oid" refs/tags/drift-tag "$zero_oid")
  assert_failure 'drifted commit tag fails' run_fake_checker "$directory" pre-push "$records"
  records=$(push_record refs/heads/removed "$zero_oid" refs/heads/removed "$clean_oid")
  : > "$fake_log"
  assert_success 'deletion skips generation' run_fake_checker "$directory" pre-push "$records"
  assert_equal 'deletion does not run generation' 0 "$(count_execs)"
  records=$(push_record refs/heads/bad "$clean_oid" refs/heads/bad "$zero_oid")
  assert_failure 'malformed record fails' run_fake_checker "$directory" pre-push "$records extra"
  missing_oid=$(printf '%*s' "$object_length" '' | tr ' ' a)
  records=$(push_record refs/heads/missing "$missing_oid" refs/heads/missing "$zero_oid")
  assert_failure 'missing branch object fails' run_fake_checker "$directory" pre-push "$records"
  records=$(push_record refs/heads/blob "$blob_oid" refs/heads/blob "$zero_oid")
  assert_failure 'non-commit branch object fails' run_fake_checker "$directory" pre-push "$records"
  sha256_directory=$(new_repo sha256-destinations 0.1.74 sha256)
  sha256_zero=$(printf '%064d' 0)
  sha256_clean_oid=$(git -C "$sha256_directory" rev-parse HEAD)
  records=$(push_record refs/heads/main "$sha256_clean_oid" refs/heads/main "$sha256_zero")
  assert_success 'SHA-256 branch record passes' run_fake_checker "$sha256_directory" pre-push "$records"
  git -C "$sha256_directory" tag -a sha256-tag -m sha256-tag "$sha256_clean_oid"
  sha256_tag_oid=$(git -C "$sha256_directory" rev-parse refs/tags/sha256-tag)
  records=$(push_record refs/tags/sha256-tag "$sha256_tag_oid" refs/tags/sha256-tag "$sha256_zero")
  assert_success 'SHA-256 annotated tag record passes' run_fake_checker "$sha256_directory" pre-push "$records"
}

test_replacements() {
  local directory
  local clean_oid
  local drift_oid
  local tag_oid
  local records
  local replacement_before
  directory=$(new_repo replacement-refs)
  clean_oid=$(git -C "$directory" rev-parse HEAD)
  printf 'drift\n' > "$directory/.github/workflows/generated.yml"
  drift_oid=$(commit_all "$directory" drift)
  git -C "$directory" replace "$drift_oid" "$clean_oid"
  replacement_before=$(git -C "$directory" replace -l)
  records=$(push_record refs/heads/replaced-drift "$drift_oid" refs/heads/replaced-drift "$zero_oid")
  assert_failure 'replacement cannot hide drift in the original commit' run_fake_checker "$directory" pre-push "$records"
  assert_equal 'replacement refs remain unchanged' "$replacement_before" "$(git -C "$directory" replace -l)"
  git -C "$directory" replace -d "$drift_oid" >/dev/null
  git -C "$directory" replace "$clean_oid" "$drift_oid"
  records=$(push_record refs/heads/replaced-clean "$clean_oid" refs/heads/replaced-clean "$zero_oid")
  assert_success 'replacement cannot introduce drift into the original clean commit' run_fake_checker "$directory" pre-push "$records"
  git -C "$directory" replace -d "$clean_oid" >/dev/null
  git -C "$directory" tag -a replaced-tag -m replaced-tag "$drift_oid"
  tag_oid=$(git -C "$directory" rev-parse refs/tags/replaced-tag)
  git -C "$directory" replace "$drift_oid" "$clean_oid"
  records=$(push_record refs/tags/replaced-tag "$tag_oid" refs/tags/replaced-tag "$zero_oid")
  assert_failure 'annotated tag is checked against the original replaced commit' run_fake_checker "$directory" pre-push "$records"
}

test_raw_materialization() {
  local directory
  local global_config="$test_root/gitconfig"
  local clean_filter="$test_root/identity-filter"
  local filter_script="$test_root/hostile-filter"
  local marker="$test_root/filter-ran"
  local capture_list="$test_root/capture-list"
  local capture_dir="$test_root/captured"
  local executable_meta="$test_root/executable-meta"
  local link_meta="$test_root/link-meta"
  local commit_oid
  local path
  local blob_oid
  local newline_path=$'raw/line\nbreak.txt'
  local index=0
  directory=$(new_repo raw-materialization)
  mkdir -p "$directory/raw"
  printf 'line one\nline two\n' > "$directory/raw/eol.txt"
  printf 'encoded text\n' | iconv -f UTF-8 -t UTF-16LE > "$directory/raw/encoded.txt"
  printf '%s\n' "ident \$Id\$" > "$directory/raw/ident.txt"
  printf 'filtered content\n' > "$directory/raw/filtered.txt"
  printf 'line name\n' > "$directory/$newline_path"
  printf '#!/bin/sh\nexit 0\n' > "$directory/raw/executable.sh"
  chmod 755 "$directory/raw/executable.sh"
  ln -s eol.txt "$directory/raw/link"
  cat > "$directory/.gitattributes" <<'EOF'
raw/eol.txt text eol=crlf
raw/encoded.txt working-tree-encoding=UTF-16LE
raw/ident.txt ident
raw/filtered.txt filter=hostile
EOF
  cat > "$clean_filter" <<'EOF'
#!/bin/sh
cat
EOF
  chmod 755 "$clean_filter"
  cat > "$filter_script" <<'EOF'
#!/bin/sh
printf 'ran\n' >> "$FILTER_MARKER"
cat >/dev/null
printf 'hostile output\n'
EOF
  chmod 755 "$filter_script"
  git config -f "$global_config" core.autocrlf true
  git config -f "$global_config" filter.hostile.clean "\"$clean_filter\""
  git config -f "$global_config" filter.hostile.smudge "\"$filter_script\""
  git config -f "$global_config" filter.hostile.required true
  commit_oid=$(GIT_CONFIG_GLOBAL="$global_config" commit_all "$directory" raw-inputs)
  printf '%s\0' raw/eol.txt raw/encoded.txt raw/ident.txt raw/filtered.txt "$newline_path" > "$capture_list"
  mkdir -p "$capture_dir"
  : > "$fake_log"
  (cd "$directory" && PATH="$fake_bin:$real_path" FAKE_GENERATED_DIR="$canonical" FAKE_LOG="$fake_log" FAKE_INSTALL_DIR="$test_root/fake-installed" GIT_CONFIG_GLOBAL="$global_config" FILTER_MARKER="$marker" FAKE_CAPTURE_LIST="$capture_list" FAKE_CAPTURE_DIR="$capture_dir" FAKE_CAPTURE_EXECUTABLE=raw/executable.sh FAKE_CAPTURE_META="$executable_meta" FAKE_CAPTURE_SYMLINK=raw/link FAKE_CAPTURE_LINK="$link_meta" bash "$checker" pre-commit)
  while IFS= read -r -d '' path || [[ -n "$path" ]]; do
    index=$((index + 1))
    blob_oid=$(git -C "$directory" rev-parse "$commit_oid:$path")
    git -C "$directory" cat-file blob "$blob_oid" > "$test_root/expected-$index"
    if cmp -s "$test_root/expected-$index" "$capture_dir/$index"; then
      printf 'PASS checkout bypass preserved blob bytes for %s\n' "$path"
    else
      printf 'FAIL checkout bypass changed blob bytes for %s\n' "$path" >&2
      return 1
    fi
  done < "$capture_list"
  assert_equal 'hostile smudge filter was not executed' absent "$(if [[ -e "$marker" ]]; then printf present; else printf absent; fi)"
  assert_equal 'executable bit was materialized' executable "$(cat "$executable_meta")"
  assert_equal 'symlink target was materialized' eol.txt "$(cat "$link_meta")"
}

test_relative_tmpdir() {
  local directory
  local relative_base='relative temp base'
  local residue
  directory=$(new_repo relative-tmpdir)
  mkdir -p "$directory/$relative_base"
  assert_success 'relative TMPDIR with spaces passes' run_relative_precommit "$directory" "$relative_base"
  residue=$(find "$directory/$relative_base" -mindepth 1 -print -quit)
  assert_equal 'relative TMPDIR leaves no temporary files' '' "$residue"
  printf 'staged drift\n' > "$directory/.github/workflows/generated.yml"
  git -C "$directory" add .github/workflows/generated.yml
  assert_failure 'relative TMPDIR drift fails' run_relative_precommit "$directory" "$relative_base"
  residue=$(find "$directory/$relative_base" -mindepth 1 -print -quit)
  assert_equal 'failed relative TMPDIR check cleans up' '' "$residue"
}

run_relative_precommit() {
  local directory=$1
  local relative_base=$2
  (cd "$directory" && TMPDIR="$relative_base" PATH="$fake_bin:$real_path" FAKE_GENERATED_DIR="$canonical" FAKE_LOG="$fake_log" FAKE_INSTALL_DIR="$test_root/fake-installed" bash "$checker" pre-commit)
}

test_generated_shape() {
  local directory
  local records
  local clean_oid
  local drift_oid
  directory=$(new_repo generated-shape)
  clean_oid=$(git -C "$directory" rev-parse HEAD)
  records=$(push_record refs/heads/main "$clean_oid" refs/heads/main "$zero_oid")
  assert_success 'generated contents, executable mode, and symlink target match' run_fake_checker "$directory" pre-push "$records"
  printf 'content drift\n' > "$directory/.github/workflows/generated.yml"
  drift_oid=$(commit_all "$directory" content-drift)
  records=$(push_record refs/heads/content-drift "$drift_oid" refs/heads/content-drift "$zero_oid")
  assert_failure 'generated content drift fails' run_fake_checker "$directory" pre-push "$records"
  git -C "$directory" checkout --quiet "$clean_oid" -- .github/workflows/generated.yml .github/workflows/run.sh .github/workflows/current
  rm "$directory/.github/workflows/current"
  ln -s missing.yml "$directory/.github/workflows/current"
  drift_oid=$(commit_all "$directory" symlink-drift)
  records=$(push_record refs/heads/symlink-drift "$drift_oid" refs/heads/symlink-drift "$zero_oid")
  assert_failure 'generated symlink target drift fails' run_fake_checker "$directory" pre-push "$records"
  git -C "$directory" checkout --quiet "$clean_oid" -- .github/workflows/generated.yml .github/workflows/current .github/workflows/run.sh
  chmod 644 "$directory/.github/workflows/run.sh"
  drift_oid=$(commit_all "$directory" mode-drift)
  records=$(push_record refs/heads/mode-drift "$drift_oid" refs/heads/mode-drift "$zero_oid")
  assert_failure 'generated executable mode drift fails' run_fake_checker "$directory" pre-push "$records"
}

test_partial_clones() {
  local directory
  local origin="$test_root/partial origin.git"
  local blobless="$test_root/blobless clone"
  local tree_filtered="$test_root/tree filtered clone"
  local unavailable="$test_root/unavailable clone"
  local sparse="$test_root/sparse clone"
  local commit_oid
  local raw_oid
  local record
  local capture_list="$test_root/partial-capture-list"
  local capture_dir="$test_root/partial-captured"
  directory=$(new_repo partial-source)
  printf 'promised bytes\n' > "$directory/raw.txt"
  commit_oid=$(commit_all "$directory" partial-source)
  git clone --quiet --bare "$directory" "$origin"
  git -C "$origin" config uploadpack.allowFilter true
  git -C "$origin" config uploadpack.allowAnySHA1InWant true
  git -c protocol.file.allow=always clone --quiet --filter=blob:none --no-checkout --no-local "$origin" "$blobless"
  raw_oid=$(git -C "$origin" rev-parse "$commit_oid:raw.txt")
  if GIT_NO_LAZY_FETCH=1 git -C "$blobless" cat-file -e "$raw_oid" 2>/dev/null; then
    printf 'FAIL blobless clone unexpectedly contains the required blob\n' >&2
    return 1
  fi
  printf 'raw.txt\0' > "$capture_list"
  mkdir -p "$capture_dir"
  record=$(push_record refs/heads/main "$commit_oid" refs/heads/main "$zero_oid")
  : > "$fake_log"
  assert_success 'blobless clone fetches required blobs through source Git' run_fake_checker_with_capture "$blobless" pre-push "$record" "$capture_list" "$capture_dir"
  git -C "$blobless" cat-file -e "$raw_oid"
  git -C "$origin" cat-file blob "$raw_oid" > "$test_root/partial-expected"
  if cmp -s "$test_root/partial-expected" "$capture_dir/1"; then
    printf 'PASS blobless materialization matches the promised blob bytes\n'
  else
    printf 'FAIL blobless materialization changed promised blob bytes\n' >&2
    return 1
  fi
  [[ ! -e "$blobless/.github" ]]
  git -c protocol.file.allow=always clone --quiet --filter=tree:0 --no-checkout --no-local "$origin" "$tree_filtered"
  record=$(push_record refs/heads/main "$commit_oid" refs/heads/main "$zero_oid")
  assert_success 'tree-filtered clone fetches nested trees through source Git' run_fake_checker "$tree_filtered" pre-push "$record"
  git -c protocol.file.allow=always clone --quiet --filter=blob:none --no-checkout --no-local "$origin" "$unavailable"
  git -C "$unavailable" remote set-url origin "$test_root/missing promised-object remote"
  : > "$fake_log"
  assert_failure 'unavailable promised object fails before generation' run_fake_checker "$unavailable" pre-push "$record"
  assert_equal 'unavailable promised object does not run generation' 0 "$(count_execs)"
  git -c protocol.file.allow=always clone --quiet --no-checkout --no-local "$origin" "$sparse"
  git -C "$sparse" sparse-checkout init --cone
  git -C "$sparse" sparse-checkout set docs
  git -C "$sparse" checkout --quiet
  [[ ! -e "$sparse/.github" ]]
  assert_success 'sparse checkout validates the complete pushed tree' run_fake_checker "$sparse" pre-push "$record"
  [[ ! -e "$sparse/.github" ]]
}

file_sha256() {
  shasum -a 256 "$1" | awk '{print $1}'
}

create_workflow_commit() {
  local repository=$1
  local parent=$2
  local branch=$3
  local marker=$4
  local workflow=.github/workflows/ci.yml
  local fixture_index="$test_root/$branch.index"
  local tree
  local commit

  GIT_INDEX_FILE="$fixture_index" git -C "$repository" read-tree "$parent"
  git -C "$repository" show "$parent:$workflow" > "$repository/$workflow"
  printf '\n%s\n' "$marker" >> "$repository/$workflow"
  GIT_INDEX_FILE="$fixture_index" git -C "$repository" add -- "$workflow"
  tree=$(GIT_INDEX_FILE="$fixture_index" git -C "$repository" write-tree)
  commit=$(printf 'fixture %s\n' "$branch" | env GIT_AUTHOR_NAME='Baseline Hook Test' GIT_AUTHOR_EMAIL=baseline-hook@example.invalid GIT_COMMITTER_NAME='Baseline Hook Test' GIT_COMMITTER_EMAIL=baseline-hook@example.invalid git -C "$repository" commit-tree "$tree" -p "$parent")
  git -C "$repository" update-ref "refs/heads/$branch" "$commit"
  git -C "$repository" show "$parent:$workflow" > "$repository/$workflow"
  printf '%s\n' "$commit"
}

create_file_commit() {
  local repository=$1
  local parent=$2
  local branch=$3
  local file_path=$4
  local marker=$5
  local fixture_index="$test_root/$branch.index"
  local tree
  local commit

  GIT_INDEX_FILE="$fixture_index" git -C "$repository" read-tree "$parent"
  git -C "$repository" show "$parent:$file_path" > "$repository/$file_path"
  printf '\n%s\n' "$marker" >> "$repository/$file_path"
  GIT_INDEX_FILE="$fixture_index" git -C "$repository" add -- "$file_path"
  tree=$(GIT_INDEX_FILE="$fixture_index" git -C "$repository" write-tree)
  commit=$(printf 'fixture %s\n' "$branch" | env GIT_AUTHOR_NAME='Baseline Hook Test' GIT_AUTHOR_EMAIL=baseline-hook@example.invalid GIT_COMMITTER_NAME='Baseline Hook Test' GIT_COMMITTER_EMAIL=baseline-hook@example.invalid git -C "$repository" commit-tree "$tree" -p "$parent")
  git -C "$repository" update-ref "refs/heads/$branch" "$commit"
  git -C "$repository" show "$parent:$file_path" > "$repository/$file_path"
  printf '%s\n' "$commit"
}

wrap_installed_hook() {
  local hook_path=$1
  local original_path=$2

  cp -p "$hook_path" "$original_path"
  cat > "$hook_path" <<'EOF'
#!/bin/sh
before_tree=$(git write-tree) || exit 125
before_config=$(shasum -a 256 "$HOOK_CONFIG_FILE" | awk '{print $1}') || exit 125
effective_index=${GIT_INDEX_FILE:-${GIT_DIR:-.git}/index}
before_index=$(shasum -a 256 "$effective_index" | awk '{print $1}') || exit 125
"$HOOK_ORIGINAL" "$@"
hook_status=$?
after_tree=$(git write-tree) || exit 125
after_config=$(shasum -a 256 "$HOOK_CONFIG_FILE" | awk '{print $1}') || exit 125
after_index=$(shasum -a 256 "$effective_index" | awk '{print $1}') || exit 125
printf '%s\n%s\n%s\n%s\n%s\n%s\n%s\n%s\n%s\n%s\n%s\n' "${GIT_DIR-}" "${GIT_INDEX_FILE-}" "$effective_index" "${GIT_WORK_TREE-}" "$before_config" "$after_config" "$before_index" "$after_index" "$before_tree" "$after_tree" "$hook_status" > "$HOOK_CONTEXT_TRACE"
exit "$hook_status"
EOF
  chmod 755 "$hook_path"
}

assert_hook_trace() {
  local label=$1
  local trace_file=$2
  local expected_status=$3
  local fixture_gitdir=$4
  local fixture_index=$5

  assert_equal "$label Git directory" "$fixture_gitdir" "$(sed -n '1p' "$trace_file")"
  local raw_index
  raw_index=$(sed -n '2p' "$trace_file")
  if [[ -n "$raw_index" ]]; then
    assert_equal "$label explicit index path" "$fixture_index" "$raw_index"
  fi
  assert_equal "$label effective index path" "$fixture_index" "$(sed -n '3p' "$trace_file")"
  assert_equal "$label source config unchanged inside hook" "$(sed -n '5p' "$trace_file")" "$(sed -n '6p' "$trace_file")"
  assert_equal "$label source index unchanged inside hook" "$(sed -n '7p' "$trace_file")" "$(sed -n '8p' "$trace_file")"
  assert_equal "$label staged tree unchanged inside hook" "$(sed -n '9p' "$trace_file")" "$(sed -n '10p' "$trace_file")"
  assert_equal "$label checker status" "$expected_status" "$(sed -n '11p' "$trace_file")"
}

assert_checker_context() {
  local label=$1
  local trace_file=$2
  local fixture_gitdir=$3
  local fixture_index=$4

  local raw_index
  assert_equal "$label checker Git directory" "$fixture_gitdir" "$(sed -n '1p' "$trace_file")"
  raw_index=$(sed -n '2p' "$trace_file")
  if [[ -n "$raw_index" ]]; then
    assert_equal "$label checker explicit index path" "$fixture_index" "$raw_index"
  fi
  assert_equal "$label checker effective index path" "$fixture_index" "$(sed -n '3p' "$trace_file")"
}

test_installed_hooks() {
  local fixture_root="$test_root/installed hook fixture"
  local seed="$fixture_root/seed"
  local installed_worktree="$fixture_root/worktree"
  local bare_remote="$fixture_root/remote.git"
  local fixture_common
  local fixture_gitdir
  local fixture_config
  local fixture_index
  local fixture_config_after_install
  local base_commit
  local head_drift_commit
  local push_drift_commit
  local clean_push_commit
  local workflow=.github/workflows/ci.yml
  local hk_pin
  local hk_dir
  local hk_executable
  local hk_version
  local install_log="$fixture_root/hk-install.log"
  local pre_commit_hook
  local pre_push_hook
  local commit_msg_hook
  local original_pre_commit="$fixture_root/pre-commit.hk"
  local original_pre_push="$fixture_root/pre-push.hk"
  local pre_commit_trace="$fixture_root/pre-commit-context.log"
  local pre_push_trace="$fixture_root/pre-push-context.log"
  local checker_env_script="$fixture_root/capture-checker-env.sh"
  local checker_env_trace="$fixture_root/checker-context.log"
  local pre_commit_pass_log="$fixture_root/pre-commit-pass.log"
  local pre_commit_drift_log="$fixture_root/pre-commit-drift.log"
  local pre_push_pass_log="$fixture_root/pre-push-pass.log"
  local pre_push_drift_log="$fixture_root/pre-push-drift.log"
  local commit_msg_pass_trace="$fixture_root/commit-msg-after-pass.log"
  local commit_msg_drift_trace="$fixture_root/commit-msg-after-drift.log"
  local host_common
  local host_gitdir
  local host_config
  local host_index
  local host_config_before
  local host_index_before
  local host_trust_before
  local hook_state="$fixture_root/mise-state"
  local hook_config="$fixture_root/mise-config"
  local hook_temp="$fixture_root/tmp"
  local status_code
  local before_tree
  local after_tree
  local remote_ref
  local -a hook_mise_environment

  mkdir -p "$fixture_root" "$hook_state" "$hook_config" "$hook_temp"
  : > "$hook_config/global.toml"
  : > "$hook_config/system.toml"
  host_common=$(git -C "$repo_root" rev-parse --path-format=absolute --git-common-dir)
  host_gitdir=$(git -C "$repo_root" rev-parse --path-format=absolute --git-dir)
  host_config="$host_common/config"
  host_index="$host_gitdir/index"
  host_config_before=$(file_sha256 "$host_config")
  host_index_before=$(file_sha256 "$host_index")
  host_trust_before=$("$mise_path" trust --show 2>&1)

  hk_pin=$("$mise_path" --cd "$repo_root" config get --file "$repo_root/mise.toml" tools.hk)
  hk_dir=$("$mise_path" --cd "$repo_root" where hk)
  hk_executable="$hk_dir/hk"
  hk_version=$("$hk_executable" --version)
  assert_equal 'installed hook test uses pinned hk' "hk $hk_pin" "$hk_version"

  git clone --quiet --no-hardlinks "$repo_root" "$seed"
  if git config --file "$seed/.git/config" --get core.worktree >/dev/null 2>&1; then
    printf 'FAIL disposable clone inherited a core.worktree override\n' >&2
    return 1
  fi
  base_commit=$(git -C "$seed" rev-parse HEAD)
  head_drift_commit=$(create_workflow_commit "$seed" "$base_commit" hook-test-head-drift 'head-only drift')
  push_drift_commit=$(create_workflow_commit "$seed" "$base_commit" hook-test-push-drift 'push-only drift')
  clean_push_commit=$(create_file_commit "$seed" "$base_commit" hook-test-push-clean docs/ci.md 'clean push fixture')
  git -C "$seed" update-ref refs/heads/hook-test-push-clean "$clean_push_commit"
  git -C "$seed" worktree add --quiet "$installed_worktree" hook-test-head-drift
  cp -p "$checker" "$installed_worktree/scripts/check-generated-workflows.sh"
  assert_equal 'fixture installs the selected checker' "$(file_sha256 "$checker")" "$(file_sha256 "$installed_worktree/scripts/check-generated-workflows.sh")"
  git init --bare --quiet "$bare_remote"
  git -C "$seed" push --quiet "$bare_remote" "$base_commit:refs/heads/codex/baseline-s2"
  git -C "$installed_worktree" remote add fixture "$bare_remote"
  git -C "$installed_worktree" fetch --quiet fixture codex/baseline-s2

  hook_mise_environment=(
    "MISE_STATE_DIR=$hook_state"
    "MISE_CONFIG_DIR=$hook_config"
    "MISE_GLOBAL_CONFIG_FILE=$hook_config/global.toml"
    "MISE_SYSTEM_CONFIG_FILE=$hook_config/system.toml"
    "MISE_OVERRIDE_CONFIG_FILENAMES=mise.toml"
    "MISE_CEILING_PATHS=$fixture_root"
    'MISE_TRUSTED_CONFIG_PATHS='
    "MISE_ENABLE_TOOLS=hk,github:tailrocks/velnor-new,rust,jactionlint"
    'MISE_ENV='
    "TMPDIR=$hook_temp"
  )
  env "${hook_mise_environment[@]}" "$mise_path" --cd "$installed_worktree" trust "$installed_worktree/mise.toml" </dev/null > "$fixture_root/trust-mise.log" 2>&1
  env "${hook_mise_environment[@]}" "$mise_path" --cd "$installed_worktree" trust "$installed_worktree/rust-toolchain.toml" </dev/null > "$fixture_root/trust-rust.log" 2>&1

  if "$hk_executable" --cd "$installed_worktree" install --force-local --legacy --mise > "$install_log" 2>&1; then
    printf 'PASS installed pinned hk hooks in a disposable linked worktree\n'
  else
    cat "$install_log"
    printf 'FAIL installed hk hooks in a disposable linked worktree\n' >&2
    return 1
  fi

  fixture_common=$(git -C "$installed_worktree" rev-parse --path-format=absolute --git-common-dir)
  fixture_gitdir=$(git -C "$installed_worktree" rev-parse --path-format=absolute --git-dir)
  fixture_config="$fixture_common/config"
  fixture_index="$fixture_gitdir/index"
  fixture_config_after_install=$(file_sha256 "$fixture_config")
  pre_commit_hook=$(git -C "$installed_worktree" rev-parse --path-format=absolute --git-path hooks/pre-commit)
  pre_push_hook=$(git -C "$installed_worktree" rev-parse --path-format=absolute --git-path hooks/pre-push)
  commit_msg_hook=$(git -C "$installed_worktree" rev-parse --path-format=absolute --git-path hooks/commit-msg)
  [[ -f "$pre_commit_hook" && -f "$pre_push_hook" ]]
  wrap_installed_hook "$pre_commit_hook" "$original_pre_commit"
  wrap_installed_hook "$pre_push_hook" "$original_pre_push"
  cat > "$commit_msg_hook" <<'EOF'
#!/bin/sh
printf 'rejected after pre-commit\n' > "$HOOK_COMMIT_MSG_TRACE"
exit 1
EOF
  chmod 755 "$commit_msg_hook"
  cat > "$checker_env_script" <<'EOF'
case "$0" in
  *check-generated-workflows.sh)
    effective_index=${GIT_INDEX_FILE:-${GIT_DIR:-.git}/index}
    printf '%s\n%s\n%s\n%s\n%s\n%s\n' "${GIT_DIR-}" "${GIT_INDEX_FILE-}" "$effective_index" "${GIT_WORK_TREE-}" "${GIT_COMMON_DIR-}" "${GIT_PREFIX-}" > "$HOOK_CHECKER_ENV_TRACE"
    ;;
esac
EOF

  git -C "$installed_worktree" show "$base_commit:$workflow" > "$installed_worktree/$workflow"
  git -C "$installed_worktree" add -- "$workflow"
  printf '\nworktree-only drift\n' >> "$installed_worktree/$workflow"
  before_tree=$(git -C "$installed_worktree" write-tree)
  assert_equal 'pre-commit fixture stages the clean tree' "$(git -C "$seed" rev-parse "$base_commit^{tree}")" "$before_tree"
  assert_failure 'pre-commit fixture has distinct unstaged content' git -C "$installed_worktree" diff --quiet -- "$workflow"
  if env "${hook_mise_environment[@]}" BASH_ENV="$checker_env_script" HOOK_CHECKER_ENV_TRACE="$checker_env_trace" HOOK_CONTEXT_TRACE="$pre_commit_trace" HOOK_ORIGINAL="$original_pre_commit" HOOK_CONFIG_FILE="$fixture_config" HOOK_COMMIT_MSG_TRACE="$commit_msg_pass_trace" GIT_AUTHOR_NAME='Baseline Hook Test' GIT_AUTHOR_EMAIL=baseline-hook@example.invalid GIT_COMMITTER_NAME='Baseline Hook Test' GIT_COMMITTER_EMAIL=baseline-hook@example.invalid git -C "$installed_worktree" commit --no-gpg-sign -m 'hook pre-commit positive' > "$pre_commit_pass_log" 2>&1; then
    status_code=0
  else
    status_code=$?
  fi
  cat "$pre_commit_pass_log"
  [[ -f "$pre_commit_trace" ]] || { printf 'FAIL installed pre-commit hook context was not captured\n' >&2; return 1; }
  cat "$pre_commit_trace"
  cat "$checker_env_trace"
  printf 'fixture_core_worktree_after_pre_commit=%s\n' "$(git config --file "$fixture_config" --get core.worktree || true)"
  assert_hook_trace 'clean staged pre-commit' "$pre_commit_trace" 0 "$fixture_gitdir" "$fixture_index"
  [[ -f "$commit_msg_pass_trace" ]] || { printf 'FAIL installed pre-commit did not reach fixture commit-msg (exit %s)\n' "$status_code" >&2; return 1; }
  assert_equal 'fixture commit stops after successful pre-commit' '1' "$status_code"
  assert_checker_context 'clean staged pre-commit' "$checker_env_trace" "$fixture_gitdir" "$fixture_index"
  after_tree=$(git -C "$installed_worktree" write-tree)
  assert_equal 'pre-commit preserves the staged tree through commit rejection' "$before_tree" "$after_tree"
  assert_equal 'pre-commit preserves fixture config outside hook' "$fixture_config_after_install" "$(file_sha256 "$fixture_config")"
  assert_equal 'pre-commit removes its temporary snapshot' '' "$(find "$hook_temp" -maxdepth 1 -name 'jackin-generated-workflows.*' -print -quit)"
  printf 'PASS installed pre-commit checks the clean staged tree instead of HEAD or worktree\n'

  git -C "$installed_worktree" reset --hard --quiet "$base_commit"
  cp -p "$checker" "$installed_worktree/scripts/check-generated-workflows.sh"
  git -C "$installed_worktree" show "$base_commit:$workflow" > "$installed_worktree/$workflow"
  printf '\nstaged-only drift\n' >> "$installed_worktree/$workflow"
  git -C "$installed_worktree" add -- "$workflow"
  git -C "$installed_worktree" show "$base_commit:$workflow" > "$installed_worktree/$workflow"
  before_tree=$(git -C "$installed_worktree" write-tree)
  assert_distinct 'pre-commit drifted index differs from HEAD' "$before_tree" "$(git -C "$installed_worktree" rev-parse "$base_commit^{tree}")"
  assert_success 'pre-commit drift fixture has a clean worktree view' git -C "$installed_worktree" diff "$base_commit" --quiet -- "$workflow"
  if env "${hook_mise_environment[@]}" BASH_ENV="$checker_env_script" HOOK_CHECKER_ENV_TRACE="$checker_env_trace" HOOK_CONTEXT_TRACE="$pre_commit_trace" HOOK_ORIGINAL="$original_pre_commit" HOOK_CONFIG_FILE="$fixture_config" HOOK_COMMIT_MSG_TRACE="$commit_msg_drift_trace" GIT_AUTHOR_NAME='Baseline Hook Test' GIT_AUTHOR_EMAIL=baseline-hook@example.invalid GIT_COMMITTER_NAME='Baseline Hook Test' GIT_COMMITTER_EMAIL=baseline-hook@example.invalid git -C "$installed_worktree" commit --no-gpg-sign -m 'hook pre-commit drift' > "$pre_commit_drift_log" 2>&1; then
    status_code=0
  else
    status_code=$?
  fi
  cat "$pre_commit_drift_log"
  cat "$pre_commit_trace"
  cat "$checker_env_trace"
  [[ ! -e "$commit_msg_drift_trace" ]] || { printf 'FAIL drifted index reached commit-msg\n' >&2; return 1; }
  assert_contains 'drifted staged pre-commit reports generated drift' 'generated-workflows: drift in index snapshot' "$pre_commit_drift_log"
  assert_equal 'drifted staged pre-commit rejects before commit-msg' '1' "$status_code"
  assert_hook_trace 'drifted staged pre-commit' "$pre_commit_trace" 1 "$fixture_gitdir" "$fixture_index"
  assert_checker_context 'drifted staged pre-commit' "$checker_env_trace" "$fixture_gitdir" "$fixture_index"
  after_tree=$(git -C "$installed_worktree" write-tree)
  assert_equal 'drifted staged tree survives commit rejection' "$before_tree" "$after_tree"
  assert_equal 'drifted pre-commit preserves fixture config outside hook' "$fixture_config_after_install" "$(file_sha256 "$fixture_config")"
  assert_equal 'drifted pre-commit removes its temporary snapshot' '' "$(find "$hook_temp" -maxdepth 1 -name 'jackin-generated-workflows.*' -print -quit)"
  printf 'PASS installed pre-commit rejects staged generated drift while ignoring clean worktree content\n'

  git -C "$installed_worktree" reset --hard --quiet "$head_drift_commit"
  cp -p "$checker" "$installed_worktree/scripts/check-generated-workflows.sh"
  before_tree=$(git -C "$installed_worktree" write-tree)
  assert_distinct 'clean pushed tree differs from checked-out HEAD' "$(git -C "$seed" rev-parse "$clean_push_commit^{tree}")" "$before_tree"
  if env "${hook_mise_environment[@]}" BASH_ENV="$checker_env_script" HOOK_CHECKER_ENV_TRACE="$checker_env_trace" HOOK_CONTEXT_TRACE="$pre_push_trace" HOOK_ORIGINAL="$original_pre_push" HOOK_CONFIG_FILE="$fixture_config" git -C "$installed_worktree" push fixture refs/heads/hook-test-push-clean:refs/heads/hook-test-push-clean > "$pre_push_pass_log" 2>&1; then
    status_code=0
  else
    status_code=$?
  fi
  cat "$pre_push_pass_log"
  cat "$pre_push_trace"
  cat "$checker_env_trace"
  assert_equal 'clean non-HEAD push succeeds' '0' "$status_code"
  assert_hook_trace 'clean non-HEAD pre-push' "$pre_push_trace" 0 "$fixture_gitdir" "$fixture_index"
  assert_checker_context 'clean non-HEAD pre-push' "$checker_env_trace" "$fixture_gitdir" "$fixture_index"
  after_tree=$(git -C "$installed_worktree" write-tree)
  assert_equal 'clean pre-push preserves the checked-out staged tree' "$before_tree" "$after_tree"
  assert_equal 'clean pre-push preserves fixture config outside hook' "$fixture_config_after_install" "$(file_sha256 "$fixture_config")"
  remote_ref=$(git --git-dir="$bare_remote" rev-parse refs/heads/hook-test-push-clean)
  assert_equal 'clean non-HEAD commit reached the disposable remote' "$clean_push_commit" "$remote_ref"
  assert_equal 'clean pre-push removes its temporary snapshot' '' "$(find "$hook_temp" -maxdepth 1 -name 'jackin-generated-workflows.*' -print -quit)"
  printf 'PASS installed pre-push checks the clean non-HEAD push record\n'

  git -C "$installed_worktree" reset --hard --quiet "$base_commit"
  cp -p "$checker" "$installed_worktree/scripts/check-generated-workflows.sh"
  before_tree=$(git -C "$installed_worktree" write-tree)
  assert_distinct 'drifted pushed tree differs from checked-out HEAD' "$(git -C "$seed" rev-parse "$push_drift_commit^{tree}")" "$before_tree"
  if env "${hook_mise_environment[@]}" BASH_ENV="$checker_env_script" HOOK_CHECKER_ENV_TRACE="$checker_env_trace" HOOK_CONTEXT_TRACE="$pre_push_trace" HOOK_ORIGINAL="$original_pre_push" HOOK_CONFIG_FILE="$fixture_config" git -C "$installed_worktree" push fixture refs/heads/hook-test-push-drift:refs/heads/hook-test-push-drift > "$pre_push_drift_log" 2>&1; then
    status_code=0
  else
    status_code=$?
  fi
  cat "$pre_push_drift_log"
  cat "$pre_push_trace"
  cat "$checker_env_trace"
  assert_contains 'drifted non-HEAD pre-push reports pushed-tree drift' 'generated-workflows: drift in push-1 snapshot' "$pre_push_drift_log"
  assert_equal 'drifted non-HEAD push rejects' '1' "$status_code"
  assert_hook_trace 'drifted non-HEAD pre-push' "$pre_push_trace" 1 "$fixture_gitdir" "$fixture_index"
  assert_checker_context 'drifted non-HEAD pre-push' "$checker_env_trace" "$fixture_gitdir" "$fixture_index"
  after_tree=$(git -C "$installed_worktree" write-tree)
  assert_equal 'drifted pre-push preserves the checked-out staged tree' "$before_tree" "$after_tree"
  assert_equal 'drifted pre-push preserves fixture config outside hook' "$fixture_config_after_install" "$(file_sha256 "$fixture_config")"
  if git --git-dir="$bare_remote" show-ref --verify --quiet refs/heads/hook-test-push-drift; then
    printf 'FAIL rejected push updated the disposable remote ref\n' >&2
    return 1
  fi
  assert_equal 'drifted pre-push removes its temporary snapshot' '' "$(find "$hook_temp" -maxdepth 1 -name 'jackin-generated-workflows.*' -print -quit)"
  printf 'PASS installed pre-push rejects drift in a non-HEAD push record\n'

  assert_equal 'installed hook fixture leaves host common config unchanged' "$host_config_before" "$(file_sha256 "$host_config")"
  assert_equal 'installed hook fixture leaves host worktree index unchanged' "$host_index_before" "$(file_sha256 "$host_index")"
  assert_equal 'installed hook fixture leaves host mise trust unchanged' "$host_trust_before" "$("$mise_path" trust --show 2>&1)"
}

test_real_mise() {
  local host_trust_before
  local host_trust_after
  local real_temp="$test_root/real mise temp"
  mkdir -p "$real_temp"
  mkdir -p "$real_temp/config"
  : > "$real_temp/config/global.toml"
  : > "$real_temp/config/system.toml"
  host_trust_before=$(mise trust --show 2>&1)
  if (
    cd "$repo_root"
    PATH="$real_path" TMPDIR="$real_temp" MISE_PARANOID=1 MISE_STATE_DIR="$real_temp/state" MISE_CONFIG_DIR="$real_temp/config" MISE_GLOBAL_CONFIG_FILE="$real_temp/config/global.toml" MISE_SYSTEM_CONFIG_FILE="$real_temp/config/system.toml" MISE_TRUSTED_CONFIG_PATHS='' MISE_ENV='' MISE_OVERRIDE_CONFIG_FILENAMES=mise.toml bash "$checker" pre-commit
  ); then
    :
  else
    printf 'FAIL real mise/Velnor check\n' >&2
    return 1
  fi
  host_trust_after=$(mise trust --show 2>&1)
  assert_equal 'real mise run leaves host trust state unchanged' "$host_trust_before" "$host_trust_after"
  assert_equal 'real mise run removes its temporary trust state' '' "$(find "$real_temp" -maxdepth 1 -name 'jackin-generated-workflows.*' -print -quit)"
}

run_mise_probe() {
  local probe_root=$1
  local paranoid=$2
  local config_ceiling=${repo_root%/*}
  shift 2
  mkdir -p "$probe_root/config"
  : > "$probe_root/config/global.toml"
  : > "$probe_root/config/system.toml"
  (
    local environment_name
    unset MISE_PARANOID MISE_CONFIG_FILE MISE_GLOBAL_CONFIG_ROOT MISE_DEFAULT_CONFIG_FILENAME MISE_NO_CONFIG MISE_NO_ENV
    while IFS= read -r environment_name; do
      case "$environment_name" in
        MISE_*_VERSION) unset "$environment_name" ;;
      esac
    done < <(compgen -e)
    export MISE_STATE_DIR="$probe_root/state"
    export MISE_CONFIG_DIR="$probe_root/config"
    export MISE_GLOBAL_CONFIG_FILE="$probe_root/config/global.toml"
    export MISE_SYSTEM_CONFIG_FILE="$probe_root/config/system.toml"
    export MISE_OVERRIDE_CONFIG_FILENAMES=mise.toml
    export MISE_CEILING_PATHS="$config_ceiling"
    export MISE_TRUSTED_CONFIG_PATHS=''
    export MISE_ENV=''
    if [[ "$paranoid" == 1 ]]; then
      export MISE_PARANOID=1
    fi
    cd "$repo_root"
    "$mise_path" "$@" </dev/null
  )
}

test_mise_trust_semantics() {
  local normal_root="$test_root/mise normal"
  local paranoid_root="$test_root/mise paranoid"
  local host_trust_before
  local host_trust_after
  local normal_before
  local normal_output
  local paranoid_before
  local paranoid_output
  local pinned_version
  host_trust_before=$(mise trust --show 2>&1)
  normal_before=$(run_mise_probe "$normal_root" 1 trust --show 2>&1)
  if [[ "$normal_before" != *'mise.toml: untrusted'* ]]; then
    printf 'FAIL isolated normal mise state did not start untrusted: %s\n' "$normal_before" >&2
    return 1
  fi
  pinned_version=$(run_mise_probe "$normal_root" 1 config get --file "$repo_root/mise.toml" 'tools."github:tailrocks/velnor-new".version')
  normal_output=$(run_mise_probe "$normal_root" 0 exec github:tailrocks/velnor-new -- velnor-actions --version 2>&1)
  assert_equal 'normal noninteractive mise exec uses the snapshot pin' "velnor-actions $pinned_version" "$normal_output"
  printf 'PASS normal mise exec runs noninteractively from untrusted state\n'
  paranoid_before=$(run_mise_probe "$paranoid_root" 1 trust --show 2>&1)
  if [[ "$paranoid_before" != *'mise.toml: untrusted'* ]]; then
    printf 'FAIL isolated paranoid mise state did not start untrusted\n' >&2
    return 1
  fi
  if run_mise_probe "$paranoid_root" 1 exec github:tailrocks/velnor-new -- velnor-actions --version > "$test_root/paranoid-untrusted.log" 2>&1; then
    printf 'FAIL paranoid mise exec accepted untrusted config\n' >&2
    return 1
  fi
  printf 'PASS paranoid noninteractive mise exec rejects untrusted config\n'
  run_mise_probe "$paranoid_root" 1 trust "$repo_root/mise.toml"
  run_mise_probe "$paranoid_root" 1 trust "$repo_root/rust-toolchain.toml"
  paranoid_output=$(run_mise_probe "$paranoid_root" 1 exec github:tailrocks/velnor-new -- velnor-actions --version 2>&1)
  assert_equal 'explicit snapshot trust permits paranoid exec' "velnor-actions $pinned_version" "$paranoid_output"
  host_trust_after=$(mise trust --show 2>&1)
  assert_equal 'mise trust probes leave host trust unchanged' "$host_trust_before" "$host_trust_after"
}

test_invalid_real_pin() {
  local directory
  local commit_oid
  local record
  local sentinel_dir="$test_root/unpinned command"
  local sentinel="$test_root/unpinned-command-ran"
  directory=$(new_repo invalid-real-pin 0.0.0-invalid)
  commit_oid=$(git -C "$directory" rev-parse HEAD)
  mkdir -p "$sentinel_dir"
  cat > "$sentinel_dir/velnor-actions" <<EOF
#!/bin/sh
printf 'ran\\n' > "$sentinel"
exit 0
EOF
  chmod 755 "$sentinel_dir/velnor-actions"
  record=$(push_record refs/heads/main "$commit_oid" refs/heads/main "$zero_oid")
  assert_failure 'invalid snapshot Velnor pin fails' run_invalid_real_pin "$directory" "$sentinel_dir" "$record" "$root_checker"
  [[ ! -e "$sentinel" ]]
  printf 'PASS invalid snapshot pin does not fall back to PATH\n'
}

run_invalid_real_pin() {
  local directory=$1
  local sentinel_dir=$2
  local record=$3
  local checker_path=$4
  printf '%s\n' "$record" | (cd "$directory" && PATH="$sentinel_dir:$real_path" MISE_AUTO_INSTALL=0 bash "$checker_path" pre-push)
}

run_case() {
  case "$1" in
    pin) test_snapshot_pin ;;
    auxiliary) test_auxiliary_refs ;;
    destinations) test_push_destinations ;;
    replacements) test_replacements ;;
    raw) test_raw_materialization ;;
    tmpdir) test_relative_tmpdir ;;
    shape) test_generated_shape ;;
    partial) test_partial_clones ;;
    installed-hooks) test_installed_hooks ;;
    trust)
      test_mise_trust_semantics
      test_real_mise
      ;;
    invalid-pin) test_invalid_real_pin ;;
    all)
      test_snapshot_pin
      test_auxiliary_refs
      test_push_destinations
      test_replacements
      test_raw_materialization
      test_relative_tmpdir
      test_generated_shape
      test_partial_clones
      test_installed_hooks
      test_mise_trust_semantics
      test_real_mise
      test_invalid_real_pin
      ;;
    *)
      printf 'usage: %s [all|pin|auxiliary|destinations|replacements|raw|tmpdir|shape|partial|installed-hooks|trust|invalid-pin]\n' "$0" >&2
      return 2
      ;;
  esac
}

run_case "${1:-all}"
