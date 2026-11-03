# Fixed controls showcase page is not in the sidebar (2026-11-03)

Source: P9.3, step B4-0 (`Control::Fixed`, BX10 library part, BX17).

## The page is hidden

The page `fixedcontrols` sits in `HIDDEN_ENTRIES` (`src/bin/showcase/app.rs`),
beside `terminalpane`. A `NAV_ENTRIES` row changes the `Components 54` count
and the sidebar of every showcase baseline, and approved baselines must not
change.

Cost: the page opens only with `--page fixedcontrols`, and the `/` filter
does not find it. It has one baseline, `functional/showcase_page_fixedcontrols`.
The three key tests of the page (`showcase_fixed_controls_*`) end without a
baseline, as test plan v6 section 5.2 allows.

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
`DataTable` in the step that first needs it.

## No coverage declaration file

AGENTS.md names `src/bin/showcase/coverage.rs` for the coverage declaration.
The file does not exist in this tree (the catalog and coverage source scan was
dropped with the old test estate, see the migration table in
[functional testing](../functional-testing.md)). The new public API has its
lines in `src/catalog.rs` only.
