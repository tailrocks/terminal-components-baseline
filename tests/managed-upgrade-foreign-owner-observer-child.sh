#!/usr/bin/env bash
# SPDX-FileCopyrightText: 2026 Alexey Zhokhov
# SPDX-License-Identifier: Apache-2.0
set -eu
/usr/bin/env -i PATH=/usr/bin:/bin /work/evidence/observer-child-elf-sentinel 'observer-child-elf\n'
/usr/bin/printf 'observer-child-shell\n'
exec /usr/bin/env -i PATH=/usr/bin:/bin /bin/bash /work/evidence/observer-grandchild-sentinel
