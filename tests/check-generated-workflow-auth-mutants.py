#!/usr/bin/env python3
# SPDX-FileCopyrightText: 2026 Alexey Zhokhov
# SPDX-License-Identifier: Apache-2.0
import hashlib
import os
from pathlib import Path
import subprocess
import sys
import tempfile

root = Path(__file__).resolve().parent.parent
checker = root / "scripts/check-generated-workflows.sh"
fixture = root / "tests/check-generated-workflow-auth.py"
source = checker.read_text()
expected_checker = "9d2fb0526f89cd58257579876786efeaacc3f88119f2f8cb782b8b0873d5c1eb"
if hashlib.sha256(checker.read_bytes()).hexdigest() != expected_checker:
    raise SystemExit("mutant driver requires the independently reviewed v10 checker")

def replace_once(text, old, new):
    if text.count(old) != 1:
        raise SystemExit("mutant target is missing or ambiguous")
    return text.replace(old, new, 1)

def make_mutant(name):
    if name == "setup-auth":
        return replace_once(source, "    hydration_environment+=(-S 'GITHUB_COM_TOKEN=${GITHUB_COM_TOKEN}')", "    :")
    if name in ("hook-pre-commit", "hook-pre-push"):
        result = replace_once(source, 'if [[ "${1-}" == prepare-cache ]]; then', 'if [[ "${1-}" == prepare-cache || "${1-}" == pre-commit || "${1-}" == pre-push ]]; then')
        old = '"${prepare_cache_mode:-0}" -eq 1 && ( "$kind" == trusted || "${trusted_snapshot_auth:-0}" -eq 1 )'
        return replace_once(result, old, '( "$kind" == trusted || "$kind" == snapshot )')
    if name == "wrong-tree":
        old = '''      printf 'generated-workflows: approved seed tree does not match the reviewed identity\\n' >&2
      exit 1'''
        return replace_once(source, old, '      resolved_seed_tree=$trusted_seed_tree')
    if name in ("response-header", "response-body"):
        return replace_once(source, 'if token.encode("ascii") in payload:', 'if False:')
    if name == "response-redirect":
        result = replace_once(source, 'if status in redirect_statuses or locations:', 'if False:')
        return result.replace('if status != 200:', 'if status not in (200, 302):', 1)
    if name == "response-identity":
        return replace_once(source, 'if not isinstance(document, dict) or document.get("sha") != oid:', 'if not isinstance(document, dict):')
    if name == "argv":
        return replace_once(source, '"--variable", "%GITHUB_COM_TOKEN"', '"--variable", token')
    raise SystemExit("unknown mutant")

cases = {
    "setup-auth": ("setup", "authenticated fixed-host boundary failed"),
    "hook-pre-commit": ("hook-pre-commit", "ordinary checker received or exposed the credential: pre-commit"),
    "hook-pre-push": ("hook-pre-push", "ordinary checker received or exposed the credential: pre-push"),
    "wrong-tree": ("wrong-tree", "wrong seed tree was not rejected before authenticated HTTP"),
    "response-header": ("response-header", "authenticated response rejection failed: header"),
    "response-body": ("response-body", "authenticated response rejection failed: body"),
    "response-redirect": ("response-redirect", "authenticated response rejection failed: redirect"),
    "response-identity": ("response-identity", "authenticated response rejection failed: identity"),
    "argv": ("setup", "authenticated fixed-host boundary failed"),
}
if sys.argv[1:] == ["--list"]:
    for name, (case, _) in cases.items():
        print(name, case)
    raise SystemExit(0)
if len(sys.argv) != 2 or sys.argv[1] not in cases:
    raise SystemExit("usage: check-generated-workflow-auth-mutants.py MUTANT | --list")
name = sys.argv[1]
case, expected_failure = cases[name]
with tempfile.TemporaryDirectory(prefix="guard-auth-mutant.") as directory:
    mutant = Path(directory) / "checker.sh"
    mutant.write_text(make_mutant(name))
    mutant.chmod(0o700)
    digest = hashlib.sha256(mutant.read_bytes()).hexdigest()
    environment = dict(os.environ, BASELINE_GUARD_TEST_CHECKER=str(mutant))
    result = subprocess.run([sys.executable, str(fixture), "--case", case], env=environment, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)
    if result.returncode == 0 or expected_failure not in result.stderr:
        print(result.stdout, end="")
        print(result.stderr, end="", file=sys.stderr)
        raise SystemExit("mutant did not fail at the required public assertion: " + name)
    print(result.stdout.replace("GuardDummy" + "Credential_47291", "[dummy credential redacted]"), end="")
    print(result.stderr.replace("GuardDummy" + "Credential_47291", "[dummy credential redacted]"), end="", file=sys.stderr)
    print("mutant", name, "checker_sha256", digest, "detected by", case)
