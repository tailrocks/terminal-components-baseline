# Core backlog round: hidden pages, decisions and what stays open (2026-11-05)

Source: the baseline backlog rows for the `junie-tui` library (bl-core).

## Two hidden showcase pages

The pages `listdetail` (List detail, group Layout) and `fixeddialogs` (Fixed
dialogs, group Overlays) sit in `HIDDEN_ENTRIES` (`src/bin/showcase/app.rs`),
beside `terminalpane`, `fixedcontrols` and `terminalviewport`. A `NAV_ENTRIES`
row changes the `Components 54` count and the sidebar of every showcase
baseline, and approved baselines must not change (D27).

Cost: both open only with `--page listdetail` or `--page fixeddialogs`, and the
`/` filter does not find them.

New baselines (first approvals, no existing baseline moved):
`showcase_page_listdetail`, `showcase_listdetail_drawer` and
`showcase_page_fixeddialogs`.

Next step: at the next intended showcase re-baseline, move the five hidden
entries to `NAV_ENTRIES` and accept every showcase baseline that shows the
sidebar in the same change, as recorded in
[2026-11-02-showcase-nav-and-listdetail-page.md](2026-11-02-showcase-nav-and-listdetail-page.md).

`ListDetail`, `ListRow`, `Line`, `Action`, `Frame` and `Event` left the
`ALLOWLIST` of `src/catalog.rs`: the `listdetail` page draws and drives all of
them. The States tab of both pages is empty, because neither widget has a
`Specimen` yet.

## Decisions

- `ViewportOps::draws_track` has a default (`false`, the behaviour before the
  method existed), so an existing implementor still compiles.
- `ViewportOps::adopt_scroll_policy(&mut self, &RenderCtx)` applies the
  app-wide horizontal and track-click policy to the state that handles input.
  `draw` runs on a shared reference and cannot keep it. The default does
  nothing. No host in this repository calls it: jackin sets no policy, so
  nothing differs there until it does.
- Dropping a `Shell` clears the thread-local clipboard hook it installed. The
  `clipboard` field is private: `Shell::set_clipboard` installs the new
  clipboard (the hook slot holds one hook, so installing replaces the old
  one), and `Shell::clipboard` reads the one in use.
- `TerminalState::scrolled_off` counts the rows that leave the top of the main
  screen even with a scrollback limit of 0, so a selection follows its text
  (`tests/terminal.rs` pinned `0` for that case, and now pins the count).
- The terminal grid maps strikethrough to `CROSSED_OUT`. Conceal and blink are
  not mapped: a program must not be able to hide output from the operator, and
  blink is a motion request that `Motion` governs.
- A clock-step request no longer clears an error or sticky status; the step is
  taken out of `StatusLine::on_input` as the fence is. This closes the open
  note in [2026-11-03-fixed-controls-showcase-page.md](2026-11-03-fixed-controls-showcase-page.md).

## Still open

- BLOCKED: visual. `FileBrowser::hints` ignores the focus it is given, so the
  footer offers `Enter Open  Space Choose` while the fixed Read-only box or a
  text field has focus. A fix (a box offers `Space Toggle` or nothing, a button
  its label, a field its edit hints) was written and passed, and it changed
  row 39 of two approved jackin baselines
  (`jackin_fixed_browser_read_only`, `jackin_fixed_browser_url`). It was
  reverted. Land it together with a deliberate re-approval of those two
  baselines, then add the hints test to `tests/fixed.rs` and a footer assertion
  to `showcase_fixeddialogs_browser_box_refuses_and_shows_its_reason`.

- Pointer timers (double-click interval, press flash, hover dwell) run on wall
  time. Freezing them broke 15 journeys (docs/P9.3-red-green.md, "What was
  tried and dropped"); a harness that paces the clock per gesture is a design
  change of its own.
- The assertion of `holla_first_use_idle_and_no_results` was rewritten in
  an earlier round and still needs an independent test-reviewer.
- Tuiscotti candidates are keyed by a digest that includes a capture time, so
  two runs of one journey give two digests for identical cells. Approve a
  candidate by the digest of the run that was reviewed.

## Rows that stay with the owner or with the jackin repository

Rows from the app and jackin backlogs that this repository cannot close in a
way that keeps every approved baseline. BLOCKED: visual rows name the baseline
that the change would move. JACKIN rows need a change in the jackin repository.

- BLOCKED: visual. B4c: the footer of the file browser follows the focus. Same
  change as the first open item above; moves `jackin_fixed_browser_read_only`
  and `jackin_fixed_browser_url` (row 39).
- BLOCKED: visual. B8b UX L1: the attaching hint text moves
  `jackin_pane_attaching`.
- BLOCKED: visual. B8b UX L2: the viewing note wording, the hidden cursor and
  the missing status move `jackin_pane_viewing`.
- BLOCKED: visual. B8b UX L3: the ink of the detached note moves
  `jackin_pane_detached`.
- BLOCKED: visual. B8b UX L4 and L5: the body text of the close-pane dialog
  (and, for the agent pane, a box that grows by a row) moves
  `jackin_close_pane_codex` and `jackin_close_pane_shell`.
- BLOCKED: visual. B8b UX L6: the label `Program title` moves
  `jackin_pane_info_title`.
- BLOCKED: visual. PR 21: the focus bar plus the gutter moves
  `jackin_role_binding`.
- BLOCKED: visual. The launch dialog takes the focus and is easy to miss. Any
  visible cue (an activity text behind the backdrop, a stage state) moves
  `jackin_launch_env_prompt`.
- BLOCKED: visual. The "no typed keys" note is a form field under the source
  radio. Hiding or rewording it moves `jackin_fixed_account_form`,
  `jackin_fixed_account_form_env`, `jackin_fixed_account_form_1password`,
  `jackin_fixed_account_form_pending`, `jackin_fixed_account_form_refused` and
  `jackin_fixed_account_edit_env`.
- BLOCKED: visual. The tab strip of the tablepro workbench syncs its busy
  spinner only when something rebuilds the strip, so a tab whose query has
  finished keeps its spinner until then. The approved `tablepro_history`
  baseline shows that stale spinner on `Query 1`; syncing the strip after the
  query finishes removes it and moves that baseline. The functional test
  now opens the history in the same input batch that runs the query, so the
  baseline no longer depends on how fast the host is.
- JACKIN. `PaneCommand::Detach` is built only by the jackin link. Sending it
  when the Capsule is left changes what an operator who returns sees (the panes
  would need a reattach), which is a design call for the jackin side.
- JACKIN. The stop hook of the console shell: `Shell::stop_handle` and
  `StopHandle` exist here, and the token in `ConsoleDeps` and its watcher are
  in `commands/console.rs` and `jackin-tui/src/console.rs` of the jackin
  repository.
- JACKIN. `LaunchEvent::Ready` needs an `Instance`. The fallback needs
  `AttachTarget` in `crates/jackin-session` to carry the summary.
