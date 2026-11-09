#!/usr/bin/env python3
# SPDX-FileCopyrightText: 2026 Alexey Zhokhov
# SPDX-License-Identifier: Apache-2.0
import hashlib
import json
import os
import stat
import sys

for root in sys.argv[1:]:
    root = os.path.abspath(root)
    records = []

    def add(path, relative):
        metadata = os.lstat(path)
        entry = {
            "path": relative,
            "mode": format(stat.S_IMODE(metadata.st_mode), "04o"),
            "uid": metadata.st_uid,
            "gid": metadata.st_gid,
            "links": metadata.st_nlink,
            "device": metadata.st_dev,
            "inode": metadata.st_ino,
            "mtime_ns": metadata.st_mtime_ns,
            "ctime_ns": metadata.st_ctime_ns,
        }
        attributes = {}
        for name in sorted(os.listxattr(path, follow_symlinks=False)):
            value = os.getxattr(path, name, follow_symlinks=False)
            attributes[name] = hashlib.sha256(value).hexdigest()
        entry["xattrs_sha256"] = attributes
        if stat.S_ISLNK(metadata.st_mode):
            entry["type"] = "link"
            entry["target"] = os.readlink(path)
        elif stat.S_ISDIR(metadata.st_mode):
            entry["type"] = "directory"
        elif stat.S_ISREG(metadata.st_mode):
            digest = hashlib.sha256()
            with open(path, "rb") as stream:
                for block in iter(lambda: stream.read(1024 * 1024), b""):
                    digest.update(block)
            entry["type"] = "file"
            entry["sha256"] = digest.hexdigest()
        else:
            entry["type"] = "special"
            entry["device_type"] = metadata.st_rdev
        records.append(entry)
        if entry["type"] == "directory":
            with os.scandir(path) as children:
                names = sorted((child.name for child in children), key=os.fsencode)
            for name in names:
                child = os.path.join(path, name)
                child_relative = name if relative == "." else f"{relative}/{name}"
                add(child, child_relative)

    if not os.path.lexists(root):
        records.append({"path": ".", "type": "missing"})
    else:
        add(root, ".")
    for record in records:
        print(json.dumps(record, ensure_ascii=True, separators=(",", ":")))
