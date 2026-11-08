# Fixed controls showcase page is not in the sidebar (2026-11-03)

Source: P9.3, step B4-0 (`Control::Fixed`, BX10 library part, BX17).

## The page is hidden

The page `fixedcontrols` sits in `HIDDEN_ENTRIES` (`src/bin/showcase/app.rs`),
beside `terminalpane`. A `NAV_ENTRIES` row changes the `Components 54` count
and the sidebar of every showcase baseline, and approved baselines must not
change.

Cost: the page opens only with `--page fixedcontrols`, and the `/` filter
does not find it. It has two baselines, `functional/showcase_page_fixedcontrols`
and `functional/showcase_fixedcontrols_focused_refused`. The other key tests of
the page (`showcase_fixed_controls_*`) end without a baseline, as design v6
section 5.2 (last paragraph) allows.

Next step: at the next intended showcase re-baseline, move both hidden
entries to `NAV_ENTRIES` (group `Inputs & forms`, after `Checks & radios`),
delete `HIDDEN_ENTRIES`, and accept every showcase baseline that shows the
sidebar in the same change. This is the step recorded for `terminalpane` in
[2026-11-02-showcase-nav-and-listdetail-page.md](2026-11-02-showcase-nav-and-listdetail-page.md).

## Table rows have no fixed state

`ListItem` (the `ListBox` rows) and the shared row painter (`VisualState.fixed`,
`RowUi`) take the fixed state. `DataTable` rows (`disabled_rows`) do not. No
screen of the planned B4a to B4c steps names a `DataTable` row as a fixed
control; the jackin rows are custom rows on `RowUi`. Add the state to
`DataTable` in the step that first needs it. Until then a `DataTable` has no
fixed row: a screen that needs a row that refuses uses a `ListBox`/`RowUi` row.

## No coverage declaration file

AGENTS.md names `src/bin/showcase/coverage.rs` for the coverage declaration.
The file does not exist in this tree (the catalog and coverage source scan was
dropped with the old test estate, see the migration table in
[functional testing](../functional-testing.md)). The new public API has its
lines in `src/catalog.rs` only.

## B4-0 review findings recorded

- Page copy (UX F8): applied. The blurb reads `One true value: focusable, drawn
  disabled, refuses every edit` and the Disable reason reads `Cannot disable ·
  all stay on`. Both fixedcontrols baselines were unmerged and unapproved, so they
  were re-accepted; rows 3 and 17 are the only rows that changed in each. The
  focused-refused baseline (`showcase_fixedcontrols_focused_refused`) is the second
  baseline of the hidden page, and it moves with `showcase_page_fixedcontrols` at
  the hidden-page re-baseline above.
- Fixed ink contrast (UX F9): accepted under D27. A fixed control keeps the
  approved disabled look (2.2:1 for a checkbox, 2.0:1 for a select value, 1.8:1
  for a button). The compensation is the focus gutter at 10.9:1 and the inline
  reason on a checkbox, a select and a button. Reopen only with a deliberate,
  owner-approved visual change; a `text_muted` ink in the fixed branch of
  `theme.rs` would move no approved baseline.
- Whole-row dim (UX F10): a gate on B4a. B4-0 paints no data row. B4a may not paint
  a data row dim: a row that carries content (the Project row with the host path)
  keeps its baseline paint, and the UX review of B4a rejects a baseline in which
  the path or the value is dim. A row that refuses keys but paints normal needs a
  library change, and that change is part of B4a.
- The clean-quit dialog draws `!` and a destructive-style `Quit` button for what is
  now a detach (UX section 3): accepted under D27. The words are right; the chrome is
  the baseline dialog, and a leave guard may keep its warning style.
- The sticky status (`Cx::sticky`) is built and shown on the page through the
  option `Reason status = sticky`. No jackin screen uses it before B4b.
- The `Request::Sticky` arms in the jackin host (`apps/jackin/src/app.rs`), the holla
  host (`src/bin/holla/app.rs`) and the tablepro host (`src/bin/tablepro/app.rs`)
  have no caller yet, and no test can reach them. Only the showcase host arm is
  exercised, by `showcase_fixed_controls_sticky_reason_stays_until_the_next_key`.
- `InstanceStatus::label()` still returns `preserved · unpushed` for a caller that
  bypasses the vocabulary. Every screen goes through `Vocabulary::status_label`.
- `lifecycle.restored` (`Restored {id} · sandbox restarted`) is shadowed: the attach
  that follows Restore replaces the status in the same input, so no screen shows
  it. `status.purged` and `status.superseded` are unreachable: the roster hides a
  purged or superseded instance. None of the three has a screen test. Decide at
  J4 and J5 whether to delete the fields.
- The clock step is delivered as a paste, and `shell::handle` dismisses an error or
  sticky status on a paste before it parses the step. A functional test cannot
  advance the clock while a sticky status shows. If one needs it, parse the step
  before `status.on_input`. Done 2026-11-05: `shell::handle` takes a test request
  out of `StatusLine::on_input`.
