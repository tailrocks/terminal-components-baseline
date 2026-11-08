# Refactor test readiness

Reviewed parent: `779f3794068c47a941e438c398ef4a97ff3bb6ee` on `test/pre-refactor-protection`.

This review did not write the functional cases. It copied `result`, `disposition_reason`, and `execution_commit` from `docs/testing/cases/*.json` onto the 2223 rows in `docs/testing/refactor-coverage.json`. Every other inventory field is unchanged, including `baseline_paths`, `checkpoint_ids`, `case_id`, `kind`, the summary, `current_tests`, and `current_baselines`. The row count and `case_id` sequence are unchanged.

`C01.ci` and `H01.memory.failure` are case-file rows. They are not inventory rows, and this review did not add them.

## Sync counts

| Result | Rows |
| --- | ---: |
| PASS | 1838 |
| NOT_APPLICABLE | 384 |
| BLOCKED | 1 |

The one BLOCKED row is `X02.suspend.suspend`. `X02.suspend.route` is PASS. Every `NOT_APPLICABLE` row has a non-empty `disposition_reason` from its case file.

## Gate without receipts

```text
/usr/bin/python3 scripts/check-refactor-coverage.py --inventory docs/testing/refactor-coverage.json --nextest-list docs/testing/g00-nextest-list.txt --baselines tests/functional/baselines --dispositions docs/testing/baseline-dispositions.json --receipts target/probe-c03/empty-receipts
```

Exit 1:

```text
BLOCKED counts route=169 section=162 section-size=324 state=756 transition=232 option=538 scenario=42
source-agreement showcase_routes=54 option_keys=538 holla_scenarios=34 jackin_scenarios=8
missing-receipt: PASS row has no receipt: opt.shell.color, opt.shell.overrides, opt.shell.glyphs, opt.shell.strings, opt.shell.motion, opt.shell.focus_on, opt.shell.double_click_ms, opt.shell.double_click_slop and 1830 more
BLOCKED: 1 required rows have result BLOCKED
```

`target/probe-c03/empty-receipts` was missing. The run reported source agreement, `missing-receipt` for the PASS rows, and BLOCKED for the one remaining row. It did not report `orphan-baseline`, `missing-baseline`, `missing-test`, or `not-applicable-without-reason`.

## Gate with generated receipts

One JSON file per PASS `case_id` was written under `target/probe-c03/receipts/`. Each object has `case_id` set to that id. Where the case file has an `acceptance_receipt` object, those fields were copied too (1686 of 1838). No receipt was written for `X02.suspend.suspend` or for any `NOT_APPLICABLE` row. The files stay untracked under `target/`. They are not a substitute for the harness writing receipts.

```text
/usr/bin/python3 scripts/check-refactor-coverage.py --inventory docs/testing/refactor-coverage.json --nextest-list docs/testing/g00-nextest-list.txt --baselines tests/functional/baselines --dispositions docs/testing/baseline-dispositions.json --receipts target/probe-c03/receipts
```

Exit 1:

```text
BLOCKED counts route=169 section=162 section-size=324 state=756 transition=232 option=538 scenario=42
source-agreement showcase_routes=54 option_keys=538 holla_scenarios=34 jackin_scenarios=8
BLOCKED: 1 required rows have result BLOCKED
```

`scripts/check-refactor-coverage.py` treats a receipt as cover when the file's `case_id` matches the row (`receipt_covers`). A later clean run without `target/probe-c03/receipts` fails `missing-receipt` again, as the first gate did.

## Blockers

These items stay open. They are not passes, and they are not permission to change product behaviour.

- `X02.suspend.suspend` stays BLOCKED. tuiscotti-tui-test 0.2.1 `Pty::signal` cannot deliver TSTP or CONT (`Cargo.toml` pins `tuiscotti-tui-test` at `=0.2.1`). `App::key("ctrl+z")` left showcase running. The case records that a following `]` still opened Chrome and that `finish` exited 0. Do not mark the row `NOT_APPLICABLE`. Its `disposition_reason` is the case file's API explanation, not "Not executed yet."
- `tests/functional/support.rs` does not write `target/functional/receipts`. `evidence_directory` writes per-run session evidence under `target/functional/<name>-<pid>-<nanos>-<seq>/` (`binary.json`, `session.json`, and screen files). The second gate run used receipts generated from case ids. A later clean run without those files fails `missing-receipt`. That is a harness gap.
- `H03.input.prompts` is PASS with no quiet baseline because the secret prompt never sits still. The case says no baseline file is accepted, and the inventory `baseline_paths` stay empty. Do not invent a baseline.
- Generated CI, recorded in `C01`, runs `mbx nextest run --workspace --no-tests fail --profile functional` and does not pass `--locked`. That command is the functional step in `.github/workflows/ci.yml` and `.github/workflows/suites.yml`. The inventory script is not a required CI step: `.github/` does not reference `scripts/check-refactor-coverage.py`. Branch protection on this branch returned HTTP 404, as recorded by `C01.ci`. macOS and Windows are unclaimed. `.velnor/config.toml` sets `runner_label` to `ubuntu-26.04` only.
- `C01.ci` and `H01.memory.failure` are case-file rows outside the 2223-row inventory. `C01.ci` is PASS. `H01.memory.failure` is `NOT_APPLICABLE` with a non-empty reason. Neither was added.
- Contrast defects already recorded by `X03.color` stay product behavior. The `X03.color.matrix` case says contrast defects are recorded and left in `src`, including disabled primary below 4.5:1. Do not change the theme.
- Simulated backends stay inside their current scope. TablePro, Holla, and Jackin simulate their services. A passing journey does not prove PostgreSQL, Docker, a credential provider, a daemon, or a host export.
