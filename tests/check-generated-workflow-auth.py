#!/usr/bin/env python3
# SPDX-FileCopyrightText: 2026 Alexey Zhokhov
# SPDX-License-Identifier: Apache-2.0
import hashlib
import difflib
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile

root = Path(__file__).resolve().parent.parent
artifacts = os.environ.get("BASELINE_GUARD_TEST_ARTIFACT_ROOT")
if not artifacts:
    raise SystemExit("set BASELINE_GUARD_TEST_ARTIFACT_ROOT to previously qualified artifacts")
cases = {"all", "setup", "hook-pre-commit", "hook-pre-push", "wrong-tree", "response-header", "response-body", "response-redirect", "response-identity"}
if not sys.argv[1:]:
    selected_case = "all"
elif sys.argv[1:] == ["--setup-only"]:
    selected_case = "setup"
elif len(sys.argv) == 3 and sys.argv[1] == "--case" and sys.argv[2] in cases:
    selected_case = sys.argv[2]
else:
    raise SystemExit("usage: check-generated-workflow-auth.py [--setup-only | --case CASE]")
def selected(case):
    return selected_case in ("all", case)
dummy = "GuardDummy" + "Credential_47291"
clean = {"PATH": "/usr/bin:/bin:/usr/sbin:/sbin", "HOME": "/tmp", "LANG": "C", "LC_ALL": "C", "GIT_NO_LAZY_FETCH": "1", "GIT_NO_REPLACE_OBJECTS": "1"}
evidence_base = os.environ.get("BASELINE_GUARD_AUTH_EVIDENCE_DIR")
evidence = None
if evidence_base:
    Path(evidence_base).mkdir(parents=True, exist_ok=True)
    evidence = Path(tempfile.mkdtemp(prefix="auth-fixture.", dir=evidence_base))
    print("evidence:", evidence, flush=True)

def run(args, **kwargs):
    return subprocess.run(args, check=True, env=clean, stdout=subprocess.PIPE, stderr=subprocess.PIPE, **kwargs)

with tempfile.TemporaryDirectory(prefix="guard-auth-test.") as temporary:
    base = Path(temporary)
    repo = base / "repository with spaces"
    fake = base / "curl-fixture"
    events = base / "events"
    scenario = base / "scenario"
    source = str(root)
    run(["/usr/bin/git", "clone", "--quiet", "--depth=1", "file://" + source, str(repo)])
    run(["/usr/bin/git", "-C", str(repo), "remote", "set-url", "origin", "https://attacker.invalid/repository"])
    (repo / "scripts").mkdir(exist_ok=True)
    for name in ("check-generated-workflows.sh", "install-generated-workflow-guard.sh", "generated-workflow-guard-config.toml", "python-inventory.sh", "python-mise.toml", "python-mise.macos-arm64.lock.toml", "python-mise.linux-x64.lock.toml", "trusted-seed.commit", "trusted-seed-metadata.pack", "trusted-seed-metadata.objects"):
        shutil.copy2(root / "scripts" / name, repo / "scripts" / name)
    fake.write_text('''#!/usr/bin/env python3
import base64, json, os, subprocess, sys
from pathlib import Path
token = os.environ.pop("GITHUB_COM_TOKEN", "")
args = sys.argv[1:]
source = ''' + repr(source) + '''
events = Path(''' + repr(str(events)) + ''')
scenario = Path(''' + repr(str(scenario)) + ''')
url = args[-1]
fixed = "https://api.github.com/repos/tailrocks/terminal-components-baseline/git/blobs/"
with events.open("a") as f:
    if token and any(token in arg for arg in args):
        f.write("credential-in-argv\\n")
    if token and (not url.startswith(fixed) or "--variable" not in args or "%GITHUB_COM_TOKEN" not in args or "--expand-header" not in args or "Authorization: Bearer {{GITHUB_COM_TOKEN}}" not in args or "--include" not in args):
        f.write("credential-scope-error\\n")
    f.write(("authenticated" if token else "anonymous") + " " + ("blob" if url.startswith(fixed) else "other") + "\\n")
    if url.startswith(fixed):
        f.write(("authenticated-oid " if token else "anonymous-oid ") + url[len(fixed):] + "\\n")
if not url.startswith(fixed):
    if "--dump-header" in args:
        Path(args[args.index("--dump-header") + 1]).write_bytes(b"HTTP/1.1 502 Fixture\\r\\n\\r\\n")
    sys.exit(0)
oid = url[len(fixed):]
if len(oid) != 40 or any(c not in "0123456789abcdef" for c in oid):
    sys.exit(1)
data = subprocess.run(["/usr/bin/git", "-C", source, "cat-file", "blob", oid], check=True, stdout=subprocess.PIPE, env={"PATH": "/usr/bin:/bin", "HOME": "/tmp", "GIT_NO_LAZY_FETCH": "1", "GIT_NO_REPLACE_OBJECTS": "1", "GIT_CONFIG_NOSYSTEM": "1", "GIT_CONFIG_GLOBAL": "/dev/null"}).stdout
body = json.dumps({"sha": oid, "size": len(data), "encoding": "base64", "content": base64.b64encode(data).decode()}).encode()
headers = b"HTTP/2 200\\r\\ndate: Fri, 06 Nov 2026 00:00:00 GMT\\r\\ncontent-type: application/json\\r\\n\\r\\n"
if token:
    mode = scenario.read_text() if scenario.exists() else ""
    if mode == "header":
        headers = b"HTTP/2 200\\r\\nX-Reflected: " + token.encode() + b"\\r\\n\\r\\n"
    elif mode == "body":
        body = json.dumps({"sha": oid, "size": len(data), "encoding": "base64", "content": base64.b64encode(data).decode(), "reflected": token}).encode()
    elif mode == "redirect":
        headers = b"HTTP/2 302\\r\\nLocation: https://attacker.invalid/\\r\\n\\r\\n"
    elif mode == "identity":
        body = json.dumps({"sha": "0" * 40, "size": len(data), "encoding": "base64", "content": base64.b64encode(data).decode()}).encode()
    sys.stdout.buffer.write(headers + body)
else:
    Path(args[args.index("--dump-header") + 1]).write_bytes(headers)
    sys.stdout.buffer.write(body)
''')
    fake.chmod(0o700)
    checker = repo / "scripts/check-generated-workflows.sh"
    alternate_checker = os.environ.get("BASELINE_GUARD_TEST_CHECKER")
    if alternate_checker:
        shutil.copy2(alternate_checker, checker)
    original = checker.read_text()
    assert original.count("curl_candidate=/usr/bin/curl") == 1
    checker.write_text(original.replace("curl_candidate=/usr/bin/curl", "curl_candidate=" + repr(str(fake))))
    digest = hashlib.sha256(checker.read_bytes()).hexdigest()
    installer = repo / "scripts/install-generated-workflow-guard.sh"
    text = installer.read_text()
    lines = text.splitlines(keepends=True)
    installer.write_text("".join("reviewed_checker_sha256=" + digest + "\n" if line.startswith("reviewed_checker_sha256=") else line for line in lines))
    if evidence:
        (evidence / "checker-boundary.patch").write_text("".join(difflib.unified_diff(original.splitlines(keepends=True), checker.read_text().splitlines(keepends=True), fromfile="reviewed-checker", tofile="fixture-checker")))
        (evidence / "fixture-hashes.txt").write_text("checker_sha256=" + digest + "\ninstaller_sha256=" + hashlib.sha256(installer.read_bytes()).hexdigest() + "\ncurl_fixture_sha256=" + hashlib.sha256(fake.read_bytes()).hexdigest() + "\n")
    common = Path(run(["/usr/bin/git", "-C", str(repo), "rev-parse", "--path-format=absolute", "--git-common-dir"]).stdout.decode().strip())
    install_root = common / "jackin-generated-workflow-guard"
    install_root.mkdir(mode=0o700)
    shutil.copytree(artifacts, install_root / "artifacts", symlinks=True)
    seed_tree = "a7413118476128a1c9b4f290277eb91ddf64ea7a"
    seed_listing = run(["/usr/bin/git", "-C", source, "ls-tree", "-r", seed_tree]).stdout.decode().splitlines()
    prefixes = ("tests/functional/baselines/functional/", "apps/jackin/tests/functional/baselines/functional/")
    excluded = set()
    included = set()
    for line in seed_listing:
        header, path = line.split("\t", 1)
        oid = header.split()[2]
        if any(path.startswith(prefix) and "/" not in path[len(prefix):] and path.endswith(".tui.json") for prefix in prefixes):
            excluded.add(oid)
        else:
            included.add(oid)
    available = set(run(["/usr/bin/git", "-C", str(repo), "cat-file", "--batch-all-objects", "--batch-check=%(objectname)" ]).stdout.decode().splitlines())
    eligible = sorted((excluded - included) & available)
    if not eligible:
        raise SystemExit("fixture lacks an available full-seed-only blob")
    omitted = eligible[0]
    run(["/usr/bin/git", "-C", source, "cat-file", "-e", omitted])
    pack_path = base / "fixture-objects.pack"
    with pack_path.open("wb") as pack:
        subprocess.run(["/usr/bin/git", "-C", str(repo), "pack-objects", "--stdout"], input=("\n".join(sorted(available - {omitted})) + "\n").encode(), stdout=pack, stderr=subprocess.PIPE, env=clean, check=True)
    objects = common / "objects"
    replacement_objects = base / "replacement-objects"
    (replacement_objects / "pack").mkdir(parents=True)
    (replacement_objects / "info").mkdir()
    with pack_path.open("rb") as pack:
        subprocess.run(["/usr/bin/git", "-C", str(repo), "index-pack", "--stdin"], stdin=pack, stdout=subprocess.PIPE, stderr=subprocess.PIPE, env=dict(clean, GIT_OBJECT_DIRECTORY=str(replacement_objects)), check=True)
    old_objects = base / "old-objects"
    objects.rename(old_objects)
    replacement_objects.rename(objects)
    shutil.rmtree(old_objects)
    if subprocess.run(["/usr/bin/git", "-C", str(repo), "cat-file", "-e", omitted], env=clean, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL).returncode == 0:
        raise SystemExit("fixture failed to omit its full-seed-only blob")
    excluded_missing = {omitted}
    if evidence:
        (evidence / "omitted-object.txt").write_text(omitted + "\n")
    env = dict(clean, GITHUB_COM_TOKEN=dummy)
    def invoke(name, args, stdin=None):
        try:
            result = subprocess.run(args, cwd=repo, env=env, input=stdin, stdout=subprocess.PIPE, stderr=subprocess.PIPE, timeout=180)
        except subprocess.TimeoutExpired as error:
            captured = (error.stdout or b"") + (error.stderr or b"")
            event_bytes = events.read_bytes() if events.exists() else b""
            if evidence:
                (evidence / (name + ".result")).write_text("timeout\n")
                if dummy.encode() not in captured + event_bytes:
                    (evidence / (name + ".stdout")).write_bytes(error.stdout or b"")
                    (evidence / (name + ".stderr")).write_bytes(error.stderr or b"")
                    (evidence / (name + ".events")).write_bytes(event_bytes)
            raise
        outputs = result.stdout + result.stderr
        event_bytes = events.read_bytes() if events.exists() else b""
        if dummy.encode() in outputs or dummy.encode() in event_bytes:
            if evidence:
                (evidence / (name + ".log")).write_text("credential detected; output withheld\n")
            raise SystemExit("credential appeared in fixture output")
        if evidence:
            (evidence / (name + ".stdout")).write_bytes(result.stdout)
            (evidence / (name + ".stderr")).write_bytes(result.stderr)
            (evidence / (name + ".events")).write_bytes(event_bytes)
            (evidence / (name + ".result")).write_text("exit=" + str(result.returncode) + "\n")
        return result
    result = invoke("setup", ["/bin/bash", str(installer)])
    if dummy.encode() in result.stdout + result.stderr:
        raise SystemExit("credential appeared in setup output")
    log = events.read_text() if events.exists() else ""
    if "authenticated blob\n" not in log or "credential-in-argv" in log or "credential-scope-error" in log:
        raise SystemExit("authenticated fixed-host boundary failed")
    if "anonymous other\n" not in log:
        raise SystemExit("setup did not reach the next anonymous boundary")
    authenticated_oids = {line.split()[1] for line in log.splitlines() if line.startswith("authenticated-oid ")}
    if not (authenticated_oids & excluded_missing):
        raise SystemExit("setup did not authenticate a missing full-seed-only blob")
    for path in install_root.rglob("*"):
        if path.is_file() and not path.is_symlink() and dummy.encode() in path.read_bytes():
            raise SystemExit("credential appeared in the installation")
    if result.returncode == 0:
        raise SystemExit("fixture unexpectedly completed real tool qualification")
    if selected_case == "setup":
        print("authenticated full-seed setup reaches anonymous next boundary: PASS")
        raise SystemExit(0)
    missing = omitted
    run(["/usr/bin/git", "-C", str(repo), "update-index", "--add", "--info-only", "--cacheinfo", "100644," + missing + ",missing-auth-fixture.txt"])
    tree = run(["/usr/bin/git", "-C", str(repo), "write-tree", "--missing-ok"]).stdout.decode().strip()
    commit_env = dict(clean, GIT_AUTHOR_NAME="Guard Fixture", GIT_AUTHOR_EMAIL="fixture@example.invalid", GIT_COMMITTER_NAME="Guard Fixture", GIT_COMMITTER_EMAIL="fixture@example.invalid")
    commit = subprocess.run(["/usr/bin/git", "-C", str(repo), "commit-tree", tree, "-m", "fixture"], env=commit_env, check=True, stdout=subprocess.PIPE).stdout.decode().strip()
    installed_checker = install_root / "versions" / digest / "check-generated-workflows.sh"
    for mode in ("pre-commit", "pre-push"):
        if not selected("hook-" + mode):
            continue
        events.write_text("")
        records = None if mode == "pre-commit" else ("refs/heads/main " + commit + " refs/heads/main " + "0" * 40 + "\n").encode()
        hook_result = invoke(mode, ["/bin/bash", str(installed_checker), mode], records)
        hook_log = events.read_text() if events.exists() else ""
        if dummy.encode() in hook_result.stdout + hook_result.stderr or "authenticated " in hook_log or "credential-" in hook_log:
            raise SystemExit("ordinary checker received or exposed the credential: " + mode)
        if "anonymous blob\n" not in hook_log:
            raise SystemExit("ordinary checker did not hydrate a missing blob anonymously: " + mode)
    if selected("wrong-tree"):
        original_checker = checker.read_text()
        original_installer = installer.read_text()
        comparison = '    if [[ "$resolved_seed_tree" != "$trusted_seed_tree" ]]; then'
        if original_checker.count(comparison) != 1:
            raise SystemExit("wrong-tree fixture could not isolate the reviewed tree gate")
        checker.write_text(original_checker.replace(comparison, '    resolved_seed_tree=' + '0' * 40 + '\n' + comparison))
        wrong_tree_digest = hashlib.sha256(checker.read_bytes()).hexdigest()
        installer.write_text(original_installer.replace("reviewed_checker_sha256=" + digest, "reviewed_checker_sha256=" + wrong_tree_digest))
        events.write_text("")
        wrong_tree = invoke("wrong-tree", ["/bin/bash", str(installer)])
        wrong_tree_log = events.read_text() if events.exists() else ""
        if wrong_tree.returncode == 0 or b"approved seed tree does not match the reviewed identity" not in wrong_tree.stderr or "authenticated " in wrong_tree_log:
            raise SystemExit("wrong seed tree was not rejected before authenticated HTTP")
        checker.write_text(original_checker)
        installer.write_text(original_installer)
        if evidence:
            (evidence / "wrong-tree-mutation.txt").write_text("injected resolved_seed_tree=" + "0" * 40 + " immediately before reviewed tree comparison\nchecker_sha256=" + wrong_tree_digest + "\n")
        print("wrong seed tree rejected before authenticated HTTP: PASS")
    seed_cache = install_root / "cache" / "seed"
    for mode, diagnostic in (("header", b"response contains the setup credential"), ("body", b"response contains the setup credential"), ("redirect", b"redirects are not permitted"), ("identity", b"response object identity does not match the requested blob")):
        if not selected("response-" + mode):
            continue
        if seed_cache.exists():
            for path in seed_cache.rglob("*"):
                if not path.is_symlink():
                    path.chmod(path.stat().st_mode | 0o700)
            shutil.rmtree(seed_cache)
        scenario.write_text(mode)
        events.write_text("")
        rejected = invoke("response-" + mode, ["/bin/bash", str(installer)])
        log = events.read_text() if events.exists() else ""
        if rejected.returncode == 0 or diagnostic not in rejected.stderr or dummy.encode() in rejected.stdout + rejected.stderr or "credential-" in log:
            raise SystemExit("authenticated response rejection failed: " + mode)
        if log.count("authenticated blob\n") != 1:
            raise SystemExit("authenticated response escaped the fixed endpoint: " + mode)
        if seed_cache.exists() and any(seed_cache.iterdir()):
            raise SystemExit("rejected response published a seed cache: " + mode)
        for path in install_root.rglob("*"):
            if path.is_file() and not path.is_symlink() and dummy.encode() in path.read_bytes():
                raise SystemExit("credential appeared in the installation: " + mode)
    print("selected credential boundary cases: PASS")
    print("private installed checker fixture sha256:", digest)
    print("authenticated fixed-host hydration and anonymous next boundary: PASS")
