#!/usr/bin/env bash
# SPDX-FileCopyrightText: 2026 Alexey Zhokhov
# SPDX-License-Identifier: Apache-2.0
set -euo pipefail

repo_root=$(cd "$(dirname "$0")/.." && pwd -P)
root_checker="$repo_root/scripts/check-generated-workflows.sh"
checker=${CHECKER_UNDER_TEST:-$root_checker}
if [[ "$checker" != /* ]]; then
  checker=$(cd "$(dirname "$checker")" && pwd -P)/$(basename "$checker")
fi
caller_path=$PATH
test_root=$(mktemp -d "${TMPDIR:-/tmp}/baseline boundary tests.XXXXXX")
trap 'find "$test_root" -depth -delete' EXIT
fixture_bin="$test_root/bin"
temporary_root="$test_root/tmp"
repository_template="$test_root/repository template"
trusted_seed_commit=a7413118476128a1c9b4f290277eb91ddf64ea7a
response_plan="$test_root/curl responses"
request_log="$test_root/curl requests"
environment_trace="$test_root/curl environment"
runtime_tmp_trace="$test_root/curl runtime TMPDIR"
observer_plan="$test_root/materialization observer"
last_output="$test_root/checker output"
last_status=0
object_length=$(git -C "$repo_root" rev-parse --show-object-format | awk '{ if ($0 == "sha1") print 40; else if ($0 == "sha256") print 64; else exit 1 }')
zero_oid=$(printf '%0*d' "$object_length" 0)

mkdir -p "$fixture_bin" "$temporary_root" "$repository_template/.github/workflows" "$repository_template/.velnor" "$repository_template/docs"
cp -p "$repo_root/Cargo.lock" "$repository_template/Cargo.lock"
cat > "$repository_template/Cargo.toml" <<'EOF'
[workspace]
members = []
resolver = "3"
EOF
cat > "$repository_template/mise.toml" <<'EOF'
[tools]
"github:tailrocks/velnor-new" = { version = "0.1.74", bin = "velnor-actions" }
jactionlint = "2.4.0"
shellcheck = "0.11.0"
rust = { version = "1.99.1", profile = "minimal", components = ["clippy", "rustfmt"], targets = ["aarch64-apple-darwin", "x86_64-apple-darwin", "aarch64-unknown-linux-gnu", "x86_64-unknown-linux-gnu"] }
EOF
cat > "$repository_template/rust-toolchain.toml" <<'EOF'
[toolchain]
channel = "1.99.1"
profile = "minimal"
components = ["clippy", "rustfmt"]
targets = ["aarch64-apple-darwin", "x86_64-apple-darwin", "aarch64-unknown-linux-gnu", "x86_64-unknown-linux-gnu"]
EOF
printf 'schema = 1\n' > "$repository_template/.velnor/config.toml"
printf 'name: fixture\n' > "$repository_template/.github/workflows/ci.yml"
printf 'fixture\n' > "$repository_template/docs/keep.md"
git -C "$repository_template" init --quiet --initial-branch=main
git -C "$repository_template" add -A
GIT_AUTHOR_NAME='Baseline Checker Test' GIT_AUTHOR_EMAIL=baseline-checker@example.invalid GIT_COMMITTER_NAME='Baseline Checker Test' GIT_COMMITTER_EMAIL=baseline-checker@example.invalid git -C "$repository_template" commit --quiet -m initial
git -C "$repository_template" fetch --quiet --no-tags --no-write-fetch-head "$repo_root" "$trusted_seed_commit"
git -C "$repository_template" update-ref refs/heads/trusted-seed "$trusted_seed_commit"

{
  printf '#!/bin/bash\ncontrol=%q\nrequest_log=%q\nenvironment_trace=%q\nruntime_tmp_trace=%q\nobserver_plan=%q\n' "$response_plan" "$request_log" "$environment_trace" "$runtime_tmp_trace" "$observer_plan"
  cat <<'EOF'
set -eu
[ "${1-}" = "--disable" ] || exit 91
for name in http_proxy https_proxy all_proxy no_proxy HTTP_PROXY HTTPS_PROXY ALL_PROXY NO_PROXY CURL_HOME CURL_CA_BUNDLE SSL_CERT_FILE SSL_CERT_DIR; do
  if printenv "$name" >/dev/null 2>&1; then
    printf 'leaked %s\n' "$name" > "$environment_trace"
    exit 92
  fi
done
printf 'clean\n' > "$environment_trace"
printf '%s\n' "${TMPDIR-}" > "$runtime_tmp_trace"
header_file=
url=
protocol=
connect_timeout=
max_time=
max_filesize=
while (($#)); do
  case "$1" in
    --disable|--silent|--show-error) shift ;;
    --proto) protocol=$2; shift 2 ;;
    --connect-timeout) connect_timeout=$2; shift 2 ;;
    --max-time) max_time=$2; shift 2 ;;
    --max-filesize) max_filesize=$2; shift 2 ;;
    --dump-header) header_file=$2; shift 2 ;;
    --header)
      case "$2" in
        'Accept: application/vnd.github+json'|'X-GitHub-Api-Version: 2022-11-28') shift 2 ;;
        *) exit 96 ;;
      esac
      ;;
    --location|-L|--proxy|-x|--cacert|--capath|--cert|--key|--insecure) exit 93 ;;
    https://*|http://*) url=$1; shift ;;
    *) exit 94 ;;
  esac
done
[[ "$protocol" == '=https' && "$connect_timeout" == 10 && "$max_time" == 30 && -n "$max_filesize" && -n "$header_file" && -n "$url" ]] || exit 95
printf '%s\n' "$url" >> "$request_log"
if [[ -s "$observer_plan" && "$url" == https://static.rust-lang.org/* ]]; then
  python3 -I -S - "$observer_plan" "$TMPDIR" <<'PY'
import json
import sys
from pathlib import Path

plan_path, runtime_tmp = sys.argv[1:]
plan = json.loads(Path(plan_path).read_text())
checker_root = Path(runtime_tmp).parent.parent
snapshot = checker_root / plan["snapshot"] / plan["path"]
if not snapshot.is_file() or snapshot.is_symlink():
    Path(plan["status"]).write_text("error: materialized snapshot file is unavailable\n")
else:
    Path(plan["output"]).write_bytes(snapshot.read_bytes())
    Path(plan["status"]).write_text("present\n" if Path(plan["canary"]).exists() else "absent\n")
PY
fi
request_number=$(wc -l < "$request_log" | tr -d ' ')
status=$(sed -n "$((request_number * 3 - 2))p" "$control")
location=$(sed -n "$((request_number * 3 - 1))p" "$control")
body_file=$(sed -n "$((request_number * 3))p" "$control")
if [[ "$status" == SLOW ]]; then
  exec python3 -c 'import time; time.sleep(35)'
fi
[[ -n "$status" ]] || status=502
printf 'HTTP/1.1 %s Fixture\r\n' "$status" > "$header_file"
if [[ -n "$location" && "$location" != '-' ]]; then
  printf 'Location: %s\r\n' "$location" >> "$header_file"
fi
printf '\r\n' >> "$header_file"
if [[ -n "$body_file" && "$body_file" != '-' ]]; then
  cat "$body_file"
fi
EOF
} > "$fixture_bin/curl"
chmod 755 "$fixture_bin/curl"

host_target=$(case "$(uname -s):$(uname -m)" in
  Darwin:arm64|Darwin:aarch64) printf 'aarch64-apple-darwin' ;;
  Linux:x86_64|Linux:amd64) printf 'x86_64-unknown-linux-gnu' ;;
  *) printf 'unsupported' ;;
esac)
bootstrap_path="/rustup/dist/$host_target/rustup-init"
checksum_path="$bootstrap_path.sha256"

set_response_plan() {
  : > "$response_plan"
  : > "$observer_plan"
  while (($#)); do
    printf '%s\n%s\n%s\n' "$1" "$2" "$3" >> "$response_plan"
    shift 3
  done
  : > "$request_log"
  : > "$environment_trace"
  : > "$runtime_tmp_trace"
}

set_denied_response() {
  set_response_plan 502 - -
}

copy_checker_for_fixture() {
  local source=$1
  local directory=$2
  local destination="$directory/scripts/check-generated-workflows.sh"
  mkdir -p "$directory/scripts"
  cp -p "$source" "$destination"
  python3 - "$destination" "$directory" <<'PY'
import hashlib
import os
import re
import sys
from pathlib import Path

checker, root = map(Path, sys.argv[1:])
manifests = []
for directory, names, filenames in os.walk(root, followlinks=False):
    current = Path(directory)
    names[:] = [name for name in names if name != ".git" and not (current / name).is_symlink()]
    manifests.extend(current / name for name in filenames if name == "Cargo.toml")
digest = hashlib.sha256()
for manifest in sorted(manifests, key=lambda path: path.relative_to(root).as_posix()):
    digest.update(manifest.relative_to(root).as_posix().encode("utf-8"))
    digest.update(b"\0")
    digest.update(hashlib.sha256(manifest.read_bytes()).hexdigest().encode("ascii"))
    digest.update(b"\n")
contents = checker.read_text()
updated, replacements = re.subn(
    r"(?m)^snapshot_manifests_sha256=[0-9a-f]{64}$",
    f"snapshot_manifests_sha256={digest.hexdigest()}",
    contents,
)
if replacements != 1:
    raise SystemExit("checker fixture could not bind its reviewed manifest inventory")
checker.write_text(updated)
PY
}

new_repo() {
  local name=$1
  local directory="$test_root/$name"
  git clone --quiet "$repository_template" "$directory"
  copy_checker_for_fixture "$checker" "$directory"
  printf '%s\n' "$directory"
}

new_repo_without_trusted_seed() {
  local name=$1
  local directory
  directory=$(new_repo "$name")
  git -C "$directory" update-ref -d refs/remotes/origin/trusted-seed
  git -C "$directory" reflog expire --expire=now --all
  git -C "$directory" gc --prune=now --quiet
  if git -C "$directory" cat-file -e "$trusted_seed_commit^{commit}" 2>/dev/null; then
    printf 'FAIL %s fixture still contains the trusted seed commit\n' "$name" >&2
    return 1
  fi
  printf '%s\n' "$directory"
}

new_repo_with_missing_seed_object() {
  local name=$1
  local missing_kind=$2
  local requested_path=${3-}
  local directory
  local tree
  local listing="$test_root/$name-tree-listing"
  local object_list="$test_root/$name-object-list"
  local missing_file="$test_root/$name-missing-object"
  local pack="$test_root/$name-object-pack"
  local missing_object
  directory=$(new_repo_without_trusted_seed "$name")
  tree=$(git -C "$repo_root" rev-parse --verify "$trusted_seed_commit^{tree}")
  git -C "$repo_root" ls-tree -r -t -z --full-tree "$tree" > "$listing"
  python3 - "$listing" "$missing_kind" "$tree" "$trusted_seed_commit" "$object_list" "$missing_file" "$requested_path" <<'PY'
import sys
from pathlib import Path

listing_path, missing_kind, root_tree, commit, object_list_path, missing_path, requested_path = sys.argv[1:]
listing = open(listing_path, "rb").read()
if not listing.endswith(b"\0"):
    raise SystemExit("trusted tree listing is incomplete")
entries = []
objects = {commit, root_tree}
for raw in listing[:-1].split(b"\0"):
    header, path = raw.split(b"\t", 1)
    mode, kind, object_id = header.decode("ascii").split(" ")
    entries.append((mode, kind, object_id, path))
    objects.add(object_id)
missing_object = None
for mode, kind, object_id, path in entries:
    if mode == "040000" and kind == "tree" and missing_kind == "tree" and (not requested_path or path.decode() == requested_path):
        missing_object = object_id
        break
    if mode in {"100644", "100755", "120000"} and kind == "blob" and missing_kind == "blob" and (not requested_path or path.decode() == requested_path):
        missing_object = object_id
        break
if missing_object is None:
    raise SystemExit("trusted seed has no object of the requested kind")
Path(missing_path).write_text(missing_object + "\n")
objects.remove(missing_object)
Path(object_list_path).write_text("".join(object_id + "\n" for object_id in sorted(objects)))
PY
  python3 - "$repo_root" "$object_list" "$pack" <<'PY'
import subprocess
import sys
from pathlib import Path

repository, object_list, pack_path = sys.argv[1:]
objects = Path(object_list).read_text().splitlines()
with open(pack_path, "wb") as pack:
    subprocess.run(["git", "-C", repository, "pack-objects", "--stdout"], check=True, input=("\n".join(objects) + "\n").encode(), stdout=pack)
PY
  git -C "$directory" index-pack --stdin < "$pack" >/dev/null
  missing_object=$(cat "$missing_file")
  if git -C "$directory" cat-file -e "$missing_object" 2>/dev/null; then
    printf 'FAIL %s fixture retained the omitted Git object\n' "$name" >&2
    return 1
  fi
  printf '%s\n' "$directory"
}

configure_promisor_canaries() {
  local directory=$1
  local marker="$test_root/promisor credential helper"
  git -C "$directory" config --local extensions.partialClone attacker
  git -C "$directory" config --local remote.attacker.promisor true
  git -C "$directory" config --local remote.attacker.url https://promisor.example.invalid/repository
  git -C "$directory" config --local credential.helper "!touch '$marker'"
  git -C "$directory" config --local url.https://rewrite.example.invalid/.insteadOf https://github.com/
  git -C "$directory" config --local core.hooksPath "$test_root/attacker hooks"
  {
    printf '%s\n' '#!/bin/sh'
    printf 'touch %q\n' "$marker"
    printf '%s\n' 'exit 97'
  } > "$fixture_bin/git-remote-https"
  chmod 755 "$fixture_bin/git-remote-https"
}

push_record() {
  printf '%s %s %s %s\n' "$1" "$2" "$3" "$4"
}

run_checker() {
  local directory=$1
  local mode=$2
  local records=${3-}
  local fixture_checker="$directory/scripts/check-generated-workflows.sh"
  if (($# >= 3)); then
    shift 3
  else
    shift 2
  fi
  local -a child_environment=("PATH=$fixture_bin:$caller_path" "TMPDIR=$temporary_root")
  child_environment+=("$@")
  last_output="$test_root/checker output"
  if [[ "$mode" == pre-push ]]; then
    if printf '%s\n' "$records" | (cd "$directory" && env "${child_environment[@]}" bash "$fixture_checker" "$mode") > "$last_output" 2>&1; then
      last_status=0
    else
      last_status=$?
    fi
  else
    if (cd "$directory" && env "${child_environment[@]}" bash "$fixture_checker" "$mode") > "$last_output" 2>&1; then
      last_status=0
    else
      last_status=$?
    fi
  fi
}

reset_repo() {
  git -C "$1" reset --hard --quiet HEAD
  git -C "$1" clean --quiet -ffdqx
  mkdir -p "$1/scripts"
  copy_checker_for_fixture "$checker" "$1"
}

set_velnor_pin() {
  local directory=$1
  local pin=$2
  local replacement="$directory/mise.toml.test"
  sed "s/version = \"0\\.1\\.74\"/version = \"$pin\"/" "$directory/mise.toml" > "$replacement"
  mv "$replacement" "$directory/mise.toml"
}

assert_contains() {
  local label=$1
  local text=$2
  if rg -F -q "$text" "$last_output"; then
    printf 'PASS %s\n' "$label"
  else
    printf 'FAIL %s: missing <%s>\n' "$label" "$text" >&2
    cat "$last_output" >&2
    return 1
  fi
}

assert_not_contains() {
  local label=$1
  local text=$2
  if rg -F -q "$text" "$last_output"; then
    printf 'FAIL %s: unexpectedly found <%s>\n' "$label" "$text" >&2
    return 1
  fi
  printf 'PASS %s\n' "$label"
}

assert_request_not_contains() {
  local label=$1
  local text=$2
  if rg -F -q "$text" "$request_log"; then
    printf 'FAIL %s: requested prohibited destination <%s>\n' "$label" "$text" >&2
    return 1
  fi
  printf 'PASS %s\n' "$label"
}

assert_failed() {
  local label=$1
  if [[ "$last_status" -ne 0 ]]; then
    printf 'PASS %s\n' "$label"
  else
    printf 'FAIL %s: checker unexpectedly succeeded\n' "$label" >&2
    return 1
  fi
}

assert_equal() {
  local label=$1
  local expected=$2
  local actual=$3
  if [[ "$expected" == "$actual" ]]; then
    printf 'PASS %s\n' "$label"
  else
    printf 'FAIL %s: expected <%s>, got <%s>\n' "$label" "$expected" "$actual" >&2
    return 1
  fi
}

assert_request_count() {
  local expected=$1
  local actual
  actual=$(wc -l < "$request_log" | tr -d ' ')
  assert_equal 'curl boundary request count' "$expected" "$actual"
}

assert_no_temporary_snapshot() {
  local remaining
  remaining=$(find "$temporary_root" -mindepth 1 -maxdepth 1 -name 'jackin-generated-workflows.*' -print -quit)
  assert_equal 'checker cleans its temporary snapshot after failure' '' "$remaining"
}

file_sha256() {
  shasum -a 256 "$1" | awk '{print $1}'
}

seed_blob_oid() {
  git -C "$repo_root" rev-parse "$trusted_seed_commit:$1"
}

write_blob_response() {
  local object_id=$1
  local raw_path=$2
  local json_path=$3
  git -C "$repo_root" cat-file blob "$object_id" > "$raw_path"
  python3 - "$object_id" "$raw_path" "$json_path" <<'PY'
import base64
import json
import sys
from pathlib import Path

object_id, raw_path, json_path = sys.argv[1:]
payload = Path(raw_path).read_bytes()
document = {
    "sha": object_id,
    "size": len(payload),
    "encoding": "base64",
    "content": base64.b64encode(payload).decode("ascii"),
}
Path(json_path).write_text(json.dumps(document, separators=(",", ":")))
PY
}

write_selected_blob_response() {
  local object_id=$1
  local raw_path=$2
  local json_path=$3
  local variant=${4-}
  python3 - "$object_id" "$raw_path" "$json_path" "$variant" <<'PY'
import base64
import json
import sys
from pathlib import Path

object_id, raw_path, json_path, variant = sys.argv[1:]
payload = Path(raw_path).read_bytes()
document = {
    "sha": object_id,
    "size": len(payload),
    "encoding": "base64",
    "content": base64.b64encode(payload).decode("ascii"),
}
if variant == "wrong-content":
    payload = b"different Git blob bytes"
    document["size"] = len(payload)
    document["content"] = base64.b64encode(payload).decode("ascii")
elif variant == "wrong-sha":
    document["sha"] = "0" * 40
elif variant == "wrong-size":
    document["size"] += 1
elif variant == "bad-base64":
    document["content"] = "%%%"
elif variant == "oversized-blob":
    payload = b"x" * (4 * 1024 * 1024 + 1)
    document["size"] = len(payload)
    document["content"] = base64.b64encode(payload).decode("ascii")
Path(json_path).write_text(json.dumps(document, separators=(",", ":")))
PY
}

write_blob_response_variant() {
  local object_id=$1
  local raw_path=$2
  local json_path=$3
  local variant=$4
  git -C "$repo_root" cat-file blob "$object_id" > "$raw_path"
  python3 - "$object_id" "$raw_path" "$json_path" "$variant" <<'PY'
import base64
import json
import sys
from pathlib import Path

object_id, raw_path, json_path, variant = sys.argv[1:]
payload = Path(raw_path).read_bytes()
document = {
    "sha": object_id,
    "size": len(payload),
    "encoding": "base64",
    "content": base64.b64encode(payload).decode("ascii"),
}
if variant == "wrong-content":
    payload = b"different Git blob bytes"
    document["size"] = len(payload)
    document["content"] = base64.b64encode(payload).decode("ascii")
elif variant == "wrong-sha":
    document["sha"] = "0" * 40
elif variant == "wrong-size":
    document["size"] += 1
elif variant == "duplicate-key":
    encoded = document["content"]
    Path(json_path).write_text(
        "{\"sha\":\"" + object_id + "\",\"sha\":\"" + object_id + "\",\"size\":"
        + str(len(payload)) + ",\"encoding\":\"base64\",\"content\":\"" + encoded + "\"}"
    )
    raise SystemExit(0)
elif variant == "malformed-json":
    Path(json_path).write_text("{\"sha\":\"" + object_id)
    raise SystemExit(0)
elif variant == "bad-base64":
    document["content"] = "%%%"
elif variant == "truncated":
    payload = payload[:-1]
    document["content"] = base64.b64encode(payload).decode("ascii")
elif variant == "wrapped-base64":
    encoded = document["content"]
    document["content"] = "\n".join(encoded[index:index + 60] for index in range(0, len(encoded), 60))
elif variant == "oversized-blob":
    payload = b"x" * (4 * 1024 * 1024 + 1)
    document["size"] = len(payload)
    document["content"] = base64.b64encode(payload).decode("ascii")
Path(json_path).write_text(json.dumps(document, separators=(",", ":")))
PY
}

source_object_inventory() {
  local directory=$1
  local object_dir
  object_dir=$(git -C "$directory" rev-parse --path-format=absolute --git-path objects)
  python3 - "$object_dir" <<'PY'
import hashlib
import os
import stat
import sys
from pathlib import Path

root = Path(sys.argv[1])
digest = hashlib.sha256()
for directory, names, filenames in os.walk(root, followlinks=False):
    current = Path(directory)
    for name in sorted(names + filenames, key=os.fsencode):
        path = current / name
        relative = os.fsencode(path.relative_to(root))
        metadata = path.lstat()
        digest.update(relative + b"\0" + str(stat.S_IFMT(metadata.st_mode)).encode() + b"\0")
        if stat.S_ISREG(metadata.st_mode):
            digest.update(hashlib.sha256(path.read_bytes()).digest())
        elif stat.S_ISLNK(metadata.st_mode):
            digest.update(os.fsencode(os.readlink(path)))
print(digest.hexdigest())
PY
}

source_checkout_inventory() {
  python3 - "$1" <<'PY'
import hashlib
import os
import stat
import sys
from pathlib import Path

root = Path(sys.argv[1])
digest = hashlib.sha256()
for directory, names, filenames in os.walk(root, followlinks=False):
    current = Path(directory)
    names[:] = [name for name in names if name != ".git"]
    for name in sorted(names + filenames, key=os.fsencode):
        path = current / name
        metadata = path.lstat()
        relative = os.fsencode(path.relative_to(root))
        digest.update(relative + b"\0" + str(stat.S_IFMT(metadata.st_mode)).encode() + b"\0")
        if stat.S_ISREG(metadata.st_mode):
            digest.update(hashlib.sha256(path.read_bytes()).digest())
        elif stat.S_ISLNK(metadata.st_mode):
            digest.update(os.fsencode(os.readlink(path)))
print(digest.hexdigest())
PY
}

source_ref_inventory() {
  git -C "$1" for-each-ref --format='%(refname) %(objectname)' | LC_ALL=C sort
}

set_materialization_observer() {
  local snapshot=$1
  local path=$2
  local output=$3
  local canary=$4
  local status=$5
  python3 - "$observer_plan" "$snapshot" "$path" "$output" "$canary" "$status" <<'PY'
import json
import sys
from pathlib import Path

destination, snapshot, path, output, canary, status = sys.argv[1:]
Path(destination).write_text(json.dumps({"snapshot": snapshot, "path": path, "output": output, "canary": canary, "status": status}))
PY
}

test_clean_boundary() {
  local directory
  local index_file
  local git_config
  local index_before
  local config_before
  directory=$(new_repo clean-boundary)
  index_file=$(git -C "$directory" rev-parse --path-format=absolute --git-path index)
  git_config=$(git -C "$directory" rev-parse --path-format=absolute --git-common-dir)/config
  index_before=$(file_sha256 "$index_file")
  config_before=$(file_sha256 "$git_config")
  mkdir -p "$directory/.cargo"
  printf '[term]\ncolor = "never"\nverbose = false\n' > "$directory/.cargo/config.toml"
  git -C "$directory" add -- .cargo/config.toml
  index_before=$(file_sha256 "$index_file")
  set_denied_response
  run_checker "$directory" pre-commit
  assert_failed 'safe project reaches the deterministic network boundary'
  assert_contains 'network boundary returns the fixed HTTP failure' 'rustup bootstrap response has an unexpected status'
  assert_request_count 1
  assert_equal 'first request uses the fixed official checksum path' "https://static.rust-lang.org$checksum_path" "$(sed -n '1p' "$request_log")"
  assert_request_not_contains 'complete trusted seed performs no GitHub blob request' 'https://api.github.com/repos/tailrocks/terminal-components-baseline/git/blobs/'
  assert_equal 'curl environment excludes inherited proxy and CA settings' clean "$(cat "$environment_trace")"
  assert_equal 'pre-commit preserves the source index' "$index_before" "$(file_sha256 "$index_file")"
  assert_equal 'checker preserves the source Git config' "$config_before" "$(file_sha256 "$git_config")"
  assert_no_temporary_snapshot
}

test_trusted_seed_missing_commit() {
  local directory
  local index_file
  local index_before
  directory=$(new_repo_without_trusted_seed missing-trusted-seed)
  index_file=$(git -C "$directory" rev-parse --path-format=absolute --git-path index)
  index_before=$(file_sha256 "$index_file")
  set_denied_response
  run_checker "$directory" pre-commit
  assert_failed 'missing trusted seed commit fails closed'
  assert_contains 'missing trusted seed commit has a specific diagnostic' 'reviewed trusted seed commit object is unavailable'
  assert_request_count 0
  assert_equal 'missing trusted seed preserves the source index' "$index_before" "$(file_sha256 "$index_file")"
  assert_no_temporary_snapshot
}

test_trusted_seed_missing_objects() {
  local directory
  local index_file
  local config_file
  local index_before
  local config_before
  local expected
  local marker="$test_root/promisor credential helper"
  local kind=tree
  directory=$(new_repo_with_missing_seed_object "missing-trusted-$kind" "$kind")
  configure_promisor_canaries "$directory"
  index_file=$(git -C "$directory" rev-parse --path-format=absolute --git-path index)
  config_file=$(git -C "$directory" rev-parse --path-format=absolute --git-common-dir)/config
  index_before=$(file_sha256 "$index_file")
  config_before=$(file_sha256 "$config_file")
  set_denied_response
  run_checker "$directory" pre-commit \
    GIT_CONFIG_COUNT=1 \
    GIT_CONFIG_KEY_0=credential.helper \
    "GIT_CONFIG_VALUE_0=!touch '$marker'"
  assert_failed "missing trusted seed $kind fails closed"
  expected='cannot read the complete reviewed trusted seed tree'
  assert_contains "missing trusted seed $kind has a specific diagnostic" "$expected"
  assert_request_count 0
  assert_equal "missing trusted seed $kind preserves the source index" "$index_before" "$(file_sha256 "$index_file")"
  assert_equal "missing trusted seed $kind preserves local Git config" "$config_before" "$(file_sha256 "$config_file")"
  assert_equal "missing trusted seed $kind does not invoke the promisor credential helper" false "$(test -e "$marker" && printf true || printf false)"
  assert_no_temporary_snapshot
}

test_trusted_seed_blob_hydration() {
  local path=Cargo.toml
  local object_id
  local body="$test_root/trusted seed blob response.json"
  local raw="$test_root/trusted seed blob.raw"
  local directory
  local index_file
  local config_file
  local index_before
  local config_before
  local objects_before
  local status_before
  local marker="$test_root/promisor credential helper"
  local endpoint
  local record
  local mode
  object_id=$(seed_blob_oid "$path")
  endpoint="https://api.github.com/repos/tailrocks/terminal-components-baseline/git/blobs/$object_id"
  write_blob_response_variant "$object_id" "$raw" "$body" wrapped-base64
  directory=$(new_repo_with_missing_seed_object missing-trusted-included-blob blob "$path")
  configure_promisor_canaries "$directory"
  index_file=$(git -C "$directory" rev-parse --path-format=absolute --git-path index)
  config_file=$(git -C "$directory" rev-parse --path-format=absolute --git-common-dir)/config
  index_before=$(file_sha256 "$index_file")
  config_before=$(file_sha256 "$config_file")
  objects_before=$(source_object_inventory "$directory")
  status_before=$(git -C "$directory" status --porcelain=v1 --untracked-files=all | shasum -a 256 | awk '{print $1}')
  record=$(push_record refs/heads/main "$(git -C "$directory" rev-parse HEAD)" refs/heads/main "$zero_oid")
  for mode in pre-commit pre-push; do
    set_response_plan 200 - "$body" 502 - -
    if [[ "$mode" == pre-push ]]; then
      run_checker "$directory" "$mode" "$record" \
        GIT_CONFIG_COUNT=1 \
        GIT_CONFIG_KEY_0=credential.helper \
        "GIT_CONFIG_VALUE_0=!touch '$marker'"
    else
      run_checker "$directory" "$mode" '' \
        GIT_CONFIG_COUNT=1 \
        GIT_CONFIG_KEY_0=credential.helper \
        "GIT_CONFIG_VALUE_0=!touch '$marker'"
    fi
    assert_failed "$mode hydrates a missing included seed blob and reaches the deterministic bootstrap boundary"
    assert_contains "$mode reports the downstream network denial" 'rustup bootstrap response has an unexpected status'
    assert_not_contains "$mode does not stop at the missing historical blob" 'cannot materialize the reviewed trusted seed'
    assert_request_count 2
    assert_equal "$mode requests the fixed GitHub blob URL" "$endpoint" "$(sed -n '1p' "$request_log")"
    assert_equal "$mode proceeds only to the fixed Rustup checksum URL" "https://static.rust-lang.org$checksum_path" "$(sed -n '2p' "$request_log")"
    assert_equal "$mode transport has no inherited proxy or CA settings" clean "$(cat "$environment_trace")"
    assert_equal "$mode does not execute the source promisor credential helper" false "$(test -e "$marker" && printf true || printf false)"
    assert_equal "$mode preserves source Git objects" "$objects_before" "$(source_object_inventory "$directory")"
    assert_equal "$mode preserves source index" "$index_before" "$(file_sha256 "$index_file")"
    assert_equal "$mode preserves source Git config" "$config_before" "$(file_sha256 "$config_file")"
    assert_equal "$mode preserves source checkout state" "$status_before" "$(git -C "$directory" status --porcelain=v1 --untracked-files=all | shasum -a 256 | awk '{print $1}')"
    assert_no_temporary_snapshot
  done
}

test_trusted_seed_empty_blob_hydration() {
  local directory
  local temporary_index="$test_root/empty trusted seed index"
  local empty_oid
  local tree
  local synthetic_commit
  local all_objects="$test_root/empty trusted seed object list"
  local object_list="$test_root/empty trusted seed available object list"
  local pack="$test_root/empty trusted seed object pack"
  local object_dir
  local empty_object_path
  local mutation="$test_root/checker with empty seed fixture pin"
  local raw="$test_root/empty trusted seed blob"
  local body="$test_root/empty trusted seed response.json"
  local index_before
  local config_file
  local config_before
  local objects_before
  local endpoint
  directory=$(new_repo empty-trusted-seed-blob)
  GIT_INDEX_FILE="$temporary_index" git -C "$directory" read-tree "$trusted_seed_commit^{tree}"
  empty_oid=$(printf '' | git -C "$directory" hash-object -w --stdin)
  GIT_INDEX_FILE="$temporary_index" git -C "$directory" update-index --add --cacheinfo "100644,$empty_oid,docs/empty-seed-blob"
  tree=$(GIT_INDEX_FILE="$temporary_index" git -C "$directory" write-tree)
  synthetic_commit=$(printf 'empty blob seed fixture\n' | GIT_AUTHOR_NAME='Baseline Checker Test' GIT_AUTHOR_EMAIL=baseline-checker@example.invalid GIT_COMMITTER_NAME='Baseline Checker Test' GIT_COMMITTER_EMAIL=baseline-checker@example.invalid git -C "$directory" commit-tree "$tree" -p "$trusted_seed_commit")
  git -C "$directory" rev-list --objects --no-object-names "$synthetic_commit" > "$all_objects"
  python3 - "$all_objects" "$object_list" "$empty_oid" <<'PY'
import sys
from pathlib import Path

source, destination, missing = sys.argv[1:]
objects = Path(source).read_text().splitlines()
if missing not in objects:
    raise SystemExit("synthetic trusted seed does not contain the empty blob")
Path(destination).write_text("".join(object_id + "\n" for object_id in objects if object_id != missing))
PY
  python3 - "$directory" "$object_list" "$pack" <<'PY'
import subprocess
import sys

repository, object_list, pack_path = sys.argv[1:]
with open(object_list, "rb") as objects, open(pack_path, "wb") as pack:
    subprocess.run(["git", "-C", repository, "pack-objects", "--stdout"], check=True, stdin=objects, stdout=pack)
PY
  git -C "$directory" index-pack --stdin < "$pack" >/dev/null
  object_dir=$(git -C "$directory" rev-parse --path-format=absolute --git-path objects)
  empty_object_path="$object_dir/${empty_oid:0:2}/${empty_oid:2}"
  rm -f "$empty_object_path"
  if git -C "$directory" cat-file -e "$empty_oid" 2>/dev/null; then
    printf 'FAIL synthetic trusted seed fixture still contains the empty blob\n' >&2
    return 1
  fi
  python3 - "$root_checker" "$mutation" "$trusted_seed_commit" "$synthetic_commit" <<'PY'
import sys
from pathlib import Path

source, destination, original, replacement = sys.argv[1:]
contents = Path(source).read_text()
if contents.count(original) != 1:
    raise SystemExit("trusted seed pin mutation was not uniquely applicable")
Path(destination).write_text(contents.replace(original, replacement, 1))
PY
  copy_checker_for_fixture "$mutation" "$directory"
  : > "$raw"
  python3 - "$empty_oid" "$body" <<'PY'
import json
import sys
from pathlib import Path

object_id, destination = sys.argv[1:]
Path(destination).write_text(json.dumps({"sha": object_id, "size": 0, "encoding": "base64", "content": ""}, separators=(",", ":")))
PY
  config_file=$(git -C "$directory" rev-parse --path-format=absolute --git-common-dir)/config
  index_before=$(file_sha256 "$(git -C "$directory" rev-parse --path-format=absolute --git-path index)")
  config_before=$(file_sha256 "$config_file")
  objects_before=$(source_object_inventory "$directory")
  endpoint="https://api.github.com/repos/tailrocks/terminal-components-baseline/git/blobs/$empty_oid"
  set_response_plan 200 - "$body" 502 - -
  run_checker "$directory" pre-commit
  assert_failed 'legitimate empty seed blob hydrates and reaches the deterministic boundary'
  assert_contains 'empty seed blob hydration reaches the Rustup boundary' 'rustup bootstrap response has an unexpected status'
  assert_request_count 2
  assert_equal 'empty seed blob uses the fixed GitHub endpoint' "$endpoint" "$(sed -n '1p' "$request_log")"
  assert_equal 'empty seed blob hydration does not mutate source objects' "$objects_before" "$(source_object_inventory "$directory")"
  assert_equal 'empty seed blob hydration preserves the source index' "$index_before" "$(file_sha256 "$(git -C "$directory" rev-parse --path-format=absolute --git-path index)")"
  assert_equal 'empty seed blob hydration preserves the source Git config' "$config_before" "$(file_sha256 "$config_file")"
  assert_no_temporary_snapshot
}

assert_blob_hydration_rejected() {
  local directory=$1
  local object_id=$2
  local label=$3
  local status=$4
  local location=$5
  local body=$6
  local expected=$7
  local endpoint="https://api.github.com/repos/tailrocks/terminal-components-baseline/git/blobs/$object_id"
  set_response_plan "$status" "$location" "$body"
  run_checker "$directory" pre-commit
  assert_failed "$label fails closed"
  assert_request_count 1
  assert_equal "$label requests only the fixed blob endpoint" "$endpoint" "$(sed -n '1p' "$request_log")"
  assert_contains "$label reports the hydration boundary" "$expected"
  assert_request_not_contains "$label never requests the redirect target or Rustup" 'https://static.rust-lang.org/'
  assert_no_temporary_snapshot
}

test_trusted_seed_blob_response_validation() {
  local path=Cargo.toml
  local object_id
  local raw="$test_root/hydration boundary raw"
  local body="$test_root/hydration boundary response.json"
  local oversized="$test_root/oversized GitHub blob response"
  local location
  local directory
  local object_inventory
  object_id=$(seed_blob_oid "$path")
  directory=$(new_repo_with_missing_seed_object missing-trusted-boundary-blob blob "$path")
  object_inventory=$(source_object_inventory "$directory")

  write_blob_response_variant "$object_id" "$raw" "$body" wrong-content
  assert_blob_hydration_rejected "$directory" "$object_id" 'wrong blob bytes under a claimed OID' 200 - "$body" 'decoded blob does not match its Git object identity'

  write_blob_response_variant "$object_id" "$raw" "$body" wrong-sha
  assert_blob_hydration_rejected "$directory" "$object_id" 'wrong declared blob OID' 200 - "$body" 'response object identity does not match the requested blob'

  write_blob_response_variant "$object_id" "$raw" "$body" wrong-size
  assert_blob_hydration_rejected "$directory" "$object_id" 'wrong declared blob size' 200 - "$body" 'decoded blob size does not match the declaration'

  write_blob_response_variant "$object_id" "$raw" "$body" duplicate-key
  assert_blob_hydration_rejected "$directory" "$object_id" 'duplicate JSON key' 200 - "$body" 'response JSON is malformed or ambiguous'

  write_blob_response_variant "$object_id" "$raw" "$body" malformed-json
  assert_blob_hydration_rejected "$directory" "$object_id" 'truncated JSON response' 200 - "$body" 'response JSON is malformed or ambiguous'

  write_blob_response_variant "$object_id" "$raw" "$body" bad-base64
  assert_blob_hydration_rejected "$directory" "$object_id" 'invalid base64 response' 200 - "$body" 'response base64 is malformed'

  write_blob_response_variant "$object_id" "$raw" "$body" truncated
  assert_blob_hydration_rejected "$directory" "$object_id" 'truncated base64 payload' 200 - "$body" 'decoded blob size does not match the declaration'

  write_blob_response_variant "$object_id" "$raw" "$body" oversized-blob
  assert_blob_hydration_rejected "$directory" "$object_id" 'oversized decoded Git blob' 200 - "$body" 'response declares an invalid blob size'

  dd if=/dev/zero bs=65537 count=96 2>/dev/null | tr '\000' A > "$oversized"
  assert_blob_hydration_rejected "$directory" "$object_id" 'oversized encoded JSON response' 200 - "$oversized" 'response exceeds the encoded size limit'

  set_response_plan 403 - -
  run_checker "$directory" pre-commit
  assert_failed 'non-200 GitHub response fails closed'
  assert_contains 'non-200 GitHub response has a specific diagnostic' 'endpoint returned HTTP 403'
  assert_request_count 1
  assert_request_not_contains 'non-200 response stops before Rustup' 'https://static.rust-lang.org/'
  assert_no_temporary_snapshot

  for location in \
    'https://api.github.com/repos/tailrocks/terminal-components-baseline/git/blobs/other' \
    'https://mirror.example.invalid/redirect' \
    'http://api.github.com/repos/tailrocks/terminal-components-baseline/git/blobs/other'; do
    set_response_plan 302 "$location" -
    run_checker "$directory" pre-commit
    assert_failed "redirect to $location fails closed"
    assert_contains "redirect to $location is prohibited" 'redirects are not permitted'
    assert_request_count 1
    assert_request_not_contains "redirect to $location is never followed" "$location"
    assert_no_temporary_snapshot
  done

  location="https://api.github.com/$(printf '%032768d' 0)"
  set_response_plan 200 "$location" -
  run_checker "$directory" pre-commit
  assert_failed 'oversized response headers fail closed'
  assert_contains 'oversized response headers are bounded' 'response headers exceed the size limit'
  assert_request_count 1
  assert_no_temporary_snapshot

  : > "$body"
  set_response_plan SLOW - -
  run_checker "$directory" pre-commit
  assert_failed 'stalled GitHub endpoint fails closed'
  assert_contains 'stalled GitHub endpoint has a deadline diagnostic' 'request exceeded the time limit'
  assert_request_count 1
  assert_no_temporary_snapshot

  assert_equal 'all rejected hydration responses preserve source objects' "$object_inventory" "$(source_object_inventory "$directory")"
}

test_trusted_seed_excluded_baseline_blob_is_not_hydrated() {
  local path=apps/jackin/tests/functional/baselines/functional/jackin_account_form_pending.tui.json
  local object_id
  local directory
  local endpoint
  object_id=$(seed_blob_oid "$path")
  endpoint="https://api.github.com/repos/tailrocks/terminal-components-baseline/git/blobs/$object_id"
  directory=$(new_repo_with_missing_seed_object missing-trusted-excluded-blob blob "$path")
  set_denied_response
  run_checker "$directory" pre-commit
  assert_failed 'missing excluded baseline blob does not prevent trusted seed preparation'
  assert_contains 'projection reaches the deterministic network boundary' 'rustup bootstrap response has an unexpected status'
  assert_request_count 1
  assert_request_not_contains 'projection never requests the excluded baseline blob' "$endpoint"
  assert_equal 'no included seed blob was requested' 'https://static.rust-lang.org/rustup/dist/' "$(sed -n '1p' "$request_log" | sed 's|\(https://static.rust-lang.org/rustup/dist/\).*|\1|')"
  assert_no_temporary_snapshot
}

test_trusted_seed_ignores_replacement_and_shallow_state() {
  local directory
  local base_commit
  local altered_commit
  local index_file
  local index_before
  local config_file
  local config_before
  directory=$(new_repo replacement-and-shallow)
  base_commit=$(git -C "$directory" rev-parse HEAD)
  printf '\n[[package]]\nname = "untrusted-replacement"\nversion = "0.0.0"\n' >> "$directory/Cargo.lock"
  git -C "$directory" add -- Cargo.lock
  GIT_AUTHOR_NAME='Baseline Checker Test' GIT_AUTHOR_EMAIL=baseline-checker@example.invalid GIT_COMMITTER_NAME='Baseline Checker Test' GIT_COMMITTER_EMAIL=baseline-checker@example.invalid git -C "$directory" commit --quiet -m altered-trusted-seed-replacement
  altered_commit=$(git -C "$directory" rev-parse HEAD)
  git -C "$directory" replace --force "$trusted_seed_commit" "$altered_commit"
  git -C "$directory" reset --hard --quiet "$base_commit"
  printf '%s\n' "$trusted_seed_commit" > "$(git -C "$directory" rev-parse --path-format=absolute --git-common-dir)/shallow"
  mkdir -p "$(git -C "$directory" rev-parse --path-format=absolute --git-common-dir)/info"
  printf '%s %s\n' "$trusted_seed_commit" "$altered_commit" > "$(git -C "$directory" rev-parse --path-format=absolute --git-common-dir)/info/grafts"
  configure_promisor_canaries "$directory"
  index_file=$(git -C "$directory" rev-parse --path-format=absolute --git-path index)
  config_file=$(git -C "$directory" rev-parse --path-format=absolute --git-common-dir)/config
  index_before=$(file_sha256 "$index_file")
  config_before=$(file_sha256 "$config_file")
  set_denied_response
  run_checker "$directory" pre-commit
  assert_failed 'replacement refs and shallow boundaries cannot alter the trusted seed'
  assert_contains 'fixed seed ignores replacement tree and reaches the external boundary' 'rustup bootstrap response has an unexpected status'
  assert_request_count 1
  assert_equal 'replacement and shallow input preserves the source index' "$index_before" "$(file_sha256 "$index_file")"
  assert_equal 'replacement and shallow input preserves local Git config' "$config_before" "$(file_sha256 "$config_file")"
  assert_equal 'replacement and shallow input does not invoke a credential helper' false "$(test -e "$test_root/promisor credential helper" && printf true || printf false)"
  assert_no_temporary_snapshot
}

test_trusted_seed_rejects_object_alternates() {
  local directory
  local object_dir
  local external_object_dir
  directory=$(new_repo trusted-seed-alternate)
  object_dir=$(git -C "$directory" rev-parse --path-format=absolute --git-path objects)
  external_object_dir=$(git -C "$repo_root" rev-parse --path-format=absolute --git-path objects)
  [[ -d "$external_object_dir" ]]
  mkdir -p "$object_dir/info"
  printf '%s\n' "$external_object_dir" > "$object_dir/info/alternates"
  set_denied_response
  run_checker "$directory" pre-commit
  assert_failed 'trusted seed rejects an external object alternate'
  assert_contains 'external object alternate has a specific diagnostic' 'source object store uses an unsupported alternate object source'
  assert_request_count 0
  assert_no_temporary_snapshot
}

test_snapshot_selection() {
  local directory
  local base_commit
  local pushed_commit
  local record
  directory=$(new_repo snapshot-selection)
  base_commit=$(git -C "$directory" rev-parse HEAD)
  set_velnor_pin "$directory" 0.1.73
  git -C "$directory" add -- mise.toml
  git -C "$directory" show "$base_commit:mise.toml" > "$directory/mise.toml"
  cmp -s "$directory/mise.toml" <(git -C "$directory" show "$base_commit:mise.toml")
  set_denied_response
  run_checker "$directory" pre-commit
  assert_failed 'pre-commit selects the staged tool pin with a clean worktree'
  assert_contains 'unsupported staged pin fails its reviewed support policy' 'snapshot tool tuple is outside the reviewed support policy'
  assert_request_count 0

  reset_repo "$directory"
  set_velnor_pin "$directory" 0.1.73
  git -C "$directory" add -- mise.toml
  GIT_AUTHOR_NAME='Baseline Checker Test' GIT_AUTHOR_EMAIL=baseline-checker@example.invalid GIT_COMMITTER_NAME='Baseline Checker Test' GIT_COMMITTER_EMAIL=baseline-checker@example.invalid git -C "$directory" commit --quiet -m unsupported-pushed-pin
  pushed_commit=$(git -C "$directory" rev-parse HEAD)
  git -C "$directory" update-ref refs/test/unsupported-pin "$pushed_commit"
  git -C "$directory" reset --hard --quiet "$base_commit"
  record=$(push_record refs/heads/unsupported-pin "$pushed_commit" refs/heads/unsupported-pin "$zero_oid")
  set_denied_response
  run_checker "$directory" pre-push "$record"
  assert_failed 'pre-push selects the pushed tool pin with a clean checkout'
  assert_contains 'unsupported pushed pin fails its reviewed support policy' 'snapshot tool tuple is outside the reviewed support policy'
  assert_request_count 0
}

test_push_record_selection() {
  local directory
  local base_commit
  local unsupported_commit
  local annotated_tag
  local record
  directory=$(new_repo push-record-selection)
  base_commit=$(git -C "$directory" rev-parse HEAD)
  set_velnor_pin "$directory" 0.1.73
  git -C "$directory" add -- mise.toml
  GIT_AUTHOR_NAME='Baseline Checker Test' GIT_AUTHOR_EMAIL=baseline-checker@example.invalid GIT_COMMITTER_NAME='Baseline Checker Test' GIT_COMMITTER_EMAIL=baseline-checker@example.invalid git -C "$directory" commit --quiet -m unsupported-pushed-pin
  unsupported_commit=$(git -C "$directory" rev-parse HEAD)
  git -C "$directory" reset --hard --quiet "$base_commit"
  blob_oid=$(printf 'tag payload\n' | git -C "$directory" hash-object -w --stdin)
  git -C "$directory" tag -a unsupported-commit-tag "$unsupported_commit" -m unsupported-commit-tag
  annotated_tag=$(git -C "$directory" rev-parse refs/tags/unsupported-commit-tag)
  set_denied_response
  record=$(push_record refs/notes/commits "$unsupported_commit" refs/notes/commits "$zero_oid")
  run_checker "$directory" pre-push "$record"
  assert_equal 'notes ref is skipped without checking a snapshot' 0 "$last_status"
  assert_request_count 0

  record=$(push_record refs/heads/main "$zero_oid" refs/heads/main "$(git -C "$directory" rev-parse HEAD)")
  run_checker "$directory" pre-push "$record"
  assert_equal 'deletion record is skipped without checking a snapshot' 0 "$last_status"
  assert_request_count 0

  record=$(push_record refs/tags/blob-tag "$blob_oid" refs/tags/blob-tag "$zero_oid")
  run_checker "$directory" pre-push "$record"
  assert_equal 'blob-valued tag is skipped without checking a snapshot' 0 "$last_status"
  assert_request_count 0

  record=$(printf '%s\n%s\n' \
    "$(push_record refs/notes/commits "$unsupported_commit" refs/notes/commits "$zero_oid")" \
    "$(push_record refs/heads/unsupported "$unsupported_commit" refs/heads/unsupported "$zero_oid")")
  run_checker "$directory" pre-push "$record"
  assert_failed 'mixed records still check a later pushed branch'
  assert_contains 'mixed records select the unsupported branch snapshot' 'snapshot tool tuple is outside the reviewed support policy'
  assert_request_count 0

  record=$(push_record refs/tags/unsupported-commit-tag "$annotated_tag" refs/tags/unsupported-commit-tag "$zero_oid")
  run_checker "$directory" pre-push "$record"
  assert_failed 'annotated commit tag snapshot is checked'
  assert_contains 'annotated commit tag selects its unsupported commit' 'snapshot tool tuple is outside the reviewed support policy'
  assert_request_count 0

  record=$(push_record refs/heads/main "$base_commit" refs/heads/main "$zero_oid")
  run_checker "$directory" pre-push "$record"
  assert_failed 'supported pushed branch reaches the deterministic network boundary'
  assert_contains 'supported pushed branch uses the approved boundary' 'rustup bootstrap response has an unexpected status'
  assert_request_count 1
  assert_no_temporary_snapshot
}

test_pushed_snapshot_ignores_replacement_refs() {
  local directory
  local original
  local replacement
  local record
  directory=$(new_repo pushed-replacement)
  replacement=$(git -C "$directory" rev-parse HEAD)
  set_velnor_pin "$directory" 0.1.73
  git -C "$directory" add -- mise.toml
  GIT_AUTHOR_NAME='Baseline Checker Test' GIT_AUTHOR_EMAIL=baseline-checker@example.invalid GIT_COMMITTER_NAME='Baseline Checker Test' GIT_COMMITTER_EMAIL=baseline-checker@example.invalid git -C "$directory" commit --quiet -m unsupported-original-pushed-pin
  original=$(git -C "$directory" rev-parse HEAD)
  git -C "$directory" replace --force "$original" "$replacement"
  git -C "$directory" reset --hard --quiet "$replacement"
  set_denied_response
  record=$(push_record refs/heads/original "$original" refs/heads/original "$zero_oid")
  run_checker "$directory" pre-push "$record"
  assert_failed 'pushed original is selected with replacement refs disabled'
  assert_contains 'pushed original keeps its unsupported pin diagnostic' 'snapshot tool tuple is outside the reviewed support policy'
  assert_request_count 0

  record=$(push_record refs/heads/replacement "$replacement" refs/heads/replacement "$zero_oid")
  run_checker "$directory" pre-push "$record"
  assert_failed 'supported replacement commit alone reaches the network boundary'
  assert_contains 'supported replacement commit has the normal boundary diagnostic' 'rustup bootstrap response has an unexpected status'
  assert_request_count 1

  assert_no_temporary_snapshot
}

test_materialization_preserves_raw_git_blob_bytes() {
  local directory
  local raw="$test_root/raw git blob"
  local observed="$test_root/materialized raw blob"
  local canary="$test_root/smudge filter canary"
  local status="$test_root/smudge filter status"
  local object_id
  local index_file
  local config_file
  local index_before
  local config_before
  local objects_before
  local checkout_before
  directory=$(new_repo raw-blob-materialization)
  git -C "$directory" config --local core.autocrlf true
  git -C "$directory" config --local filter.baseline-smudge.smudge "!touch '$canary'"
  printf 'docs/raw.txt filter=baseline-smudge\n' > "$directory/.gitattributes"
  git -C "$directory" add -- .gitattributes
  printf 'first line\nsecond line\n' > "$raw"
  cp -p "$raw" "$directory/docs/raw.txt"
  object_id=$(git -C "$directory" hash-object -w --stdin < "$raw")
  git -C "$directory" update-index --add --cacheinfo "100644,$object_id,docs/raw.txt"
  index_file=$(git -C "$directory" rev-parse --path-format=absolute --git-path index)
  config_file=$(git -C "$directory" rev-parse --path-format=absolute --git-common-dir)/config
  index_before=$(file_sha256 "$index_file")
  config_before=$(file_sha256 "$config_file")
  objects_before=$(source_object_inventory "$directory")
  checkout_before=$(file_sha256 "$directory/docs/raw.txt")
  set_denied_response
  set_materialization_observer index docs/raw.txt "$observed" "$canary" "$status"
  run_checker "$directory" pre-commit
  assert_failed 'materialized raw blob case reaches the deterministic network boundary'
  assert_contains 'raw blob observer reaches the external boundary' 'rustup bootstrap response has an unexpected status'
  assert_request_count 1
  if cmp -s "$raw" "$observed"; then
    assert_equal 'snapshot materialization preserves exact Git blob bytes including LF' true true
  else
    assert_equal 'snapshot materialization preserves exact Git blob bytes including LF' true false
  fi
  assert_equal 'smudge filter was not invoked while materializing' absent "$(cat "$status")"
  assert_equal 'smudge filter canary remains absent' false "$(test -e "$canary" && printf true || printf false)"
  assert_equal 'raw blob case preserves source index' "$index_before" "$(file_sha256 "$index_file")"
  assert_equal 'raw blob case preserves source Git config' "$config_before" "$(file_sha256 "$config_file")"
  assert_equal 'raw blob case preserves source Git objects' "$objects_before" "$(source_object_inventory "$directory")"
  assert_equal 'raw blob case preserves source checkout' "$checkout_before" "$(file_sha256 "$directory/docs/raw.txt")"
  assert_no_temporary_snapshot
}

prepare_missing_pushed_object() {
  local name=$1
  local kind=$2
  local base_commit
  local tree_oid
  local blob_oid
  local target_oid
  local objects_file="$test_root/$name all objects"
  local available_file="$test_root/$name available objects"
  local pack="$test_root/$name object pack"
  local object_dir
  local object_path
  local marker="$test_root/promisor credential helper"
  missing_push_repo=$(new_repo "$name")
  base_commit=$(git -C "$missing_push_repo" rev-parse HEAD)
  printf 'unique %s pushed snapshot blob for partial object test\n' "$name" > "$missing_push_repo/docs/missing-pushed-object.txt"
  git -C "$missing_push_repo" add -- docs/missing-pushed-object.txt
  GIT_AUTHOR_NAME='Baseline Checker Test' GIT_AUTHOR_EMAIL=baseline-checker@example.invalid GIT_COMMITTER_NAME='Baseline Checker Test' GIT_COMMITTER_EMAIL=baseline-checker@example.invalid git -C "$missing_push_repo" commit --quiet -m missing-pushed-object-fixture
  missing_push_commit=$(git -C "$missing_push_repo" rev-parse HEAD)
  tree_oid=$(git -C "$missing_push_repo" rev-parse "$missing_push_commit^{tree}")
  blob_oid=$(git -C "$missing_push_repo" rev-parse "$missing_push_commit:docs/missing-pushed-object.txt")
  missing_push_blob_oid=$blob_oid
  case "$kind" in
    commit) target_oid=$missing_push_commit ;;
    tree) target_oid=$tree_oid ;;
    blob) target_oid=$blob_oid ;;
    *) printf 'unknown missing pushed object kind %s\n' "$kind" >&2; return 2 ;;
  esac
  git -C "$missing_push_repo" reset --hard --quiet "$base_commit"
  if [[ "$kind" != commit ]]; then
    git -C "$missing_push_repo" rev-list --objects --no-object-names "$missing_push_commit" > "$objects_file"
    python3 - "$objects_file" "$available_file" "$target_oid" <<'PY'
import sys
from pathlib import Path

source, destination, missing = sys.argv[1:]
objects = dict.fromkeys(Path(source).read_text().splitlines())
if missing not in objects:
    raise SystemExit("pushed snapshot fixture does not contain its selected object")
objects.pop(missing)
Path(destination).write_text("".join(oid + "\n" for oid in objects))
PY
    git -C "$missing_push_repo" pack-objects --stdout < "$available_file" > "$pack"
    git -C "$missing_push_repo" index-pack --stdin < "$pack" >/dev/null
  fi
  object_dir=$(git -C "$missing_push_repo" rev-parse --path-format=absolute --git-path objects)
  object_path="$object_dir/${target_oid:0:2}/${target_oid:2}"
  rm -f "$object_path"
  if git -C "$missing_push_repo" cat-file -e "$target_oid" 2>/dev/null; then
    printf 'FAIL %s fixture still contains its selected pushed object\n' "$kind" >&2
    return 1
  fi
  rm -f "$marker"
  configure_promisor_canaries "$missing_push_repo"
}

test_pushed_missing_objects() {
  local kind
  local index_file
  local config_file
  local index_before
  local config_before
  local objects_before
  local checkout_before
  local refs_before
  local record
  local marker="$test_root/promisor credential helper"
  local missing_push_blob_oid
  for kind in commit tree blob; do
    prepare_missing_pushed_object "missing-pushed-$kind" "$kind"
    index_file=$(git -C "$missing_push_repo" rev-parse --path-format=absolute --git-path index)
    config_file=$(git -C "$missing_push_repo" rev-parse --path-format=absolute --git-common-dir)/config
    index_before=$(file_sha256 "$index_file")
    config_before=$(file_sha256 "$config_file")
    objects_before=$(source_object_inventory "$missing_push_repo")
    checkout_before=$(git -C "$missing_push_repo" status --porcelain=v1 --untracked-files=all | shasum -a 256 | awk '{print $1}')
    refs_before=$(source_ref_inventory "$missing_push_repo")
    record=$(push_record refs/heads/missing-pushed "$missing_push_commit" refs/heads/missing-pushed "$zero_oid")
    set_denied_response
    run_checker "$missing_push_repo" pre-push "$record" "GIT_EXEC_PATH=$fixture_bin"
    assert_failed "missing pushed $kind fails closed"
    case "$kind" in
      commit) assert_contains 'missing pushed commit has a stable diagnostic' 'cannot resolve pushed commit' ;;
      tree) assert_contains 'missing pushed tree has a stable diagnostic' 'cannot resolve pushed tree' ;;
      blob)
        assert_contains 'missing pushed blob stops at the denied official hydration response' 'selected snapshot blob hydration endpoint returned HTTP 502'
        assert_equal 'missing pushed blob requests its fixed official endpoint' "https://api.github.com/repos/tailrocks/terminal-components-baseline/git/blobs/$missing_push_blob_oid" "$(sed -n '1p' "$request_log")"
        ;;
    esac
    if [[ "$kind" == blob ]]; then
      assert_request_count 1
    else
      assert_request_count 0
    fi
    assert_equal "missing pushed $kind does not invoke the Git transport or credential helper" false "$(test -e "$marker" && printf true || printf false)"
    assert_equal "missing pushed $kind preserves source objects" "$objects_before" "$(source_object_inventory "$missing_push_repo")"
    assert_equal "missing pushed $kind preserves the source index" "$index_before" "$(file_sha256 "$index_file")"
    assert_equal "missing pushed $kind preserves local Git config" "$config_before" "$(file_sha256 "$config_file")"
    assert_equal "missing pushed $kind preserves source checkout state" "$checkout_before" "$(git -C "$missing_push_repo" status --porcelain=v1 --untracked-files=all | shasum -a 256 | awk '{print $1}')"
    assert_equal "missing pushed $kind preserves source refs" "$refs_before" "$(source_ref_inventory "$missing_push_repo")"
    assert_no_temporary_snapshot
  done
}

test_staged_missing_blob() {
  local directory
  local blob_oid
  local object_dir
  local object_path
  local index_file
  local config_file
  local index_before
  local config_before
  local objects_before
  local checkout_before
  local refs_before
  local raw="$test_root/staged missing blob.raw"
  local body="$test_root/staged missing blob.json"
  local observed="$test_root/staged hydrated blob"
  local canary="$test_root/staged hydration smudge canary"
  local status="$test_root/staged hydration smudge status"
  local endpoint
  local variant
  local marker="$test_root/promisor credential helper"
  directory=$(new_repo staged-missing-blob)
  printf 'unique staged blob missing from the source object store\r\nsecond exact line\n' > "$raw"
  cp -p "$raw" "$directory/docs/missing-staged-blob.txt"
  blob_oid=$(git -C "$directory" hash-object -w --stdin < "$raw")
  git -C "$directory" update-index --add --cacheinfo "100644,$blob_oid,docs/missing-staged-blob.txt"
  object_dir=$(git -C "$directory" rev-parse --path-format=absolute --git-path objects)
  object_path="$object_dir/${blob_oid:0:2}/${blob_oid:2}"
  rm -f "$object_path"
  if git -C "$directory" cat-file -e "$blob_oid" 2>/dev/null; then
    printf 'FAIL staged fixture still contains its missing blob object\n' >&2
    return 1
  fi
  rm -f "$marker"
  configure_promisor_canaries "$directory"
  index_file=$(git -C "$directory" rev-parse --path-format=absolute --git-path index)
  config_file=$(git -C "$directory" rev-parse --path-format=absolute --git-common-dir)/config
  index_before=$(file_sha256 "$index_file")
  config_before=$(file_sha256 "$config_file")
  objects_before=$(source_object_inventory "$directory")
  checkout_before=$(source_checkout_inventory "$directory")
  refs_before=$(source_ref_inventory "$directory")
  endpoint="https://api.github.com/repos/tailrocks/terminal-components-baseline/git/blobs/$blob_oid"
  write_selected_blob_response "$blob_oid" "$raw" "$body"
  set_response_plan 200 - "$body" 502 - -
  set_materialization_observer index docs/missing-staged-blob.txt "$observed" "$canary" "$status"
  run_checker "$directory" pre-commit '' "GIT_EXEC_PATH=$fixture_bin"
  assert_failed 'hydrated staged blob reaches the deterministic generator boundary'
  assert_contains 'staged hydration proceeds beyond the unavailable object' 'rustup bootstrap response has an unexpected status'
  assert_request_count 2
  assert_equal 'staged hydration requests the fixed official blob endpoint' "$endpoint" "$(sed -n '1p' "$request_log")"
  assert_equal 'staged hydration then reaches the fixed Rustup endpoint' "https://static.rust-lang.org$checksum_path" "$(sed -n '2p' "$request_log")"
  if cmp -s "$raw" "$observed"; then
    assert_equal 'staged snapshot preserves exact hydrated Git blob bytes' true true
  else
    assert_equal 'staged snapshot preserves exact hydrated Git blob bytes' true false
  fi
  assert_equal 'staged blob hydration does not invoke source filters' absent "$(cat "$status")"
  assert_equal 'staged blob hydration does not execute the source credential helper' false "$(test -e "$marker" && printf true || printf false)"
  assert_equal 'staged blob hydration preserves source objects' "$objects_before" "$(source_object_inventory "$directory")"
  assert_equal 'staged blob hydration preserves source index' "$index_before" "$(file_sha256 "$index_file")"
  assert_equal 'staged blob hydration preserves local Git config' "$config_before" "$(file_sha256 "$config_file")"
  assert_equal 'staged blob hydration preserves source checkout' "$checkout_before" "$(source_checkout_inventory "$directory")"
  assert_equal 'staged blob hydration preserves source refs' "$refs_before" "$(source_ref_inventory "$directory")"

  for variant in wrong-content wrong-sha bad-base64 oversized-blob; do
    write_selected_blob_response "$blob_oid" "$raw" "$body" "$variant"
    set_response_plan 200 - "$body"
    run_checker "$directory" pre-commit '' "GIT_EXEC_PATH=$fixture_bin"
    assert_failed "staged blob response variant $variant fails closed"
    case "$variant" in
      wrong-content) assert_contains 'wrong staged content hash is rejected' 'decoded blob does not match its Git object identity' ;;
      wrong-sha) assert_contains 'wrong staged declared identity is rejected' 'response object identity does not match the requested blob' ;;
      bad-base64) assert_contains 'malformed staged base64 is rejected' 'response base64 is malformed' ;;
      oversized-blob) assert_contains 'oversized staged blob is rejected' 'response declares an invalid blob size' ;;
    esac
    assert_request_count 1
    assert_equal "staged $variant case does not reach Rustup" false "$(rg -F -q 'https://static.rust-lang.org/' "$request_log" && printf true || printf false)"
    assert_equal "staged $variant preserves source objects" "$objects_before" "$(source_object_inventory "$directory")"
    assert_equal "staged $variant preserves source index" "$index_before" "$(file_sha256 "$index_file")"
    assert_equal "staged $variant preserves local Git config" "$config_before" "$(file_sha256 "$config_file")"
    assert_equal "staged $variant preserves source checkout" "$checkout_before" "$(source_checkout_inventory "$directory")"
    assert_equal "staged $variant preserves source refs" "$refs_before" "$(source_ref_inventory "$directory")"
    assert_equal "staged $variant does not execute the source credential helper" false "$(test -e "$marker" && printf true || printf false)"
    assert_no_temporary_snapshot
  done

  set_denied_response
  run_checker "$directory" pre-commit '' "GIT_EXEC_PATH=$fixture_bin"
  assert_failed 'unavailable official staged blob fails closed'
  assert_contains 'unavailable staged blob reports the official API status' 'selected snapshot blob hydration endpoint returned HTTP 502'
  assert_request_count 1
  assert_equal 'unavailable staged blob requests the fixed endpoint' "$endpoint" "$(sed -n '1p' "$request_log")"
  assert_equal 'unavailable staged blob preserves source objects' "$objects_before" "$(source_object_inventory "$directory")"
  assert_equal 'unavailable staged blob preserves source index' "$index_before" "$(file_sha256 "$index_file")"
  assert_equal 'unavailable staged blob preserves local Git config' "$config_before" "$(file_sha256 "$config_file")"
  assert_equal 'unavailable staged blob preserves source checkout' "$checkout_before" "$(source_checkout_inventory "$directory")"
  assert_equal 'unavailable staged blob preserves source refs' "$refs_before" "$(source_ref_inventory "$directory")"
  assert_equal 'missing staged blob does not invoke the Git transport or credential helper' false "$(test -e "$marker" && printf true || printf false)"
  assert_no_temporary_snapshot
}

test_pushed_missing_blob_hydration() {
  local directory
  local base_commit
  local pushed_commit
  local raw="$test_root/pushed missing blob.raw"
  local body="$test_root/pushed missing blob.json"
  local observed="$test_root/pushed hydrated blob"
  local canary="$test_root/pushed hydration smudge canary"
  local status="$test_root/pushed hydration smudge status"
  local blob_oid
  local object_dir
  local object_path
  local record
  local index_file
  local config_file
  local index_before
  local config_before
  local objects_before
  local refs_before
  local checkout_before
  local endpoint
  local marker="$test_root/promisor credential helper"
  directory=$(new_repo pushed-missing-blob-hydration)
  base_commit=$(git -C "$directory" rev-parse HEAD)
  printf 'unique pushed blob hydrated from the approved official endpoint\r\nsecond exact line\n' > "$raw"
  cp -p "$raw" "$directory/docs/missing-pushed-blob.txt"
  blob_oid=$(git -C "$directory" hash-object -w --stdin < "$raw")
  git -C "$directory" update-index --add --cacheinfo "100644,$blob_oid,docs/missing-pushed-blob.txt"
  GIT_AUTHOR_NAME='Baseline Checker Test' GIT_AUTHOR_EMAIL=baseline-checker@example.invalid GIT_COMMITTER_NAME='Baseline Checker Test' GIT_COMMITTER_EMAIL=baseline-checker@example.invalid git -C "$directory" commit --quiet -m pushed-missing-blob-hydration
  pushed_commit=$(git -C "$directory" rev-parse HEAD)
  git -C "$directory" reset --hard --quiet "$base_commit"
  object_dir=$(git -C "$directory" rev-parse --path-format=absolute --git-path objects)
  object_path="$object_dir/${blob_oid:0:2}/${blob_oid:2}"
  rm -f "$object_path"
  if git -C "$directory" cat-file -e "$blob_oid" 2>/dev/null; then
    printf 'FAIL pushed hydration fixture still contains its missing blob\n' >&2
    return 1
  fi
  configure_promisor_canaries "$directory"
  index_file=$(git -C "$directory" rev-parse --path-format=absolute --git-path index)
  config_file=$(git -C "$directory" rev-parse --path-format=absolute --git-common-dir)/config
  index_before=$(file_sha256 "$index_file")
  config_before=$(file_sha256 "$config_file")
  objects_before=$(source_object_inventory "$directory")
  refs_before=$(source_ref_inventory "$directory")
  checkout_before=$(source_checkout_inventory "$directory")
  record=$(push_record refs/heads/missing-pushed "$pushed_commit" refs/heads/missing-pushed "$zero_oid")
  endpoint="https://api.github.com/repos/tailrocks/terminal-components-baseline/git/blobs/$blob_oid"
  write_selected_blob_response "$blob_oid" "$raw" "$body"
  set_response_plan 200 - "$body" 502 - -
  set_materialization_observer push-1 docs/missing-pushed-blob.txt "$observed" "$canary" "$status"
  run_checker "$directory" pre-push "$record" "GIT_EXEC_PATH=$fixture_bin"
  assert_failed 'hydrated pushed blob reaches the deterministic generator boundary'
  assert_contains 'pushed hydration proceeds beyond the unavailable object' 'rustup bootstrap response has an unexpected status'
  assert_request_count 2
  assert_equal 'pushed hydration requests the fixed official blob endpoint' "$endpoint" "$(sed -n '1p' "$request_log")"
  assert_equal 'pushed hydration then reaches the fixed Rustup endpoint' "https://static.rust-lang.org$checksum_path" "$(sed -n '2p' "$request_log")"
  if cmp -s "$raw" "$observed"; then
    assert_equal 'pushed snapshot preserves exact hydrated Git blob bytes' true true
  else
    assert_equal 'pushed snapshot preserves exact hydrated Git blob bytes' true false
  fi
  assert_equal 'pushed blob hydration does not invoke source filters' absent "$(cat "$status")"
  assert_equal 'pushed blob hydration does not execute the source credential helper' false "$(test -e "$marker" && printf true || printf false)"
  assert_equal 'pushed blob hydration preserves source objects' "$objects_before" "$(source_object_inventory "$directory")"
  assert_equal 'pushed blob hydration preserves source index' "$index_before" "$(file_sha256 "$index_file")"
  assert_equal 'pushed blob hydration preserves local Git config' "$config_before" "$(file_sha256 "$config_file")"
  assert_equal 'pushed blob hydration preserves source checkout' "$checkout_before" "$(source_checkout_inventory "$directory")"
  assert_equal 'pushed blob hydration preserves source refs' "$refs_before" "$(source_ref_inventory "$directory")"
  assert_no_temporary_snapshot
}

test_shared_blob_reuse_across_readers() {
  local object_path=Cargo.toml
  local object_id
  local directory
  local raw="$test_root/shared seed snapshot blob.raw"
  local body="$test_root/shared seed snapshot blob.json"
  local observed="$test_root/shared seed snapshot materialized blob"
  local canary="$test_root/shared seed snapshot smudge canary"
  local status="$test_root/shared seed snapshot smudge status"
  local index_file
  local config_file
  local index_before
  local config_before
  local objects_before
  local checkout_before
  local refs_before
  local endpoint
  local marker="$test_root/promisor credential helper"
  object_id=$(seed_blob_oid "$object_path")
  write_blob_response_variant "$object_id" "$raw" "$body" wrapped-base64
  directory=$(new_repo_with_missing_seed_object shared-selected-seed-blob blob "$object_path")
  cp -p "$raw" "$directory/docs/shared-seed-cargo.toml"
  git -C "$directory" update-index --add --cacheinfo "100644,$object_id,docs/shared-seed-cargo.toml"
  configure_promisor_canaries "$directory"
  index_file=$(git -C "$directory" rev-parse --path-format=absolute --git-path index)
  config_file=$(git -C "$directory" rev-parse --path-format=absolute --git-common-dir)/config
  index_before=$(file_sha256 "$index_file")
  config_before=$(file_sha256 "$config_file")
  objects_before=$(source_object_inventory "$directory")
  checkout_before=$(source_checkout_inventory "$directory")
  refs_before=$(source_ref_inventory "$directory")
  endpoint="https://api.github.com/repos/tailrocks/terminal-components-baseline/git/blobs/$object_id"
  set_response_plan 200 - "$body" 502 - - 200 - "$body"
  set_materialization_observer index docs/shared-seed-cargo.toml "$observed" "$canary" "$status"
  run_checker "$directory" pre-commit '' "GIT_EXEC_PATH=$fixture_bin"
  assert_failed 'shared selected and trusted blob reaches the deterministic generator boundary'
  assert_request_count 2
  assert_equal 'shared blob is fetched from the fixed official endpoint once' "$endpoint" "$(sed -n '1p' "$request_log")"
  assert_equal 'trusted seed reuses the selected blob instead of requesting it again' "https://static.rust-lang.org$checksum_path" "$(sed -n '2p' "$request_log")"
  assert_contains 'shared verified blob lets both readers reach Rustup' 'rustup bootstrap response has an unexpected status'
  if cmp -s "$raw" "$observed"; then
    assert_equal 'shared selected blob preserves exact Git bytes' true true
  else
    assert_equal 'shared selected blob preserves exact Git bytes' true false
  fi
  assert_equal 'shared blob hydration does not invoke source filters' absent "$(cat "$status")"
  assert_equal 'shared blob hydration does not execute the source credential helper' false "$(test -e "$marker" && printf true || printf false)"
  assert_equal 'shared blob hydration preserves source objects' "$objects_before" "$(source_object_inventory "$directory")"
  assert_equal 'shared blob hydration preserves source index' "$index_before" "$(file_sha256 "$index_file")"
  assert_equal 'shared blob hydration preserves local Git config' "$config_before" "$(file_sha256 "$config_file")"
  assert_equal 'shared blob hydration preserves source checkout' "$checkout_before" "$(source_checkout_inventory "$directory")"
  assert_equal 'shared blob hydration preserves source refs' "$refs_before" "$(source_ref_inventory "$directory")"
  assert_no_temporary_snapshot

  set_denied_response
  run_checker "$directory" pre-commit '' "GIT_EXEC_PATH=$fixture_bin"
  assert_failed 'unavailable shared blob response fails before generation'
  assert_contains 'shared blob denial has its hydration diagnostic' 'selected snapshot blob hydration endpoint returned HTTP 502'
  assert_request_count 1
  assert_equal 'shared blob denial requests the fixed endpoint' "$endpoint" "$(sed -n '1p' "$request_log")"
  assert_request_not_contains 'shared blob denial stops before Rustup' 'https://static.rust-lang.org/'
  assert_equal 'shared blob denial preserves source objects' "$objects_before" "$(source_object_inventory "$directory")"
  assert_equal 'shared blob denial preserves source index' "$index_before" "$(file_sha256 "$index_file")"
  assert_equal 'shared blob denial preserves local Git config' "$config_before" "$(file_sha256 "$config_file")"
  assert_equal 'shared blob denial preserves source checkout' "$checkout_before" "$(source_checkout_inventory "$directory")"
  assert_equal 'shared blob denial preserves source refs' "$refs_before" "$(source_ref_inventory "$directory")"
  assert_no_temporary_snapshot
}

test_distinct_blob_budget_across_readers() {
  local seed_path=Cargo.toml
  local seed_oid
  local selected_oid
  local directory
  local raw="$test_root/distinct selected blob.raw"
  local selected_body="$test_root/distinct selected blob.json"
  local seed_body="$test_root/distinct seed blob.json"
  local observed="$test_root/distinct selected materialized blob"
  local canary="$test_root/distinct selected smudge canary"
  local status="$test_root/distinct selected smudge status"
  local object_dir
  local object_path
  local index_file
  local config_file
  local index_before
  local config_before
  local objects_before
  local checkout_before
  local refs_before
  local seed_endpoint
  local selected_endpoint
  local marker="$test_root/promisor credential helper"
  seed_oid=$(seed_blob_oid "$seed_path")
  write_blob_response "$seed_oid" "$test_root/distinct seed blob.raw" "$seed_body"
  directory=$(new_repo_with_missing_seed_object distinct-selected-seed-blobs blob "$seed_path")
  printf 'a second distinct selected blob debits the invocation budget\r\n' > "$raw"
  cp -p "$raw" "$directory/docs/distinct-selected-blob.txt"
  selected_oid=$(git -C "$directory" hash-object -w --stdin < "$raw")
  git -C "$directory" update-index --add --cacheinfo "100644,$selected_oid,docs/distinct-selected-blob.txt"
  object_dir=$(git -C "$directory" rev-parse --path-format=absolute --git-path objects)
  object_path="$object_dir/${selected_oid:0:2}/${selected_oid:2}"
  rm -f "$object_path"
  if [[ "$selected_oid" == "$seed_oid" ]] || git -C "$directory" cat-file -e "$selected_oid" 2>/dev/null; then
    printf 'FAIL distinct selected blob fixture is not absent and distinct\n' >&2
    return 1
  fi
  write_selected_blob_response "$selected_oid" "$raw" "$selected_body"
  configure_promisor_canaries "$directory"
  index_file=$(git -C "$directory" rev-parse --path-format=absolute --git-path index)
  config_file=$(git -C "$directory" rev-parse --path-format=absolute --git-common-dir)/config
  index_before=$(file_sha256 "$index_file")
  config_before=$(file_sha256 "$config_file")
  objects_before=$(source_object_inventory "$directory")
  checkout_before=$(source_checkout_inventory "$directory")
  refs_before=$(source_ref_inventory "$directory")
  selected_endpoint="https://api.github.com/repos/tailrocks/terminal-components-baseline/git/blobs/$selected_oid"
  seed_endpoint="https://api.github.com/repos/tailrocks/terminal-components-baseline/git/blobs/$seed_oid"
  set_response_plan 200 - "$selected_body" 200 - "$seed_body" 502 - -
  set_materialization_observer index docs/distinct-selected-blob.txt "$observed" "$canary" "$status"
  run_checker "$directory" pre-commit '' "GIT_EXEC_PATH=$fixture_bin"
  assert_failed 'two distinct missing readers reach the deterministic generator boundary'
  assert_contains 'distinct missing objects consume separate hydration slots' 'rustup bootstrap response has an unexpected status'
  assert_request_count 3
  assert_equal 'selected blob uses its own fixed endpoint' "$selected_endpoint" "$(sed -n '1p' "$request_log")"
  assert_equal 'distinct trusted blob uses its own fixed endpoint' "$seed_endpoint" "$(sed -n '2p' "$request_log")"
  assert_equal 'two hydration requests then reach Rustup' "https://static.rust-lang.org$checksum_path" "$(sed -n '3p' "$request_log")"
  if cmp -s "$raw" "$observed"; then
    assert_equal 'distinct selected blob preserves exact Git bytes' true true
  else
    assert_equal 'distinct selected blob preserves exact Git bytes' true false
  fi
  assert_equal 'distinct blob hydration does not invoke source filters' absent "$(cat "$status")"
  assert_equal 'distinct blob hydration does not execute the source credential helper' false "$(test -e "$marker" && printf true || printf false)"
  assert_equal 'distinct blob hydration preserves source objects' "$objects_before" "$(source_object_inventory "$directory")"
  assert_equal 'distinct blob hydration preserves source index' "$index_before" "$(file_sha256 "$index_file")"
  assert_equal 'distinct blob hydration preserves local Git config' "$config_before" "$(file_sha256 "$config_file")"
  assert_equal 'distinct blob hydration preserves source checkout' "$checkout_before" "$(source_checkout_inventory "$directory")"
  assert_equal 'distinct blob hydration preserves source refs' "$refs_before" "$(source_ref_inventory "$directory")"
  assert_no_temporary_snapshot
}

test_pushed_raw_original_blob() {
  local directory
  local raw="$test_root/pushed raw original blob"
  local observed="$test_root/pushed materialized raw blob"
  local canary="$test_root/pushed smudge filter canary"
  local status="$test_root/pushed smudge filter status"
  local original_oid
  local replacement_oid
  local pushed_commit
  local record
  local index_file
  local config_file
  local index_before
  local config_before
  local objects_before
  local checkout_before
  directory=$(new_repo pushed-raw-blob)
  git -C "$directory" config --local core.autocrlf true
  git -C "$directory" config --local filter.baseline-smudge.smudge "!touch '$canary'"
  printf 'docs/push-raw.txt filter=baseline-smudge\n' > "$directory/.gitattributes"
  git -C "$directory" add -- .gitattributes
  printf 'first pushed line\r\nsecond pushed line\r\n' > "$raw"
  cp -p "$raw" "$directory/docs/push-raw.txt"
  original_oid=$(git -C "$directory" hash-object -w --stdin < "$raw")
  replacement_oid=$(printf 'replacement bytes must not be observed\n' | git -C "$directory" hash-object -w --stdin)
  git -C "$directory" update-index --add --cacheinfo "100644,$original_oid,docs/push-raw.txt"
  GIT_AUTHOR_NAME='Baseline Checker Test' GIT_AUTHOR_EMAIL=baseline-checker@example.invalid GIT_COMMITTER_NAME='Baseline Checker Test' GIT_COMMITTER_EMAIL=baseline-checker@example.invalid git -C "$directory" commit --quiet -m pushed-raw-blob
  pushed_commit=$(git -C "$directory" rev-parse HEAD)
  git -C "$directory" replace --force "$original_oid" "$replacement_oid"
  index_file=$(git -C "$directory" rev-parse --path-format=absolute --git-path index)
  config_file=$(git -C "$directory" rev-parse --path-format=absolute --git-common-dir)/config
  index_before=$(file_sha256 "$index_file")
  config_before=$(file_sha256 "$config_file")
  objects_before=$(source_object_inventory "$directory")
  checkout_before=$(file_sha256 "$directory/docs/push-raw.txt")
  record=$(push_record refs/heads/pushed-raw "$pushed_commit" refs/heads/pushed-raw "$zero_oid")
  set_denied_response
  set_materialization_observer push-1 docs/push-raw.txt "$observed" "$canary" "$status"
  run_checker "$directory" pre-push "$record"
  assert_failed 'complete pushed tree reaches the deterministic network boundary'
  assert_contains 'pushed raw object observer reaches the external boundary' 'rustup bootstrap response has an unexpected status'
  assert_request_count 1
  if cmp -s "$raw" "$observed"; then
    assert_equal 'pushed snapshot uses original raw Git blob bytes despite a replacement ref' true true
  else
    assert_equal 'pushed snapshot uses original raw Git blob bytes despite a replacement ref' true false
  fi
  assert_equal 'pushed snapshot materialization does not invoke smudge filters' absent "$(cat "$status")"
  assert_equal 'pushed snapshot leaves the smudge filter canary absent' false "$(test -e "$canary" && printf true || printf false)"
  assert_equal 'pushed raw case preserves source index' "$index_before" "$(file_sha256 "$index_file")"
  assert_equal 'pushed raw case preserves source Git config' "$config_before" "$(file_sha256 "$config_file")"
  assert_equal 'pushed raw case preserves source objects' "$objects_before" "$(source_object_inventory "$directory")"
  assert_equal 'pushed raw case preserves the source checkout' "$checkout_before" "$(file_sha256 "$directory/docs/push-raw.txt")"
  assert_no_temporary_snapshot
}

test_snapshot_entry_limit() {
  local directory
  local index_file
  local index_before
  local config_file
  local config_before
  local objects_before
  local checkout_before
  local blob_oid
  directory=$(new_repo snapshot-entry-limit)
  index_file=$(git -C "$directory" rev-parse --path-format=absolute --git-path index)
  blob_oid=$(git -C "$directory" rev-parse HEAD:docs/keep.md)
  python3 - "$blob_oid" <<'PY' | GIT_INDEX_FILE="$index_file" git -C "$directory" update-index --index-info
import sys

blob = sys.argv[1]
for number in range(8193):
    sys.stdout.write(f"100644 {blob}\tgenerated/entry-{number:05d}\n")
PY
  config_file=$(git -C "$directory" rev-parse --path-format=absolute --git-common-dir)/config
  index_before=$(file_sha256 "$index_file")
  config_before=$(file_sha256 "$config_file")
  objects_before=$(source_object_inventory "$directory")
  checkout_before=$(git -C "$directory" status --porcelain=v1 --untracked-files=all | shasum -a 256 | awk '{print $1}')
  set_denied_response
  run_checker "$directory" pre-commit
  assert_failed 'snapshot with 8,193 entries is rejected'
  assert_contains 'snapshot entry cap reports its stable diagnostic' 'snapshot tree exceeds the entry count limit'
  assert_request_count 0
  assert_equal 'entry-limit fixture preserves the source index' "$index_before" "$(file_sha256 "$index_file")"
  assert_equal 'entry-limit fixture preserves local Git config' "$config_before" "$(file_sha256 "$config_file")"
  assert_equal 'entry-limit fixture preserves source objects' "$objects_before" "$(source_object_inventory "$directory")"
  assert_equal 'entry-limit fixture preserves source checkout state' "$checkout_before" "$(git -C "$directory" status --porcelain=v1 --untracked-files=all | shasum -a 256 | awk '{print $1}')"
  assert_no_temporary_snapshot
}

test_snapshot_listing_limit() {
  local directory
  local index_file
  local index_before
  local config_file
  local config_before
  local objects_before
  local checkout_before
  local blob_oid
  directory=$(new_repo snapshot-listing-limit)
  index_file=$(git -C "$directory" rev-parse --path-format=absolute --git-path index)
  blob_oid=$(git -C "$directory" rev-parse HEAD:docs/keep.md)
  python3 - "$blob_oid" <<'PY' | GIT_INDEX_FILE="$index_file" git -C "$directory" update-index --index-info
import sys

blob = sys.argv[1]
components = "/".join(["a" * 240] * 4)
for number in range(8180):
    sys.stdout.write(f"100644 {blob}\t{components}/entry-{number:04d}-abcdefghijklmnop\n")
PY
  config_file=$(git -C "$directory" rev-parse --path-format=absolute --git-common-dir)/config
  index_before=$(file_sha256 "$index_file")
  config_before=$(file_sha256 "$config_file")
  objects_before=$(source_object_inventory "$directory")
  checkout_before=$(source_checkout_inventory "$directory")
  set_denied_response
  run_checker "$directory" pre-commit
  assert_failed 'snapshot with an 8 MiB-plus recursive listing is rejected'
  assert_contains 'snapshot listing cap reports its stable diagnostic' 'snapshot tree listing exceeds the size limit'
  assert_request_count 0
  assert_equal 'listing-limit fixture preserves the source index' "$index_before" "$(file_sha256 "$index_file")"
  assert_equal 'listing-limit fixture preserves local Git config' "$config_before" "$(file_sha256 "$config_file")"
  assert_equal 'listing-limit fixture preserves source objects' "$objects_before" "$(source_object_inventory "$directory")"
  assert_equal 'listing-limit fixture preserves source checkout state' "$checkout_before" "$(source_checkout_inventory "$directory")"
  assert_no_temporary_snapshot
}

test_snapshot_blob_limits() {
  local directory
  local index_file
  local index_before
  local config_file
  local config_before
  local objects_before
  local checkout_before
  local blob_oid
  local raw="$test_root/oversized snapshot blob"
  directory=$(new_repo snapshot-blob-limit)
  index_file=$(git -C "$directory" rev-parse --path-format=absolute --git-path index)
  python3 -c 'import sys; sys.stdout.buffer.write(b"x" * (16 * 1024 * 1024 + 1))' > "$raw"
  blob_oid=$(git -C "$directory" hash-object -w --stdin < "$raw")
  git -C "$directory" update-index --add --cacheinfo "100644,$blob_oid,docs/oversized-blob"
  config_file=$(git -C "$directory" rev-parse --path-format=absolute --git-common-dir)/config
  index_before=$(file_sha256 "$index_file")
  config_before=$(file_sha256 "$config_file")
  objects_before=$(source_object_inventory "$directory")
  checkout_before=$(git -C "$directory" status --porcelain=v1 --untracked-files=all | shasum -a 256 | awk '{print $1}')
  set_denied_response
  run_checker "$directory" pre-commit
  assert_failed 'snapshot with a blob above 16 MiB is rejected'
  assert_contains 'per-blob cap reports its stable diagnostic' 'snapshot blob exceeds the per-object size limit'
  assert_request_count 0
  assert_equal 'per-blob fixture preserves the source index' "$index_before" "$(file_sha256 "$index_file")"
  assert_equal 'per-blob fixture preserves local Git config' "$config_before" "$(file_sha256 "$config_file")"
  assert_equal 'per-blob fixture preserves source objects' "$objects_before" "$(source_object_inventory "$directory")"
  assert_equal 'per-blob fixture preserves source checkout state' "$checkout_before" "$(git -C "$directory" status --porcelain=v1 --untracked-files=all | shasum -a 256 | awk '{print $1}')"
  assert_no_temporary_snapshot

  directory=$(new_repo snapshot-aggregate-limit)
  index_file=$(git -C "$directory" rev-parse --path-format=absolute --git-path index)
  python3 -c 'import sys; sys.stdout.buffer.write(b"y" * (16 * 1024 * 1024))' > "$raw"
  blob_oid=$(git -C "$directory" hash-object -w --stdin < "$raw")
  python3 - "$blob_oid" <<'PY' | GIT_INDEX_FILE="$index_file" git -C "$directory" update-index --index-info
import sys

blob = sys.argv[1]
for number in range(33):
    sys.stdout.write(f"100644 {blob}\taggregate/entry-{number:02d}\n")
PY
  config_file=$(git -C "$directory" rev-parse --path-format=absolute --git-common-dir)/config
  index_before=$(file_sha256 "$index_file")
  config_before=$(file_sha256 "$config_file")
  objects_before=$(source_object_inventory "$directory")
  checkout_before=$(git -C "$directory" status --porcelain=v1 --untracked-files=all | shasum -a 256 | awk '{print $1}')
  set_denied_response
  run_checker "$directory" pre-commit
  assert_failed '33 occurrences of a 16 MiB blob exceed the 512 MiB aggregate cap'
  assert_contains 'aggregate cap reports its stable diagnostic' 'snapshot exceeds the total materialized blob size limit'
  assert_request_count 0
  assert_equal 'aggregate fixture preserves the source index' "$index_before" "$(file_sha256 "$index_file")"
  assert_equal 'aggregate fixture preserves local Git config' "$config_before" "$(file_sha256 "$config_file")"
  assert_equal 'aggregate fixture preserves source objects' "$objects_before" "$(source_object_inventory "$directory")"
  assert_equal 'aggregate fixture preserves source checkout state' "$checkout_before" "$(git -C "$directory" status --porcelain=v1 --untracked-files=all | shasum -a 256 | awk '{print $1}')"
  assert_no_temporary_snapshot
}

test_hydrated_repeated_blob_occurrence_limit() {
  local directory
  local raw="$test_root/repeated hydrated blob.raw"
  local body="$test_root/repeated hydrated blob.json"
  local blob_oid
  local object_dir
  local object_path
  local index_file
  local config_file
  local index_before
  local config_before
  local objects_before
  local checkout_before
  local refs_before
  local marker="$test_root/promisor credential helper"
  local endpoint
  local number
  directory=$(new_repo hydrated-repeated-occurrences)
  dd if=/dev/zero bs=1048576 count=4 2>/dev/null > "$raw"
  blob_oid=$(git -C "$directory" hash-object -w --stdin < "$raw")
  for number in {1..129}; do
    printf '100644 %s\tdocs/repeated-hydrated-%03d.bin\n' "$blob_oid" "$number"
  done | git -C "$directory" update-index --index-info
  object_dir=$(git -C "$directory" rev-parse --path-format=absolute --git-path objects)
  object_path="$object_dir/${blob_oid:0:2}/${blob_oid:2}"
  rm -f "$object_path"
  if git -C "$directory" cat-file -e "$blob_oid" 2>/dev/null; then
    printf 'FAIL repeated-blob fixture still contains its missing blob\n' >&2
    return 1
  fi
  configure_promisor_canaries "$directory"
  index_file=$(git -C "$directory" rev-parse --path-format=absolute --git-path index)
  config_file=$(git -C "$directory" rev-parse --path-format=absolute --git-common-dir)/config
  index_before=$(file_sha256 "$index_file")
  config_before=$(file_sha256 "$config_file")
  objects_before=$(source_object_inventory "$directory")
  checkout_before=$(source_checkout_inventory "$directory")
  refs_before=$(source_ref_inventory "$directory")
  endpoint="https://api.github.com/repos/tailrocks/terminal-components-baseline/git/blobs/$blob_oid"
  write_selected_blob_response "$blob_oid" "$raw" "$body"
  set_response_plan 200 - "$body"
  run_checker "$directory" pre-commit '' "GIT_EXEC_PATH=$fixture_bin"
  assert_failed 'hydrated repeated occurrences exceed the materialization aggregate limit'
  assert_contains 'post-hydration preflight counts repeated blob occurrences' 'snapshot exceeds the total materialized blob size limit'
  assert_request_count 1
  assert_equal 'repeated entries hydrate their shared blob only once' "$endpoint" "$(sed -n '1p' "$request_log")"
  assert_request_not_contains 'aggregate rejection stops before Rustup' 'https://static.rust-lang.org/'
  assert_equal 'repeated-blob hydration preserves source objects' "$objects_before" "$(source_object_inventory "$directory")"
  assert_equal 'repeated-blob hydration preserves source index' "$index_before" "$(file_sha256 "$index_file")"
  assert_equal 'repeated-blob hydration preserves local Git config' "$config_before" "$(file_sha256 "$config_file")"
  assert_equal 'repeated-blob hydration preserves source checkout' "$checkout_before" "$(source_checkout_inventory "$directory")"
  assert_equal 'repeated-blob hydration preserves source refs' "$refs_before" "$(source_ref_inventory "$directory")"
  assert_equal 'repeated-blob hydration does not execute source helpers' false "$(test -e "$marker" && printf true || printf false)"
  assert_no_temporary_snapshot
}

test_snapshot_rejects_gitlinks() {
  local directory
  local index_file
  local index_before
  local config_file
  local config_before
  local objects_before
  local checkout_before
  local record
  local target_commit
  directory=$(new_repo snapshot-gitlink)
  index_file=$(git -C "$directory" rev-parse --path-format=absolute --git-path index)
  target_commit=$(git -C "$directory" rev-parse HEAD)
  git -C "$directory" update-index --add --cacheinfo "160000,$target_commit,docs/submodule"
  config_file=$(git -C "$directory" rev-parse --path-format=absolute --git-common-dir)/config
  index_before=$(file_sha256 "$index_file")
  config_before=$(file_sha256 "$config_file")
  objects_before=$(source_object_inventory "$directory")
  checkout_before=$(git -C "$directory" status --porcelain=v1 --untracked-files=all | shasum -a 256 | awk '{print $1}')
  set_denied_response
  run_checker "$directory" pre-commit
  assert_failed 'snapshot containing a Git link fails closed'
  assert_contains 'Git link rejection has a specific diagnostic' 'snapshot contains unsupported gitlink entries'
  assert_request_count 0
  assert_equal 'Git-link fixture preserves the source index' "$index_before" "$(file_sha256 "$index_file")"
  assert_equal 'Git-link fixture preserves local Git config' "$config_before" "$(file_sha256 "$config_file")"
  assert_equal 'Git-link fixture preserves source objects' "$objects_before" "$(source_object_inventory "$directory")"
  assert_equal 'Git-link fixture preserves source checkout state' "$checkout_before" "$(git -C "$directory" status --porcelain=v1 --untracked-files=all | shasum -a 256 | awk '{print $1}')"
  assert_no_temporary_snapshot

  reset_repo "$directory"
  record=$(push_record refs/heads/main "$(git -C "$directory" rev-parse HEAD)" refs/heads/main "$zero_oid")
  set_denied_response
  run_checker "$directory" pre-push "$record"
  assert_failed 'ordinary pushed snapshot still reaches the expected network boundary'
  assert_contains 'ordinary pushed snapshot has its normal boundary diagnostic' 'rustup bootstrap response has an unexpected status'
  assert_request_count 1
  assert_no_temporary_snapshot
}

test_relative_tmpdir_public_hooks() {
  local directory
  local mode
  local record
  local remaining
  local runtime_tmp_path
  local runtime_tmp_is_absolute
  directory=$(new_repo relative-tmpdir)
  mkdir -p "$directory/relative temp root"
  record=$(push_record refs/heads/main "$(git -C "$directory" rev-parse HEAD)" refs/heads/main "$zero_oid")
  for mode in pre-commit pre-push; do
    set_denied_response
    if [[ "$mode" == pre-push ]]; then
      run_checker "$directory" "$mode" "$record" 'TMPDIR=relative temp root'
    else
      run_checker "$directory" "$mode" '' 'TMPDIR=relative temp root'
    fi
    assert_failed "$mode with a relative existing TMPDIR reaches the expected external boundary"
    assert_contains "$mode with relative TMPDIR does not lose its temp paths" 'rustup bootstrap response has an unexpected status'
    assert_request_count 1
    runtime_tmp_path=$(cat "$runtime_tmp_trace")
    if [[ "$runtime_tmp_path" == /* ]]; then
      runtime_tmp_is_absolute=true
    else
      runtime_tmp_is_absolute=false
    fi
    assert_equal "$mode passes an absolute runtime TMPDIR to the external network boundary" true "$runtime_tmp_is_absolute"
    remaining=$(find "$directory/relative temp root" -mindepth 1 -maxdepth 1 -name 'jackin-generated-workflows.*' -print -quit)
    assert_equal "$mode cleans the checker temp root created under relative TMPDIR" '' "$remaining"
  done
}

test_unreviewed_snapshot_lock_source() {
  local directory
  local index_file
  local index_before
  directory=$(new_repo unreviewed-lock-source)
  python3 - "$directory/Cargo.lock" <<'PY'
import sys
from pathlib import Path

path = Path(sys.argv[1])
data = path.read_bytes()
old = b"registry+https://github.com/rust-lang/crates.io-index"
new = b"registry+https://registry.example.invalid/index"
if old not in data:
    raise SystemExit("fixture lock has no reviewed crates.io source")
path.write_bytes(data.replace(old, new))
PY
  git -C "$directory" add -- Cargo.lock
  index_file=$(git -C "$directory" rev-parse --path-format=absolute --git-path index)
  index_before=$(file_sha256 "$index_file")
  set_denied_response
  run_checker "$directory" pre-commit
  assert_failed 'snapshot lock with an unreviewed registry source fails closed'
  assert_contains 'snapshot lock source rejection is specific' 'Cargo.lock is outside the reviewed dependency support policy'
  assert_request_count 0
  assert_equal 'unreviewed snapshot lock source preserves the source index' "$index_before" "$(file_sha256 "$index_file")"
  assert_no_temporary_snapshot
}

test_unreviewed_snapshot_manifest_source() {
  local directory
  directory=$(new_repo unreviewed-manifest-source)
  cat >> "$directory/Cargo.toml" <<'EOF'

[workspace.dependencies]
unreviewed = { git = "https://example.invalid/unreviewed", branch = "main" }
EOF
  git -C "$directory" add -- Cargo.toml
  set_denied_response
  run_checker "$directory" pre-commit
  assert_failed 'snapshot with an unreviewed Cargo manifest source fails closed'
  assert_contains 'snapshot manifest source rejection is specific' 'Cargo manifests are outside the reviewed dependency support policy'
  assert_request_count 0
  assert_no_temporary_snapshot
}

run_cargo_config_case() {
  local directory=$1
  local label=$2
  local path=$3
  local config=$4
  reset_repo "$directory"
  mkdir -p "$directory/$(dirname "$path")"
  printf '%s\n' "$config" > "$directory/$path"
  git -C "$directory" add -- "$path"
  set_denied_response
  run_checker "$directory" pre-commit
  assert_failed "$label fails closed before fetching"
  assert_contains "$label reports Cargo config rejection" 'Cargo config contains unsupported settings'
  assert_request_count 0
  assert_not_contains "$label does not disclose the canary marker" 'checker-test-canary'
}

test_cargo_config_vectors() {
  local directory
  local canary="$test_root/cargo canary"
  directory=$(new_repo cargo-config-vectors)
  printf '#!/bin/sh\nprintf checker-test-canary > %q\n' "$test_root/canary output" > "$canary"
  chmod 755 "$canary"
  run_cargo_config_case "$directory" 'root build.rustc config' '.cargo/config' $'[build]\nrustc = "'"$canary"$'"'
  run_cargo_config_case "$directory" 'root rustc-wrapper config' '.cargo/config.toml' $'[build]\nrustc-wrapper = "'"$canary"$'"'
  run_cargo_config_case "$directory" 'nested rustc-workspace-wrapper config' 'crates/nested/.cargo/config' $'[build]\nrustc-workspace-wrapper = "'"$canary"$'"'
  run_cargo_config_case "$directory" 'nested target runner and linker config' 'crates/nested/.cargo/config.toml' $'[target.x86_64-unknown-linux-gnu]\nrunner = ["'"$canary"$'"]\nlinker = "'"$canary"$'"'
  run_cargo_config_case "$directory" 'environment injection config' '.cargo/config.toml' $'[env]\nRUSTC = "'"$canary"$'"\nPATH = "'"$test_root"$'"'
  run_cargo_config_case "$directory" 'registry replacement config' '.cargo/config' $'[source.crates-io]\nreplace-with = "untrusted"'
  run_cargo_config_case "$directory" 'compiler flags and output path config' 'crates/nested/.cargo/config.toml' $'[build]\nrustflags = ["--test"]\ntarget-dir = "'"$test_root"$'"'
  run_cargo_config_case "$directory" 'links override config' 'crates/nested/.cargo/config' $'[target.x86_64-unknown-linux-gnu.links.native]\nrustc-link-search = "'"$test_root"$'"'
  assert_equal 'Cargo execution canary is not created' 'false' "$(test -e "$test_root/canary output" && printf true || printf false)"
}

test_toolchain_and_symlink() {
  local directory
  local record
  directory=$(new_repo toolchain-symlink)
  printf '[toolchain]\nchannel = "1.98.0"\nprofile = "minimal"\ncomponents = ["clippy", "rustfmt"]\ntargets = ["aarch64-apple-darwin", "x86_64-apple-darwin", "aarch64-unknown-linux-gnu", "x86_64-unknown-linux-gnu"]\n' > "$directory/rust-toolchain.toml"
  git -C "$directory" add -- rust-toolchain.toml
  set_denied_response
  run_checker "$directory" pre-commit
  assert_failed 'mismatched Rust toolchain fails before fetch'
  assert_contains 'mismatched Rust toolchain reports its declaration error' 'snapshot Rust toolchain channel or profile is unsupported'
  assert_request_count 0

  reset_repo "$directory"
  printf '[toolchain]\nchannel = "1.99.1"\nprofile = "minimal"\ncomponents = ["clippy", "rustfmt"]\ntargets = ["aarch64-apple-darwin", "x86_64-apple-darwin", "aarch64-unknown-linux-gnu", "x86_64-unknown-linux-gnu"]\n' > "$directory/rust-toolchain"
  git -C "$directory" add -- rust-toolchain
  set_denied_response
  run_checker "$directory" pre-commit
  assert_failed 'legacy Rust toolchain declaration fails before fetch'
  assert_contains 'legacy Rust toolchain declaration is rejected' 'legacy rust-toolchain file is unsupported'
  assert_request_count 0

  reset_repo "$directory"
  cp "$directory/mise.toml" "$directory/mise-target.toml"
  rm "$directory/mise.toml"
  ln -s mise-target.toml "$directory/mise.toml"
  git -C "$directory" add -- mise.toml mise-target.toml
  set_denied_response
  run_checker "$directory" pre-commit
  assert_failed 'symlinked mise.toml fails closed before fetch'
  assert_contains 'symlinked mise.toml is rejected by the snapshot parser' 'missing regular snapshot config mise.toml'
  assert_request_count 0

  reset_repo "$directory"
  ln -s /etc/passwd "$directory/docs/escape"
  git -C "$directory" add -- docs/escape
  set_denied_response
  run_checker "$directory" pre-commit
  assert_failed 'escaping tracked symlink fails before fetch'
  assert_contains 'escaping tracked symlink reports containment rejection' 'symlink target leaves snapshot'
  assert_request_count 0

  reset_repo "$directory"
  ln -s keep.md "$directory/docs/internal"
  git -C "$directory" add -- docs/internal
  git -C "$directory" commit --quiet -m internal-symlink
  record=$(push_record refs/heads/internal-symlink "$(git -C "$directory" rev-parse HEAD)" refs/heads/internal-symlink "$zero_oid")
  set_denied_response
  run_checker "$directory" pre-push "$record"
  assert_failed 'supported internal symlink reaches the deterministic network boundary'
  assert_contains 'internal symlink survives snapshot materialization' 'rustup bootstrap response has an unexpected status'
  assert_request_count 1
  assert_no_temporary_snapshot
}

assert_redirect_rejected() {
  local directory=$1
  local label=$2
  local location=$3
  local expected=$4
  local record
  reset_repo "$directory"
  record=$(push_record refs/heads/main "$(git -C "$directory" rev-parse HEAD)" refs/heads/main "$zero_oid")
  set_response_plan 302 "$location" -
  run_checker "$directory" pre-push "$record"
  assert_failed "$label fails closed"
  assert_contains "$label reports redirect rejection" "$expected"
  assert_request_count 1
  if [[ "$location" != "https://static.rust-lang.org$checksum_path" ]]; then
    assert_request_not_contains "$label does not request the rejected destination" "$location"
  fi
}

test_redirect_policy() {
  local directory
  local record
  local body="$test_root/bad checksum"
  local official_url="https://static.rust-lang.org$checksum_path"
  local mixed_url="https://STATIC.RUST-LANG.ORG$checksum_path"
  local loc1="https://STATIC.RUST-LANG.ORG$checksum_path"
  local loc2="https://StAtIc.RuSt-LaNg.OrG$checksum_path"
  local loc3="https://sTaTiC.rUsT-lAnG.oRg$checksum_path"
  local loc4="https://stAtIc.rUst-LaNg.oRg$checksum_path"
  directory=$(new_repo redirect-policy)
  printf 'malformed manifest\n' > "$body"
  set_response_plan 302 "$mixed_url" - 200 - "$body"
  record=$(push_record refs/heads/main "$(git -C "$directory" rev-parse HEAD)" refs/heads/main "$zero_oid")
  run_checker "$directory" pre-push "$record"
  assert_failed 'same-host approved redirect reaches checksum parsing'
  assert_contains 'same-host approved redirect reaches the manifest validator' 'official Rustup checksum manifest is malformed'
  assert_request_count 2
  assert_equal 'first official request is the checksum URL' "$official_url" "$(sed -n '1p' "$request_log")"
  assert_equal 'redirect stays on the approved host and path' "$mixed_url" "$(sed -n '2p' "$request_log")"

  assert_redirect_rejected "$directory" 'off-host intermediate redirect' "https://mirror.example/rustup/dist/$host_target/rustup-init.sha256" 'rustup bootstrap URL is outside the approved official distribution path'
  assert_redirect_rejected "$directory" 'HTTP downgrade redirect' "http://static.rust-lang.org$checksum_path" 'rustup bootstrap URL is outside the approved official distribution path'
  assert_redirect_rejected "$directory" 'redirect with userinfo' "https://user@static.rust-lang.org$checksum_path" 'rustup bootstrap URL is outside the approved official distribution path'
  assert_redirect_rejected "$directory" 'redirect with nonstandard port' "https://static.rust-lang.org:444$checksum_path" 'rustup bootstrap URL is outside the approved official distribution path'
  assert_redirect_rejected "$directory" 'redirect to disallowed official path' "https://static.rust-lang.org/rustup/dist/$host_target/untrusted" 'rustup bootstrap URL is outside the approved official distribution path'
  assert_redirect_rejected "$directory" 'malformed redirect' 'https://[::1' 'rustup bootstrap redirect URL is malformed'
  assert_not_contains 'malformed redirect does not continue to the bootstrap executable' 'authenticated Rustup bootstrap failed'
  assert_not_contains 'malformed redirect produces no Python traceback' 'Traceback'
  assert_redirect_rejected "$directory" 'redirect loop' "$official_url" 'rustup bootstrap redirect loop detected'

  reset_repo "$directory"
  record=$(push_record refs/heads/main "$(git -C "$directory" rev-parse HEAD)" refs/heads/main "$zero_oid")
  set_response_plan 302 "$loc1" - 302 "$loc2" - 302 "$loc3" - 302 "$loc4" -
  run_checker "$directory" pre-push "$record"
  assert_failed 'overlong HTTPS redirect chain fails closed'
  assert_contains 'overlong redirect chain reaches its hop limit' 'rustup bootstrap exceeded the redirect limit'
  assert_request_count 4
}

test_response_bounds() {
  local directory
  local body="$test_root/oversized checksum"
  local manifest="$test_root/mismatched checksum"
  local binary="$test_root/untrusted bootstrap bytes"
  local record
  directory=$(new_repo response-bounds)
  dd if=/dev/zero bs=4097 count=1 2>/dev/null | tr '\000' A > "$body"
  set_response_plan 200 - "$body"
  run_checker "$directory" pre-commit
  assert_failed 'oversized checksum response fails closed'
  assert_contains 'oversized checksum response is bounded' 'rustup bootstrap response exceeds the size limit'
  assert_request_count 1

  reset_repo "$directory"
  set_response_plan 200 - -
  run_checker "$directory" pre-commit
  assert_failed 'empty checksum response fails closed'
  assert_contains 'empty checksum response is rejected' 'rustup bootstrap response is empty'
  assert_request_count 1

  reset_repo "$directory"
  printf '%064d  rustup-init\n' 0 > "$manifest"
  printf '#!/bin/sh\nprintf checker-test-canary > %q\n' "$test_root/bootstrap canary" > "$binary"
  set_response_plan 200 - "$manifest" 200 - "$binary"
  record=$(push_record refs/heads/main "$(git -C "$directory" rev-parse HEAD)" refs/heads/main "$zero_oid")
  run_checker "$directory" pre-push "$record"
  assert_failed 'mismatched bootstrap bytes fail closed'
  assert_contains 'bootstrap checksum mismatch is reported' 'rustup bootstrap checksum does not match the official manifest'
  assert_request_count 2
  assert_equal 'unverified bootstrap is not executed' 'false' "$(test -e "$test_root/bootstrap canary" && printf true || printf false)"
  assert_no_temporary_snapshot
}

test_curl_environment_isolation() {
  local directory
  directory=$(new_repo curl-environment)
  set_denied_response
  run_checker "$directory" pre-commit "" \
    HTTP_PROXY=http://127.0.0.1:9 \
    HTTPS_PROXY=http://127.0.0.1:9 \
    CURL_HOME="$test_root/curl config" \
    CURL_CA_BUNDLE="$test_root/attacker ca" \
    SSL_CERT_FILE="$test_root/attacker cert" \
    RUSTC="$test_root/attacker rustc" \
    CARGO_HOME="$test_root/attacker cargo"
  assert_failed 'hostile transport and compiler environment still reaches the fixed curl boundary'
  assert_contains 'hostile environment does not change the request result' 'rustup bootstrap response has an unexpected status'
  assert_request_count 1
  assert_equal 'transport child receives no proxy, custom CA or curl config variables' clean "$(cat "$environment_trace")"
  assert_not_contains 'checker output does not disclose hostile values' "$test_root/attacker"
}

run_case() {
  case "$1" in
    all)
      test_clean_boundary
      test_snapshot_selection
      test_push_record_selection
      test_pushed_snapshot_ignores_replacement_refs
      test_materialization_preserves_raw_git_blob_bytes
      test_pushed_raw_original_blob
      test_pushed_missing_objects
      test_staged_missing_blob
      test_pushed_missing_blob_hydration
      test_shared_blob_reuse_across_readers
      test_distinct_blob_budget_across_readers
      test_snapshot_entry_limit
      test_snapshot_listing_limit
      test_snapshot_blob_limits
      test_hydrated_repeated_blob_occurrence_limit
      test_snapshot_rejects_gitlinks
      test_relative_tmpdir_public_hooks
      test_trusted_seed_missing_commit
      test_trusted_seed_missing_objects
      test_trusted_seed_blob_hydration
      test_trusted_seed_empty_blob_hydration
      test_trusted_seed_blob_response_validation
      test_trusted_seed_excluded_baseline_blob_is_not_hydrated
      test_trusted_seed_ignores_replacement_and_shallow_state
      test_trusted_seed_rejects_object_alternates
      test_unreviewed_snapshot_lock_source
      test_unreviewed_snapshot_manifest_source
      test_cargo_config_vectors
      test_toolchain_and_symlink
      test_redirect_policy
      test_response_bounds
      test_curl_environment_isolation
      ;;
    boundary) test_clean_boundary ;;
    snapshots) test_snapshot_selection ;;
    push-records) test_push_record_selection ;;
    replacement) test_pushed_snapshot_ignores_replacement_refs ;;
    raw-blob) test_materialization_preserves_raw_git_blob_bytes; test_pushed_raw_original_blob ;;
    selected-hydration) test_staged_missing_blob; test_pushed_missing_blob_hydration; test_pushed_missing_objects; test_shared_blob_reuse_across_readers; test_distinct_blob_budget_across_readers ;;
    shared-blob-reuse) test_shared_blob_reuse_across_readers ;;
    distinct-blob-budget) test_distinct_blob_budget_across_readers ;;
    pushed-blob-hydration) test_pushed_missing_blob_hydration ;;
    pushed-missing) test_pushed_missing_objects ;;
    staged-missing) test_staged_missing_blob ;;
    snapshot-limits) test_snapshot_entry_limit; test_snapshot_listing_limit; test_snapshot_blob_limits ;;
    hydrated-occurrences) test_hydrated_repeated_blob_occurrence_limit ;;
    listing-limit) test_snapshot_listing_limit ;;
    gitlink) test_snapshot_rejects_gitlinks ;;
    relative-tmpdir) test_relative_tmpdir_public_hooks ;;
    trusted-seed) test_trusted_seed_missing_commit; test_trusted_seed_missing_objects; test_trusted_seed_blob_hydration; test_trusted_seed_empty_blob_hydration; test_trusted_seed_blob_response_validation; test_trusted_seed_excluded_baseline_blob_is_not_hydrated; test_trusted_seed_ignores_replacement_and_shallow_state; test_trusted_seed_rejects_object_alternates ;;
    hydration) test_trusted_seed_blob_hydration ;;
    empty-blob) test_trusted_seed_empty_blob_hydration ;;
    hydration-boundaries) test_trusted_seed_blob_response_validation ;;
    missing-metadata) test_trusted_seed_missing_commit; test_trusted_seed_missing_objects ;;
    projection) test_trusted_seed_excluded_baseline_blob_is_not_hydrated ;;
    lock-source) test_unreviewed_snapshot_lock_source ;;
    manifest-source) test_unreviewed_snapshot_manifest_source ;;
    cargo) test_cargo_config_vectors ;;
    toolchain) test_toolchain_and_symlink ;;
    redirects) test_redirect_policy ;;
    responses) test_response_bounds ;;
    environment) test_curl_environment_isolation ;;
    *)
      printf 'usage: %s [all|boundary|snapshots|push-records|replacement|raw-blob|selected-hydration|shared-blob-reuse|distinct-blob-budget|pushed-blob-hydration|hydrated-occurrences|pushed-missing|snapshot-limits|gitlink|relative-tmpdir|trusted-seed|hydration|empty-blob|hydration-boundaries|missing-metadata|projection|lock-source|manifest-source|cargo|toolchain|redirects|responses|environment]\n' "$0" >&2
      return 2
      ;;
  esac
}

run_case "${1:-all}"
