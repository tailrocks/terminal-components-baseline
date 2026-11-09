#!/usr/bin/env bash
# SPDX-FileCopyrightText: 2026 Alexey Zhokhov
# SPDX-License-Identifier: Apache-2.0
set -eu
/usr/bin/printf 'observer-wrapper-start\n'
exec /usr/bin/env -i PATH=/usr/bin:/bin /bin/bash /work/evidence/observer-child-sentinel
