# Functional harness interface

Screen workers launch with `App::new` or `Launch::new`. Both use
`SessionMode::Candidate`. Ordinary `cargo nextest` runs stay in that mode
and do not accept baselines.

`SessionMode::Reference` is the capture path. It spawns only when the
binary's file name is `showcase`, `tablepro`, `holla`, or `jackin` and its
sha256 equals that entry in `docs/testing/reference-manifest.json`. Any
other file fails with `reference identity`. Candidate mode fails with
`reference binary` when the file's sha256 equals any recorded reference
digest. A normal `CARGO_BIN_EXE` build does not match those digests.

A successful launch writes `session.json` in its evidence directory under
`target/functional`. The file is not placed under
`tests/functional/baselines`. Its keys are `mode`, `reference_sha`,
`lockfile_sha256`, `binary_path`, `binary_sha256`, `args`, `cols`, `rows`,
`color`, `platform`, `history`, `motion`, `clock`, `TERM`,
`comparator_name`, and `comparator_rev`.

`App::visual` and `App::check_visual` pass an empty precondition.
`App::check_visual_with_precondition` records `precondition <text>` in the
action trace before the frozen comparison. The comparison reads the
baseline root and does not create or modify baseline files.

New baselines are produced only by `scripts/accept-baseline.py`, using a
sample captured from a Reference-mode session. The script shells out to
`tuiscotti accept` and writes the receipt at `--receipt`, which must sit
outside the baseline root. It refuses to run when
`FUNCTIONAL_CANDIDATE_GATE=1`.

## Public items

Constants: `SHOWCASE`, `TABLEPRO`, `HOLLA`, `JACKIN`.

Types: `Point`, `SessionMode`, `Launch`, `LaunchError`, `App`.

Functions: `try_launch`, `text`, `row_text`, `find_all`, `find`, `find_in`,
`cell_bg`, `crop`, `file_sha256`.

`Launch::new`.

`App::new`, `evidence_dir`, `child_running`, `screen`, `wait_screen`,
`stable_screen`, `visual`, `check_visual`, `check_visual_with_precondition`,
`try_sustained`, `wait_text`, `wait_absent`, `settle`, `point`, `key`,
`type_text`, `paste`, `advance_clock`, `move_to`, `down`, `up`, `drag_to`,
`click`, `click_text`, `wheel`, `resize`, `finish`, `expect_exited`,
`kill_child`.
