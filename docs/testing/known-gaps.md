# Known gaps

These are current-source observations. They are not permission to change product behaviour.

## Catalog and specimen

`SpecimenState` has 14 states, including `editing`.
`src/catalog.rs` lists 13 `state::` ids and omits `editing`.
Pages that draw `editing` still owe a specimen checkpoint.
The catalog entry is not added in this test programme because that is a production change.
The omission stays a characterized inventory gap.

## Clock paste

The shared harness sets `JUNIE_TUI_TEST_CLOCK=1` and Showcase page visits paste `junie-tui:advance-clock:`.
Only Jackin overrides `advance_clock` to consume that paste.
`shell::handle` still delivers the paste as ordinary input when `advance_clock` returns `None`.
The harness must stop sending the paste to applications that do not implement it.
No production clock hook is added to Showcase.

## Existing claims

The keyboard-activation test can accept a result from an earlier click.
The table-sort test discards the row-order value it captured.
The form test does not reset or submit a valid form.
The dialog test does not cycle the modal focus ring.
The Jackin stop, settings, launch-failure, and reconnect baselines do not show the state their names claim.
TablePro `resize_60x20` and Holla `resize_narrow` are 120 by 40 because they were captured after the terminal grew.
Those captures are retired only when a reference binary produces a replacement with a new baseline id.

## Historical visual-baseline

The local repository has no `visual-baseline` tag.
`git ls-remote --tags origin` returned no tags.
The GitHub release `visual-baseline` was not found.
Historical parity is not claimed. The tag is not created.

## Simulated backends

TablePro, Holla, and Jackin simulate their services.
A passing journey does not prove PostgreSQL, Docker, a credential provider, a daemon, or a host export.

## Pre-refactor protection

These observations are not permission to change product behaviour.

The suspend API gap stays open.
`X02.suspend.suspend` is BLOCKED because tuiscotti-tui-test 0.2.1 `Pty::signal` cannot deliver TSTP or CONT.
`App::key("ctrl+z")` left showcase running.
The row is not `NOT_APPLICABLE`.

The functional harness does not emit coverage receipts.
`tests/functional/support.rs` writes session evidence under `target/functional/` and does not write `target/functional/receipts`.
`scripts/check-refactor-coverage.py` requires a receipt for every PASS row.
A clean run without hand-built receipt files fails `missing-receipt`.

The Holla secret prompt has no quiet frame.
`H03.input.prompts` is PASS and has no baseline, because the prompt never sits still.
Do not invent a baseline.

The CI generator limits stay as recorded in `C01`.
Generated CI runs `mbx nextest run --workspace --no-tests fail --profile functional` and does not pass `--locked`.
The inventory script is not a required CI step.
Branch protection on this branch returned HTTP 404.
macOS and Windows are unclaimed.
