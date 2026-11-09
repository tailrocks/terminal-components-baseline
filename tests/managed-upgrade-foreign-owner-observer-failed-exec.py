#!/usr/bin/env python3
# SPDX-FileCopyrightText: 2026 Alexey Zhokhov
# SPDX-License-Identifier: Apache-2.0
import errno
import os
import sys

path = sys.argv[1]
try:
    os.execve(path, [path], {})
except OSError as error:
    if error.errno != errno.ENOENT:
        raise
    print("observer-failed-exec-ENOENT")
