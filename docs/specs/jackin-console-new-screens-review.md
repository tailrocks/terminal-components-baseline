# Review: the new jackin console screens (P9.2, D27 gate)

Reviewer: `ux-specialist`. Head `0980024` on `wt/P9.2`. Contract: `docs/specs/jackin-console-new-screens.md` and the P9.2 spec decisions in `jackin/rewrite/decisions.tsv`. Decisions D24, D26, D27 and D28 apply.

Method. I read the newest `sample.tui.json` of each of the 8 ids (by `captured_at_ms`), rebuilt `jackin` and `showcase` at the head, and drove every scenario in a 120x40 pty with a terminal emulator (and 80x24, 60x18, 100x12 for crashes). Paused-clock frames come from the artifacts. Interactive states (pickers, dialogs, F5, save, remove, capture) come from the live runs. No baseline was accepted.

## Verdicts

| # | Baseline id | Verdict | Why |
| --- | --- | --- | --- |
| 1 | `jackin_role_editor` | CHANGES | R1: a staged manifest that fails the check can be previewed and saved. |
| 2 | `jackin_role_binding` | CHANGES | R2: the `reviewer` row loses its source chip. R3: the status text pushes the key hints out of the footer. |
| 3 | `jackin_docker_setting` | CHANGES | R4: the footer has no `i Risks` hint on the Docker row. |
| 4 | `jackin_docker_no_engine` | CHANGES | R4, same footer. |
| 5 | `jackin_doctor` | CHANGES | R5: the frame selects an `ok` row, so the failing detail and the fix are not in the baseline. |
| 6 | `showcase_page_terminalpane` | CHANGES | R6: the page panics at 80x24 and 120x20. The 120x40 frame itself is correct. |
| 7 | `showcase_terminal_replay` | ACCEPT | Row 0 reads `;224;84mtail` with no colour, `1 of 4 chunks`, all four modes `off`: the spec table. |
| 8 | `jackin_launch_openshell` | CHANGES | R7: the credentials block lists Claude accounts and clips its second line; the sandbox name breaks D07. |

R1, R5, R6 and R7 change what the user sees or whether the app survives. R2 to R4 change a baseline-visible footer or row.

## Required changes

**R1. Role editor saves a manifest that the check rejects (major, `role_editor.rs`).**
Expected (spec 2.3, 2.9): the check runs before the preview opens, and Save is unreachable with a finding. Found: type `Dockerfile.old`, press `Ctrl+S` without `F5`. The `Save role` preview opens with the Check panel still reading `Check passed`. Pressing Save writes the role. The Roles detail then shows `Dockerfile  Dockerfile.old` and `Check  passed`. The test passes only because it presses `F5` first (`jackin_role_editor_check_and_save`, step 4).
Fix: `Ctrl+S` and the `Save…` button send `CheckRole` first. Open the preview only on `Done(vec![])`. On findings, open the `Check failed · sentinel` dialog that already exists. After any edit that follows the last check, show the panel as stale (`Not checked since your last edit · F5`) instead of `Check passed`. Add a functional step that saves without `F5` and expects the dialog. The final frame of the existing test should stay byte-identical, so re-capture and compare before accepting.
Same screen, minor: the add-entry form uses the placeholder `SENTINEL_NOTE` (`role_editor.rs:744`). It is a demo word in the library and the name of an existing entry. Use a neutral placeholder such as `EXAMPLE_NAME`. Not in a baseline frame.

**R2. Role list drops the source chip for bound roles (minor, shared `ListDetail` row).**
Frame row 5: `✓ reviewer            Codex · Experiments` has no `git` chip, while `the-architect` and `sentinel` show theirs. The column disappears when the binding text is long. Failing roles show `failed` and `broken` in the chip column, which hides their source. Fix in the shared row layout, not per screen: reserve the chip column and truncate the binding summary first (`Codex · Experim…`). Show the check state through the mark only, or keep the source chip and add the word elsewhere. At 80x24 the chip is present, so this is a width rule.

**R3. Status text hides the key hints (minor, role binding footer).**
Final frame footer: `↑↓ Move  Esc Back  …` plus an 83-character status. `Enter Pick`, `d Clear` and `u Undo` are gone, on the one frame that demonstrates clear and undo. Shorten the status to `Cleared the Codex binding of sentinel · u undoes`. The row already says `uses Codex · Primary ★ provider default`, so the longer clause adds nothing. Re-capture and check that the hints fit.

**R4. Docker row hint (minor, workspace editor, baselines 3 and 4).**
Spec 4.4 lists `i` as the risks key. The role editor shows `Space Toggle  i Risks  F5 Check ...` on the same row. The workspace editor shows `Space Toggle  Tab Next  [ ] Switch tab ...` and the row notice says `press i to read them`. Two places give two answers for one interaction. Add `i Risks` to the hint of the focused Docker row in the shared code that the role editor uses. Both frames change by one hint, as intended. Only when `surfaces.docker` is `Some`, so existing baselines stay as they are.

**R5. Doctor baseline frame (minor, test and frame).**
The frame header says `degraded · 1 failing`, but the detail pane shows `OpenShell binary  ok` with no Notice. The new part of this screen is the failing detail: `Status fail`, `Found`, `Expected` and the `!` problem plus fix line. Select `Gateway health` before `visual("functional/jackin_doctor")`. I saw that layout live for `Engine socket` and it is right (`! /var/run/docker.sock not found` / `Start Docker or OrbStack, then press r.`).

**R6. Terminal pane page panics on small terminals (major, `terminal_pane.rs`).**
`showcase --page terminalpane` at 80x24 aborts: `index outside of buffer: the area is Rect { x: 0, y: 0, width: 80, height: 24 } but index is (21, 24)`. Same at 120x20. Reproduced three times, also by resizing a running page from 120x40 to 80x24. 60x18, 40x12 and 100x30 do not panic. Cause: `render_playground` writes the control row at `frame.bottom() + 1` and the echo lines at `+2` and `+3` without clamping to `area.bottom()`. Spec 6.4 requires `Narrow: controls wrap`. Fix: clamp or skip rows that fall outside `area`, and keep the same for a pane of the `80x24` size option on a short terminal. Add a regression test that opens the page at 80x24 and 120x20 and fails on the unfixed code (AGENTS.md, Proof). The 120x40 baseline frame is unaffected.

**R7. OpenShell launch scenario data (minor, `sim/launch.rs` and `sim/mod.rs`, baseline 8).**
Frame rows 16 and 17: `credentials  5 accounts · Codex · Primary (1Password) · Claude · Personal · Claude …` and `... quota readable · prov…`. Two defects. A Codex-only launch lists Claude accounts, against D26: only accounts that an agent uses reach the OpenShell provider. And the second line ends in a clipped `prov…` inside an approved baseline. Give `openshell()` a credentials summary of the Codex account only, short enough to fit. Also the sandbox chip `jackin-payments-platform-b5df` (29 characters) breaks D07, which allows 19 (`jackin-<slug7>-<suffix4>`, here `jackin-payment-b5df`). The test asserts only `jackin-`, so tighten it to the D07 shape.

## Deviations

| # | Deviation | Verdict | Condition |
| --- | --- | --- | --- |
| 1 | Terminal pane page not in the sidebar (`HIDDEN_ENTRIES`) | ACCEPT | See below. |
| 2 | Play all skips the fullscreen leave chunk | ACCEPT | Count the leave chunk. |
| 3 | One session per option set | ACCEPT | Not verified in depth. |
| 4 | Docker hint rule (`Off by default`, `On · gateway · endpoint`) | ACCEPT | R4 for the missing `i` hint. |
| 5 | Chrome wording (`inside/entering the Construct`) | ACCEPT | Amend the spec text. |
| 6 | Added fields | ACCEPT | Listed below. |
| 7 | Roles meta `5 roles · 2 !` (spec said `1 !`) | ACCEPT | None. |
| 8 | `Add role…` answers `Not in this preview` | ACCEPT | Follow-up task. |
| 9 | Wrapped risk lines in the cockpit info dialog | ACCEPT | See below. |
| 10 | `ALLOWLIST` entries in `catalog.rs` | ACCEPT | Follow-up for `ListDetail`. |

The brief lists ten distinct items, not twelve. I judged the ten.

1. **Hidden page.** Accept because adding a row changes `Components 54` and the sidebar of every showcase baseline, and AGENTS.md forbids changing an existing baseline. The cost is real: the page is reachable only with `--page terminalpane`, and `/ Terminal` finds only the old `Terminal` page. Record it in `docs/audits` and move the entry to `NAV_ENTRIES` in the next intended showcase re-baseline. Spec 6.1 asked for a `NAV_ENTRIES` row, so the spec needs the same note.
2. **Play all.** Accept. Spec 6.5 step 3 expects `alt screen on` after Play all, which the fixture's own `ESC[?1049l` would contradict, so the spec was inconsistent and the builder's choice is right. The counter is wrong, though: after Play all it reads `3 of 3 chunks` with `alt screen on`, and one more Step flips `alt screen` to `off` with the counter unchanged. Count the leave chunk (`3 of 4`, then `4 of 4`). No baseline uses the `fullscreen` fixture, so none changes.
3. **One session per option set.** I read this as "an option change rebuilds the terminal state". That is consistent with spec 6.2 (size changes call `resize`) and Reset. I did not test a size change with content on screen.
4. **Docker hint.** The values are right. The role editor puts the hint after the label with one space (`Docker for every launch of this role Off by default · ...`), while the workspace editor aligns it in a column. Minor, not blocking.
5. **Chrome wording.** Accept. `Construct` is the base image name in `CONTEXT.md` and the baseline's brand wording; it is not a Docker or Capsule word. The strip is drawn by the shell (`app.rs:1919`) and I saw no Docker or Capsule word on any new screen. The functional test skips line 0 to avoid it, while spec 7.3 says "none of Construct". Amend 7.3 to say so and name the line the test skips.
6. **Added fields.** Accepted: the `scrollback` row and `PgUp PgDn Scrollback` hint on the terminal page (closes Q9), the `Gateway` prop in the Doctor detail, the `Hooks` and `Check` rows in the role detail.
7. **Meta count.** `data-eng` and `writer` both fail, so `2 !` is correct and the spec figure was a miscount.
8. **Add role….** Spec section 1.3 has no `Effect` for it, so the builder had nothing to call; that gap is mine (Q10 only says "registers an existing directory or git URL"). The row and the empty-state action lead to the status `Not in this preview · register a role with jackin role add`. Accept for the baseline. Open a follow-up before P9.4: `Effect::AddRole` plus a `FormDialog` (path or git URL), because in jackin this is a dead end on the screen that D26 names as the place to manage roles.
9. **Cockpit risk lines.** Accept: `Props` do not wrap and cut the text. They render as `Client key · The agent can read ...` while the confirm dialog and the `i` dialog use a label column, so the three views of the same four facts differ slightly. Not blocking. The four facts match D24 word for word (the last says `you remove them` for D24's `the operator removes them`). The cockpit one-line warning says only `the agent can read the engine key`. The decision row for D24 says the warning lists the key exposure and the egress bypass. Add the bypass to that line (not in a baseline, because `launch-openshell` has Docker off).
10. **ALLOWLIST.** Accept under the P9.1 decision (app-specific console blocks, reason `PREVIEW`). Two entries are not app-specific: `ListDetail` (a generic list and detail block, used by Doctor and Roles) and `unsaved_dialog`. Record a showcase page for `ListDetail` in `docs/audits`; the reason text says "approved baselines", which becomes true only when these eight are accepted.

## What passed

D24: the four facts are verbatim in the confirm dialog, the `i` view and the cockpit info dialog; the confirm opens with focus on `Cancel`, `Esc` cancels, turning off needs no confirm; the no-engine notice names `jackin gateway add`. D26: the order line reads flag, workspace, role, provider default, only account; the row text `none · uses Codex · Primary ★ provider default` follows it; the picker lists only same-provider accounts. OpenShell wording: no Docker or Capsule word on the Roles, Doctor, role editor or launch screens apart from the Docker feature rows and the Doctor engine-socket check, where Docker is the real thing. Stage order: Image build, OpenShell provider, Sandbox, Credentials, Upload, Hooks, Codex. Narrow: Roles and Doctor use the drawer; the role editor shows `1 error · F5`; no new jackin scenario crashed at 80x24, 60x18 or 100x12. Safety: Remove role confirms with `Cancel` focused; typed input stayed through validator errors and a failed check.

## Not verified

Help overlay rows and the `File` menu items for the new routes. Colour tones (warning versus error) on notices and the save preview prop; I read text and a few attributes only. `NO_COLOR` and `--glyphs ascii` renders. Roles empty state, Doctor never-run state, role editor loading, read-only git role, stale and not-writable save paths. `u` undo after a clear (the test covers it). The terminal page with `--color 16` and `none`, the capture hint on `Ctrl+G` after Reset, and the `split` and `midstream` start offsets other than the CSI one. The cockpit credential hold and failure dialogs on the new rail.

## Draft questions (not deviations)

None block this gate. Q3 matters for P9.4: the launch picker must show the account the launch uses, so it has to use the D26 order in jackin too.
