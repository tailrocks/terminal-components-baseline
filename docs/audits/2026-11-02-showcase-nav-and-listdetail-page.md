# Terminal pane sidebar entry and a ListDetail showcase page (2026-11-02)

Source: the P9.2 review in `docs/specs/jackin-console-new-screens-review.md`
(deviations 1, 4, 9 and 10).

## Terminal pane page is not in the sidebar

The page `terminalpane` sits in `HIDDEN_ENTRIES` (`src/bin/showcase/app.rs`).
A `NAV_ENTRIES` row changes the `Components 54` count and the sidebar of
every showcase baseline, and existing baselines must not change.

Cost: the page is reachable only with `--page terminalpane`, and the `/`
filter finds the old `Terminal` page only.

Next step: at the next intended showcase re-baseline, move the entry to
`NAV_ENTRIES` (group `Text & code`, after `Terminal`), delete
`HIDDEN_ENTRIES`, and accept every showcase baseline that shows the
sidebar in the same change.

## ListDetail has no showcase page

Update 2026-11-05: the hidden page `listdetail` exists; see
[2026-11-05-core-backlog.md](2026-11-05-core-backlog.md). The text below is the
original record.

`ListDetail` is a generic list and detail block (`widgets::list_detail`;
the jackin Doctor and Roles screens use it). Its catalog lines sit in the
`ALLOWLIST` of `src/catalog.rs` with the reason `PREVIEW`. The reason text says "approved baselines", which becomes true
only when the eight P9.2 baselines are accepted.

Next step: add a showcase page `listdetail` (group `Screens`) with demo
rows that cover a long binding summary, a failing row, a faint row, a
narrow width (drawer mode) and the empty state, then delete the
`ListDetail`, `ListRow`, `Line`, `Action`, `Frame` and `Event` allowlist
lines. The page also shows the column rule: the chip column is reserved
and the right column truncates first. The library already has
`ui::leave::unsaved_dialog`; the jackin crate keeps its own
`screens::modals::unsaved_dialog` because the approved baselines show its
text. Unifying the two is a deliberate visual change that needs a
re-approval.

## Recorded, not changed

- Docker hint position: the role editor puts the hint after the label with
  one space, and the workspace editor aligns it in a column. Not blocking.
- Cockpit risk lines: `Props` do not wrap, so the cockpit info dialog cuts
  long lines while the confirm dialog uses a label column. The four facts
  match D24. Not blocking.
