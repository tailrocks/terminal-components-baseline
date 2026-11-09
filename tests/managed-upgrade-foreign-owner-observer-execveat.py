#!/usr/bin/env python3
# SPDX-FileCopyrightText: 2026 Alexey Zhokhov
# SPDX-License-Identifier: Apache-2.0
import ctypes
import hashlib
import json
import os
import sys

path = os.path.abspath(sys.argv[1])
witness_path = sys.argv[2]
descriptor = os.open(path, os.O_RDONLY | os.O_CLOEXEC)
metadata = os.fstat(descriptor)
digest = hashlib.sha256()
with open(path, "rb") as source:
    for block in iter(lambda: source.read(1048576), b""):
        digest.update(block)
witness = {
    "path": path,
    "fd": descriptor,
    "device": metadata.st_dev,
    "inode": metadata.st_ino,
    "size": metadata.st_size,
    "sha256": digest.hexdigest(),
}
with open(witness_path, "x", encoding="utf-8") as stream:
    json.dump(witness, stream, sort_keys=True, separators=(",", ":"))
    stream.write("\n")
    stream.flush()
    os.fsync(stream.fileno())
library = ctypes.CDLL(None, use_errno=True)
function = library.execveat
function.argtypes = [ctypes.c_int, ctypes.c_char_p, ctypes.POINTER(ctypes.c_char_p), ctypes.POINTER(ctypes.c_char_p), ctypes.c_int]
function.restype = ctypes.c_int
argv = (ctypes.c_char_p * 4)(os.fsencode(path), b"%s\n", b"observer-execveat-sentinel", None)
environment = (ctypes.c_char_p * 1)()
function(descriptor, b"", argv, environment, 0x1000)
error_number = ctypes.get_errno()
raise OSError(error_number, os.strerror(error_number))
