<!-- SPDX-License-Identifier: LicenseRef-Proprietary -->

# Functional frame synchronization quarantine

Tracking reference: `JACKIN-UI-FRAME-SYNC-2026-11-09`.

The root Rust gate on candidate binary SHA256 `0ace67afac6540e492fd2041476cbe92eb81b2171aa0adbf6998a7ef1521dbfd` failed `legacy::j01_prelude::j01_prelude_steps` and `legacy::j02_environments::j02_environments_env`. The same binary, launch arguments, lockfile, reference, and terminal dimensions passed both exact selectors in a scoped rerun. Both cases are temporarily ignored with this tracking reference while their readiness conditions are corrected.

## J01 narrow modal

The failed narrow capture had 80x24 terminal dimensions and the expected duplicate-name text, but its modal was shifted one cell right from the frozen frame. Its corner began at column 10; the accepted frame begins at column 9. The existing `status_duplicate` predicate therefore accepted a frame that was not laid out for the restored width. After restoring 80x24, the test checked dimensions and semantic text but did not wait for the expected narrow modal geometry or a visually quiet capture.

The case now gates the frozen comparison on the accepted modal corner geometry, waits for the terminal to become visually quiet, and asserts that the status and geometry still hold on the returned snapshot. A persistent wrong layout fails readiness or the assertion. The frozen frame comparison and its baseline remain unchanged.

Original evidence:

- Full gate log: `/Users/donbeave/Projects/jackin-project/.rewrite-tools/baseline-combined-api-guard-gates-20261109-v4/nextest.log`.
- Original session and frame checkpoints: `/Users/donbeave/Projects/jackin-project/wt/baseline178-integrated-v1/target/functional/j01-prelude-steps-42347-1794204713170723000-0/`.
- Frozen comparison report and actual candidate sample: `/Users/donbeave/Library/Caches/mbx/targets/v1/ce566b8c22986880024c99d4b10fd08b7a34593cb4eff9fa81f5a5cac41fb511/functional/j01-prelude-steps-42347-1794204713170723000-0/check-artifacts/pre_refactor/j01_prelude/j01_prelude_steps_narrow/863a2caa8c69e7893a60540e2b8c2dc2fd8a5399109c59842cacb9a8c0e07e4a/`.

## J02 Save focus

The failed checkpoint contained the `Enter Save` help and Cancel label, so the old wait returned, but focus was still on the workspace Scope row. The immediately following assertion expected the Save button to be focused. The terminal had advanced to Save by the later failure capture. Save is an enabled primary button regardless of focus; the form validates required values on activation. The case now locates the unique Save button on the actual new/edit environment form and gets its center from that snapshot. One existing duplicate-submit path reaches Save with real Tab input one step at a time, observing each focused-row change within one six-stop form cycle, and asserts focus before invoking the helper. This exercises the already-focused one-click activation. When initially unfocused, the helper clicks to focus and waits for a fresh completion marker, form closure, or the same form with Save visibly focused before any second activation. The expected completion marker must be absent in the pre-click snapshot. Existing post-click duplicate, added, and updated state assertions remain the activation check. The original Scope-focused checkpoint is a valid button-location state; a manager screen without a form title and the unrelated workspace Save preview are rejected.

Original evidence:

- Full gate log: `/Users/donbeave/Projects/jackin-project/.rewrite-tools/baseline-combined-api-guard-gates-20261109-v4/nextest.log`.
- Session, actual false-ready checkpoint `026.txt`, and post-failure screen: `/Users/donbeave/Projects/jackin-project/wt/baseline178-integrated-v1/target/functional/j02-environments-env-62163-1794204734807486000-0/`.

## API boundary and closure

Tuiscotti snapshots atomically capture terminal emulator cells and dimensions. That guarantees a coherent snapshot, not application input processing or redraw completion. Its visual quiet wait samples the active screen and is not an application frame or DEC 2026 synchronized-update boundary. Tests must wait for the actual screen state they require and treat a persistent layout mismatch as a failure.

## Closure

The temporary ignores for these two cases were removed after the independent reviewer recorded a scoped KEEP in `/Users/donbeave/Projects/jackin-project/.rewrite-tools/baseline-combined-api-guard-gates-20261109-v4/independent-v4/receipt.md`. That receipt includes two independent targeted runs with both cases passing, and retains the actual J02 Save-focused snapshot followed by one click and the duplicate-rejected state. The earlier unfocused paths remain covered by the v3 retained traces referenced by that receipt.

After removing the ignores, the normal exact-selector command ran both tests without `--run-ignored`: nextest run `714d906e-4039-421f-a540-9d18528a8385`, 2 passed, 2142 skipped. `mise exec -- mbx fmt --all --check` passed, and targeted `mise exec -- mbx clippy --locked --manifest-path apps/jackin/Cargo.toml --package jackin-console --test functional --all-features -- -D warnings` passed.

This closes only these two test readiness defects. It does not establish a general application-frame or DEC 2026 synchronization guarantee. The frozen visual baselines and all existing post-submit outcome assertions are unchanged. Keep the separate resize-fence quarantine unchanged.
