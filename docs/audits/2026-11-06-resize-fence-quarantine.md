<!-- SPDX-License-Identifier: LicenseRef-Proprietary -->

# Resize fence lifecycle quarantine

The functional case `lifecycle::showcase_help_survives_shrink_and_growth_and_returns_focus` is quarantined after one failure in the full root Rust gate run on the unchanged source tree `e5080af6e4dca3c0464640419d159185d96f6991`. At `help reflows at 160×50`, the fence timed out after 10 seconds; the BEL count remained 3 → 3 and the help screen was still visible. The identical targeted case passed twice without source changes. These results establish intermittency, not a cause or a confirmed UI regression.

The captured failure has no raw stdin or handled-event trace. A shared terminal input framing hazard is plausible: an unmodified Escape byte immediately followed by the bracketed-paste fence opener may be parsed together, consuming the opener. That does not explain the still-visible help screen on its own and does not establish that this sequence occurred. The product is not exonerated; investigate the shared input protocol and framing path rather than extending deadlines or adding sleeps.

Unquarantine only after a deterministic reproduction identifies the framing failure, a shared harness fix addresses the cause, an independent reviewer observes the relevant test fail on broken behavior and pass on the fix, and repeated functional runs pass. Preserve the native Escape behavior; do not use an unnegotiated CSI-u shortcut that masks the terminal interaction.

Evidence: `handoff/state-2026-11-05/current/baseline-fence-flake-investigation.md` records the investigation and exact targeted command. The frozen full-gate log is `baseline-pr4-ui-root-gates-v3/nextest.log`; targeted passing output is `baseline-fence-targeted.log`.
