#!/usr/bin/env python3
# SPDX-FileCopyrightText: 2026 Alexey Zhokhov
# SPDX-License-Identifier: Apache-2.0
import hashlib
import json
import os
import re
import shutil
import stat
import struct
import subprocess
import tempfile
from pathlib import Path

source = Path("/observer-source")
proof = Path("/proof")
root = Path("/opt/exec-observer")
evidence = root / "evidence"
expected = {
    "strace-7.3.tar.xz": ("529634c40910895a9a1f3c5c1e65da2ca9ed72fb158dfc4bcc89efdf05e39b72", 2928960),
    "strace-7.3.tar.xz.asc": ("c7aea1a61162024da80e92852dc78e88edc03d2b61bb72579ece6da7d77f2873", 833),
    "GPG-KEY": ("8d038c52364a3937067d847c2c0397a53f5c3b5fc12b725f0d1be6d6eb52f2ad", 5401),
    "origin-metadata.json": ("8e673c2c43cb4d78d6e8ac8d736b8dd2beb09ec33ea41130007118bd7b1293be", None),
}
release_primary_fingerprint = "7BECFE3AF7B280BB52FF77F104BA4521C996DDE1"
release_subkey_fingerprints = {
    "FB3DCD19FF874735E4F61CE64E9FE8522F160EC2",
    "9D9065873A2C5345A447602A12A6C4AD7D939856",
}
release_signing_subkey_fingerprints = {"FB3DCD19FF874735E4F61CE64E9FE8522F160EC2"}
build_tool_packages = (
    "apt",
    "bash",
    "binutils",
    "build-essential",
    "ca-certificates",
    "coreutils",
    "dpkg",
    "findutils",
    "gcc",
    "gnupg",
    "gpgv",
    "grep",
    "libc-bin",
    "libc6-dev",
    "make",
    "mawk",
    "python3",
    "sed",
    "tar",
    "xz-utils",
)


def sha(path):
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for block in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(block)
    return digest.hexdigest()


def write(path, content):
    path = Path(path)
    path.parent.mkdir(mode=0o755, parents=True, exist_ok=True)
    path.write_bytes(content.encode("utf-8") if isinstance(content, str) else content)
    path.chmod(0o644)


def run(command, timeout, cwd=None, environment=None):
    return subprocess.run(command, cwd=cwd, env=environment, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=False, timeout=timeout, check=False)


def package_identity(path):
    header = path.read_bytes()[:20]
    if len(header) < 20 or header[:4] != b"\x7fELF" or header[4] != 2 or header[5] not in (1, 2):
        raise RuntimeError(f"not an ELF64 executable: {path}")
    endian = "<" if header[5] == 1 else ">"
    machine = struct.unpack(f"{endian}H", header[18:20])[0]
    return machine


def command_evidence(name, result):
    write(evidence / f"{name}.stdout", result.stdout)
    write(evidence / f"{name}.stderr", result.stderr)
    write(evidence / f"{name}.status.txt", f"status={result.returncode}\n")


def fail(message):
    write(evidence / "observer-tools-build-status.txt", f"status=unavailable\nreason={message}\n")
    write(evidence / "observer-tools-closure.tsv", "")
    write(evidence / "observer-tools-elf.tsv", "")
    write(evidence / "observer-tools-versions.tsv", "")
    write(evidence / "observer-tools-ldd.txt", "")


def main():
    root.mkdir(mode=0o755, parents=True, exist_ok=True)
    for directory in (root / "bin", root / "lib", evidence):
        directory.mkdir(mode=0o755, exist_ok=True)
        directory.chmod(0o755)
    for name in ("observer-tools-build-status.txt", "observer-tools-build.log", "observer-tools-configure-help.txt", "observer-tools-ldd.txt", "observer-tools-ldd.stderr", "observer-tools-closure.tsv", "observer-tools-elf.tsv", "observer-tools-versions.tsv", "observer-tools-signature-status.txt", "observer-tools-signature.stdout", "observer-tools-signature.stderr", "observer-tools-source.tsv", "observer-tools-strace-version.txt", "observer-tools-strace-version.stderr"):
        write(evidence / name, "")
    write(evidence / "observer-tools-build-status.txt", "status=unavailable\nreason=not-started\n")
    try:
        for name, (digest, size) in expected.items():
            path = source / name
            if path.is_symlink() or not path.is_file() or sha(path) != digest or (size is not None and path.stat().st_size != size):
                raise RuntimeError(f"frozen official source input differs: {name}")
        metadata = json.loads((source / "origin-metadata.json").read_text(encoding="utf-8"))
        if metadata.get("format") != "strace-official-source-origin-v1" or metadata.get("version") != "7.3" or metadata.get("signature_status") != "not-verified":
            raise RuntimeError("frozen official source origin metadata has an unexpected schema or signature claim")
        if metadata.get("requests", {}).get("tarball", {}).get("url") != "https://strace.io/files/7.3/strace-7.3.tar.xz" or metadata["requests"]["tarball"].get("sha256") != expected["strace-7.3.tar.xz"][0]:
            raise RuntimeError("origin metadata does not bind the exact official 7.3 source tarball")
        if metadata.get("requests", {}).get("official_release_index", {}).get("status") != 200 or metadata["requests"]["official_release_index"].get("url") != "https://strace.io/files/7.3/":
            raise RuntimeError("origin metadata does not bind the official 7.3 release index")
        latest = metadata.get("requests", {}).get("latest_release_page", {})
        if latest.get("status") != 200 or latest.get("url") != "https://github.com/strace/strace/releases/latest" or latest.get("final_url") != "https://github.com/strace/strace/releases/tag/v7.3":
            raise RuntimeError("origin metadata does not bind the official latest stable release page to v7.3")
        release_index = (source / "release-index.html").read_bytes()
        if b"strace-7.3.tar.xz" not in release_index or b"strace-7.3.tar.xz.asc" not in release_index or sha(source / "release-index.html") != metadata["requests"]["official_release_index"].get("sha256"):
            raise RuntimeError("official release index does not bind the frozen source tarball and detached signature")
        latest_page = (source / "release-latest.html").read_bytes()
        if sha(source / "release-latest.html") != latest.get("sha256") or b"strace" not in latest_page:
            raise RuntimeError("latest stable release page bytes differ from the retained origin observation")
        source_records = [
            f"version\t7.3",
            f"tarball_url\t{metadata['requests']['tarball']['url']}",
            f"tarball_sha256\t{expected['strace-7.3.tar.xz'][0]}",
            f"signature_url\t{metadata['requests']['detached_signature']['url']}",
            f"signature_sha256\t{expected['strace-7.3.tar.xz.asc'][0]}",
            f"key_url\t{metadata['requests']['release_key']['url']}",
            f"key_sha256\t{expected['GPG-KEY'][0]}",
            f"origin_metadata_sha256\t{expected['origin-metadata.json'][0]}",
        ]
        write(evidence / "observer-tools-source.tsv", "\n".join(source_records) + "\n")
        with tempfile.TemporaryDirectory(prefix="exec-observer-gnupg-") as key_temporary:
            gnupg = Path(key_temporary) / "gnupg"
            gnupg.mkdir(mode=0o700)
            env = dict(os.environ)
            env["GNUPGHOME"] = str(gnupg)
            imported = run(["/usr/bin/gpg", "--batch", "--import", str(source / "GPG-KEY")], 30, environment=env)
            command_evidence("observer-tools-gpg-import", imported)
            if imported.returncode != 0:
                raise RuntimeError("release key import failed")
            listed = run(["/usr/bin/gpg", "--batch", "--with-colons", "--fingerprint", "--list-keys"], 30, environment=env)
            command_evidence("observer-tools-gpg-key-list", listed)
            if listed.returncode != 0:
                raise RuntimeError("release key fingerprint query failed")
            keys = []
            current_key = None
            pending_fingerprint = None
            for line in listed.stdout.decode("utf-8", "strict").splitlines():
                fields = line.split(":")
                if fields[0] == "pub":
                    if pending_fingerprint is not None:
                        raise RuntimeError("release key listing omitted a primary fingerprint")
                    current_key = {"primary": None, "subkeys": []}
                    keys.append(current_key)
                    pending_fingerprint = "primary"
                elif fields[0] == "sub":
                    if current_key is None or pending_fingerprint is not None or len(fields) <= 11:
                        raise RuntimeError("release key listing has an unbound subkey record")
                    current_key["subkeys"].append({"fingerprint": None, "capabilities": fields[11]})
                    pending_fingerprint = "subkey"
                elif fields[0] == "fpr":
                    if current_key is None or pending_fingerprint is None or len(fields) <= 9:
                        raise RuntimeError("release key listing has an unbound fingerprint record")
                    fingerprint = fields[9]
                    if not re.fullmatch(r"[A-F0-9]{40,64}", fingerprint):
                        raise RuntimeError("release key listing contains a malformed fingerprint")
                    if pending_fingerprint == "primary" and current_key["primary"] is None:
                        current_key["primary"] = fingerprint
                    elif pending_fingerprint == "subkey" and current_key["subkeys"][-1]["fingerprint"] is None:
                        current_key["subkeys"][-1]["fingerprint"] = fingerprint
                    else:
                        raise RuntimeError("release key listing contains an unexpected fingerprint record")
                    pending_fingerprint = None
            parsed_subkeys = keys[0]["subkeys"] if len(keys) == 1 else []
            subkey_fingerprints = [item["fingerprint"] for item in parsed_subkeys]
            signing_subkeys = {item["fingerprint"] for item in parsed_subkeys if item["fingerprint"] and "s" in item["capabilities"]}
            if pending_fingerprint is not None or len(keys) != 1 or keys[0]["primary"] != release_primary_fingerprint or set(subkey_fingerprints) != release_subkey_fingerprints or signing_subkeys != release_signing_subkey_fingerprints:
                raise RuntimeError("release key import did not yield the pinned single primary key with its two expected subkeys")
            primary_fingerprint = keys[0]["primary"]
            exported = run(["/usr/bin/gpg", "--batch", "--export", primary_fingerprint], 30, environment=env)
            if exported.returncode != 0 or not exported.stdout:
                raise RuntimeError("release key export failed")
            write(evidence / "observer-tools-gpg-export.status.txt", f"status={exported.returncode}\nbytes={len(exported.stdout)}\n")
            keyring = Path(key_temporary) / "release-keyring.gpg"
            keyring.write_bytes(exported.stdout)
            verified = run(["/usr/bin/gpgv", "--status-fd=1", "--keyring", str(keyring), str(source / "strace-7.3.tar.xz.asc"), str(source / "strace-7.3.tar.xz")], 30, environment=env)
            write(evidence / "observer-tools-signature.stdout", verified.stdout)
            write(evidence / "observer-tools-signature.stderr", verified.stderr)
            status_lines = [line.split() for line in verified.stdout.decode("utf-8", "replace").splitlines() if line.startswith("[GNUPG:] ")]
            valid_signatures = [fields for fields in status_lines if len(fields) >= 3 and fields[1] == "VALIDSIG"]
            if verified.returncode != 0 or len(valid_signatures) != 1 or any(fields[1] in {"BADSIG", "ERRSIG", "NO_PUBKEY"} for fields in status_lines):
                raise RuntimeError("official release detached signature verification failed")
            signature_fields = valid_signatures[0]
            signing_fingerprint = signature_fields[2]
            signature_primary = signature_fields[-1] if len(signature_fields) >= 12 else signing_fingerprint
            if signature_primary != primary_fingerprint or signing_fingerprint not in {primary_fingerprint, *signing_subkeys}:
                raise RuntimeError("detached signature is not bound to the pinned release primary key and its certified signing subkey")
            write(evidence / "observer-tools-signature-status.txt", f"status={verified.returncode}\nprimary_fingerprint={primary_fingerprint}\nsubkey_fingerprints={','.join(sorted(subkey_fingerprints))}\nsigning_subkey_fingerprints={','.join(sorted(signing_subkeys))}\nsigning_fingerprint={signing_fingerprint}\nsignature_primary_fingerprint={signature_primary}\nvalid_signature_record=present\n")
            source_fingerprint = primary_fingerprint
        source_temporary = Path("/tmp/exec-observer-strace-7.3")
        shutil.rmtree(source_temporary, ignore_errors=True)
        source_temporary.mkdir(mode=0o700, parents=True)
        source_tree = source_temporary / "strace-7.3"
        unpacked = run(["/usr/bin/tar", "-xf", str(source / "strace-7.3.tar.xz"), "-C", str(source_temporary)], 30)
        command_evidence("observer-tools-source-extract", unpacked)
        if unpacked.returncode != 0 or not (source_tree / "configure").is_file():
            raise RuntimeError("official strace source tarball could not be extracted")
        help_result = run([str(source_tree / "configure"), "--help"], 30, cwd=source_tree)
        command_evidence("observer-tools-configure-help", help_result)
        write(evidence / "observer-tools-configure-help.txt", help_result.stdout)
        if help_result.returncode != 0 or b"--enable-stacktrace=yes|no|check" not in help_result.stdout or b"--enable-mpers=yes|no|check|m32|mx32" not in help_result.stdout:
            raise RuntimeError("source configure help does not expose the reviewed stacktrace and personality options")
        build_command = [
            "/usr/bin/timeout",
            "--signal=TERM",
            "--kill-after=5s",
            "300",
            "/bin/bash",
            "-c",
            "set -eu; ./configure --prefix=/opt/exec-observer --enable-stacktrace=no --enable-mpers=no; /usr/bin/make -j2; /usr/bin/make install",
        ]
        built = run(build_command, 315, cwd=source_tree)
        command_evidence("observer-tools-source-build", built)
        write(evidence / "observer-tools-build.log", built.stdout + built.stderr)
        if built.returncode != 0:
            raise RuntimeError(f"bounded strace source build failed with status {built.returncode}")
        tracer = root / "bin/strace"
        if not tracer.is_file() or tracer.is_symlink() or package_identity(tracer) != 183:
            raise RuntimeError("built observer is not an AArch64 ELF executable")
        version = run([str(tracer), "-V"], 10)
        command_evidence("observer-tools-version", version)
        write(evidence / "observer-tools-strace-version.txt", version.stdout)
        write(evidence / "observer-tools-strace-version.stderr", version.stderr)
        if version.returncode != 0 or b"strace -- version 7.3" not in version.stdout:
            raise RuntimeError("built observer does not report the frozen latest stable version 7.3")
        ldd = run(["/usr/bin/ldd", str(tracer)], 10)
        write(evidence / "observer-tools-ldd.txt", ldd.stdout)
        write(evidence / "observer-tools-ldd.stderr", ldd.stderr)
        if ldd.returncode != 0 or b"not found" in ldd.stdout or b"not found" in ldd.stderr:
            raise RuntimeError("built observer dynamic dependency closure is incomplete")
        readelf = run(["/usr/bin/readelf", "-l", str(tracer)], 10)
        command_evidence("observer-tools-readelf", readelf)
        interpreter_match = re.search(rb"Requesting program interpreter: ([^]]+)\]", readelf.stdout)
        if readelf.returncode != 0 or interpreter_match is None:
            raise RuntimeError("built observer does not expose an absolute ELF interpreter")
        interpreter = interpreter_match.group(1).decode("utf-8")
        if not interpreter.startswith("/"):
            raise RuntimeError("built observer ELF interpreter is not absolute")
        dependencies = []
        for raw in ldd.stdout.decode("utf-8", "replace").splitlines():
            arrow = re.search(r"=>\s+(/\S+)", raw)
            direct = re.match(r"\s*(/\S+)", raw)
            dependency = arrow.group(1) if arrow else direct.group(1) if direct else None
            if dependency is not None:
                dependencies.append(dependency)
        dependencies.append(interpreter)
        copied = {}
        closure_records = []
        for original in dependencies:
            resolved = Path(original).resolve(strict=True)
            if not resolved.is_file():
                raise RuntimeError(f"observer dependency is not a regular file: {original}")
            digest = sha(resolved)
            name = Path(original).name
            if name in copied and copied[name] != digest:
                raise RuntimeError(f"observer dependency basename collision has unequal bytes: {name}")
            copied[name] = digest
            destination = root / "lib" / name
            if not destination.exists():
                shutil.copyfile(resolved, destination)
                destination.chmod(0o755)
            elif sha(destination) != digest:
                raise RuntimeError(f"private observer dependency copy differs: {name}")
            machine = package_identity(destination)
            if machine != 183:
                raise RuntimeError(f"private observer dependency is not AArch64: {name}")
            closure_records.append(f"{original}\t{resolved}\t{destination}\t{digest}\t{machine}")
        native_source = proof / "managed-upgrade-foreign-owner-observer-native-true.c"
        native_true = root / "bin/observer-native-true"
        compiled = run(["/usr/bin/gcc", "-static", "-O2", "-s", "-o", str(native_true), str(native_source)], 30)
        command_evidence("observer-tools-native-true-build", compiled)
        if compiled.returncode != 0 or not native_true.is_file() or package_identity(native_true) != 183:
            raise RuntimeError("native AArch64 static true fixture could not be built")
        closure_records.insert(0, f"tracer\t{tracer.resolve()}\t{tracer}\t{sha(tracer)}\t{package_identity(tracer)}")
        closure_records.append(f"native_true\t{native_true.resolve()}\t{native_true}\t{sha(native_true)}\t{package_identity(native_true)}")
        write(evidence / "observer-tools-closure.tsv", "original_path\tresolved_path\tprivate_path\tsha256\telf_machine\n" + "\n".join(closure_records) + "\n")
        package_lines = []
        inventory = run(["/usr/bin/dpkg-query", "-W", "-f=${binary:Package}\t${Version}\n", *build_tool_packages], 10)
        command_evidence("observer-tools-packages", inventory)
        if inventory.returncode != 0:
            raise RuntimeError("observer builder package versions could not be recorded")
        package_lines = inventory.stdout.decode("utf-8", "strict").splitlines()
        if len(package_lines) != len(build_tool_packages) or any("\t" not in line for line in package_lines):
            raise RuntimeError("observer builder package inventory is incomplete")
        write(evidence / "observer-tools-versions.tsv", "package\tversion\n" + "\n".join(package_lines) + "\n")
        write(evidence / "observer-tools-elf.tsv", "path\telf_machine\tsha256\n" + "\n".join(f"{path}\t{package_identity(path)}\t{sha(path)}" for path in (tracer, root / "lib" / Path(interpreter).name, native_true)) + "\n")
        write(evidence / "observer-tools-build-status.txt", f"status=available\nsource=7.3\nsource_signature=verified\nrelease_key_fingerprint={source_fingerprint}\nstrace_machine=183\nelf_interpreter={interpreter}\ndependency_count={len(dependencies)}\nconfigure_timeout_seconds=300\nmake_jobs=2\n")
    except (OSError, RuntimeError, subprocess.TimeoutExpired, UnicodeError, ValueError, KeyError, json.JSONDecodeError) as error:
        fail(str(error).replace("\n", " "))


main()
