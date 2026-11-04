# Current state

This repository is the design reference implementation: a Cargo workspace
of two crates (edition 2024, Rust 1.99.1). `junie-tui` (v0.1.0) holds the
terminal component library and the showcase, tablepro and holla demo
applications. `jackin-console` (`apps/jackin`) holds the jackin demo: a
library of the jackin console screens plus the `jackin-preview` binary. It is
the exact authority for product appearance and interaction (see
[product](product.md)). The modular multi-crate workspace described in
[architecture](architecture.md) is the future refactoring state; it
does not exist in this tree yet.

## Structure

| Path | Owns |
| --- | --- |
| `src/lib.rs` | Library root (`junie_tui`); public module surface. |
| `src/core/` | Foundational types: IDs, events, intents, validation, diagnostics. |
| `src/shell/` | Application shell behaviour shared by every app: focus routing, hit testing, layers, modals, hints, help, quit guards. |
| `src/ui/` | Rendering context, overlay layer (`RenderCtx::overlay`, `flush_overlays`, `begin_modal`, `modal_backdrop`), paint primitives. |
| `src/widgets/` | The reusable component families (lists, trees, tables, fields, dialogs, menus, pickers, editors, scroll regions, splits). |
| `src/bin/showcase/` | Showcase: the executable component catalog (54 sidebar pages, plus the hidden pages `terminalpane`, `fixedcontrols` and `terminalviewport`). Thin consumer: pages add wiring, demo data and option tables only. |
| `src/bin/tablepro/` | TablePro: simulated database workbench (connections, explorer, query tabs, results, history). |
| `src/bin/holla/` | Holla: simulated launcher/finder, files, disk cleanup, services. Backends are simulated; tests assert the next in-app consequence. |
| `src/terminal.rs`, `src/terminal/viewport.rs`, `src/terminal/forward.rs` | Terminal pane block (feature `terminal`): `TerminalState`, `TermView`, `TermEvent`, `TerminalView`, `TerminalViewport`, `ForwardPolicy`, `encode_key`. The state renders a `termpane` grid fed from a byte stream, returns the events the stream raised (replies, title, clipboard, bell, notification, cwd, scrollback cleared) and does no I/O. `TerminalViewport` owns a state and is the terminal counterpart of `TextViewport` behind `widgets::viewport_ops::ViewportOps`: the scroll position is a row count from the live tail, kept on the same text while output arrives, rows are numbered from the first row ever pushed into scrollback so a selection survives output, the alternate screen has no history and a new screen or a cleared scrollback drops the selection, a copy joins soft-wrapped rows and trims blanks only at the end of a logical line, and the last column of the area is always the scroll track (`TerminalViewport::grid_size` gives the program every row and every other column, so its width never changes when history first appears; there is no bottom bar). A pane scrolled back fades its first and last rows like every scroll view; the live tail does not. `TerminalState::focus_events` and `mouse_reporting` read the modes the program turned on, `clear_scrollback` (on the state and on the viewport) drops the history and keeps the screen, and `ViewportOps::draws_track` tells a host that the view draws its own scroll track. `ForwardPolicy` is the pure byte filter for a pane: no key sends `Ctrl+D` or `Ctrl+Q`, a paste loses its control bytes (an `ESC` in it cannot close the bracket early), a paste with newlines into a program without bracketed paste is held (`Paste::NeedsConfirm`) until the host confirms (`ForwardPolicy::confirmed`), and a paste over `paste_max_bytes` (64 KiB) is refused (`Paste::TooLarge`). |
| `apps/jackin/src/` | The `jackin-console` library (`jackin_console`): the jackin screens (manager, editor, accounts, usage, settings, cockpit, capsule, prelude, roles, doctor and the modals), the `domain` types, `rain` and the other jackin-only modules, over the plain-data `world::World` and the `backend::Backend` and `backend::OnePassword` interfaces. The Capsule reaches each pane only through `panes::PaneBody` and `junie_tui::widgets::viewport_ops::ViewportOps`; `Backend::spawn_pane` and `Backend::initial_panes` build the bodies. `panes::TerminalPane` is the library's terminal body (no `demo` needed): a `PaneLink` (`send`, `set_size`, `drain(now_ms, budget)`) opened lazily by a `LinkSource` at the first size that fits (20 columns by 4 rows, the track column not counted), with the seven link states `LinkStatus::{Pending, Attaching, Attached(Lease), Detached(DetachCause), Exited, Failed}`, the replay window after an attach (`PaneTiming`: closes at 800 ms, or after 600 ms once the output was quiet for 150 ms; the pane shows a placeholder and sends no reply meanwhile), focus reports (`ESC [ I` and `ESC [ O` when the program asked for them) and attention (a bell or notification while the pane is not the focused pane of the active tab marks it `Blocked`; exit code 0 is `Done`, another code `Blocked`). `PanePolicy` (in `AppOptions.panes`; the preview sets `ClipboardWrite::Deny`) decides what the program may do: the pure rule `panes::clipboard::decide` takes an OSC 52 write only from the focused agent pane within `gesture_window_ms` (5,000) of the operator's last forwarded key or click in it, up to `max_decoded_bytes` (100,000), and refuses reads and every shell-pane write; each refusal shows its status at most every `refusal_status_every_ms` (10,000) per pane. The Capsule gives the policy to every daemon (`Daemon::set_policy`), tells each pane its size after every layout change and its focus after every event (`CapsuleScreen::sync_focus`, also when a menu, palette or dialog opens or closes), draws the pane's `face` (the grid, the dimmed grid, or an empty state) and its one-row `note`, drains the pane's `PaneEffect`s (a status, or a copy) on its tick, asks before closing a pane only when `close_needs_confirm()` (a pane with a live program), confirms a multi-line paste into a pane without paste protection, and shows the program's title (sanitised, 64 columns) as a `Title` row of the container info. `Go::Attach` builds the daemon from `Backend::initial_panes` when an instance has none. |
| `apps/jackin/src/fixtures/`, `apps/jackin/src/sim/` | `fixtures::Scenario`, `Surfaces::demo` and `Vocabulary::demo` and the 17 fixture worlds (`fixtures::world_for`; `capsule-terminal` is the Capsule with seven tabs of terminal panes, one in each link state, over scripted `sim::link::SimLink`s; `openshell-fixed` is the one with `Surfaces.fixed` and the OpenShell words; `launch-checks` is the one whose backend refuses a role the workspace does not allow and asks the env values a role prompts for), the start table `fixtures::entry_for` and the one demo constructor `fixtures::app_for`, which calls `app::assemble`; the simulated `Backend` (it owns the launch plan and `sim::launch::SimLaunch`), `OnePassword`, launch content and the scripted session player (`sim::pane`: `ScriptPane`, `AgentProcess`, `SessionScript`, `Step`). All behind the `demo` feature so the jackin repository can render the same worlds. The library `App` takes a `World`, an `app::Entry` (first page, outro when framed) and `app::AppOptions` (motion, frame, theme, `ClipboardMode`); it has no `Scenario`. |
| `apps/jackin/src/bin/jackin-preview/` | The jackin demo binary: the command line over `fixtures` and `sim`. Binary name is `jackin-preview` (`cargo run --bin jackin-preview --features demo`). |
| `apps/jackin/tests/*.rs` | Library tests of the `jackin-console` data API (`World`, `Snapshot`, the time source, external messages), run by `cargo nextest run --all-targets`. They are not part of the CI `functional` suite. |
| `apps/jackin/tests/functional/` | The jackin journeys and their approved baselines (`baselines/functional/jackin_*.tui.json`), on the shared harness `tests/functional/support.rs`. |
| `tests/functional/` | The root crate's test estate: live PTY functional/integration tests plus the frozen visual gate. See [functional testing](functional-testing.md). |
| `.velnor/config.toml` | Source of truth for generated CI (`.github/` is generated, never hand-edited). See [CI](ci.md). |

## Cargo features

| Feature | Default | Adds |
| --- | --- | --- |
| `cli` | yes | `clap`, the `cli` module and `Motion` as a `clap::ValueEnum`. The four binaries require it; `jackin-console` enables it on its `junie-tui` dependency. |
| `terminal` | yes | `termpane` and the `terminal` module. |

`jackin-console` features (none is a default):

| Feature | Adds |
| --- | --- |
| `demo` | `clap`, `junie-tui/cli`, the `fixtures` and `sim` modules, the `jackin-preview` binary and the `functional` test target. A consumer that supplies its own `World` and `Backend` leaves it off. |
| `product` | Nothing. It lets the generated CI select the product shape (`--no-default-features --features product`); no code may test it. |

`tests/product_manifest.rs` fixes this shape, and
`tests/library_has_no_demo.rs` keeps demo names out of the library
outside `fixtures/`, `sim/` and `bin/`.

## jackin-console data hooks

A consumer changes content, never structure, through `World` fields and
`Surfaces` (`domain::surfaces`). `Surfaces` has no library `Default`:
`WorldParts::new` takes it, `fixtures::Surfaces::demo(scenario)` is the demo
value (so the approved baselines do not move) and
`Surfaces::openshell(LiveFacts { version, config_path })` is the live value. A new field
is a compile error in both. The host adds its own data with the builders
`.roles(..)`, `.docker(..)`, `.doctor(..)` and `.about(..)`, never text.

| Hook | Default | What it changes |
| --- | --- | --- |
| `World::new(WorldParts)` | | Builds a `World` without a struct literal; `WorldParts::new` takes the clock, home, cwd, `OnePassword`, `Backend` and the `Surfaces`; later fields get defaults. |
| `World.time` | `TimeSource::Sim` | `Wall(WallSource)` follows the host clock and UTC offset in `World::tick`. |
| `World::apply(Snapshot)` | | Replaces workspaces, roles, instances, global config, accounts, surfaces and GitHub repos; keeps panes, jobs, clock and filesystem. |
| `Effect`, `Msg`, `Backend::perform`, `settle`, `due` | | A save effect carries the id (`None` for a new workspace), the `base` the screen loaded and the record to store (`SaveWorkspace`, `SaveGlobal`, `SaveAccounts`); `DeleteWorkspace`, `Restore` and `TrustRole` carry a key. After an effect no screen writes the `World`: a successful reply carries the `Snapshot` and `World::apply` installs it (`WorkspaceSaved`, `GlobalSaved`, `AccountsSaved`, `WorkspaceDeleted`, `Restored`, `RoleLoaded`, `RoleTrusted`, `Stopped`, `Purged`, `Refreshed`), a failed reply carries the error text and the screen keeps its pending copy and shows the text. `AccountRefreshed` carries a `Refresh` and `AccountValidated` a `Validation`, which `World::absorb` writes into the account. `Backend::perform` answers with `Completion::ready(after_ms, msg)` or `Completion::settle(after_ms)`; the second makes the world call `Backend::settle(effect, world)` when the reply falls due. A reply with no delay is delivered after the input that started it. `Backend::due(world)` lets a backend raise a message on its own (the simulated second client). The simulator (`sim::SimBackend`) owns the workspace id counter, the first-refresh failure, the broker outage, the heartbeat of `Instance.last_seen_secs` (answered with `Effect::Refresh`) and the takeover timer; `sim::snapshot(&World)` builds its replies. |
| `App::with_external`, `Remote` | no receiver | Lets another thread send `Remote::Msg` or `Remote::Reloaded` to the app. |
| `Backend::spawnable`, `Backend::pane_timing` | `Ok(())`, `PaneTiming::default()` | `spawnable(&World, &Instance, &PaneKind)` answers whether a pane may start; an `Err(Reason)` disables the row of the new-tab picker with the reason as its detail. `pane_timing()` is the replay window a backend gives the `TerminalPane`s it builds. |
| `Surfaces.vocabulary` | `Vocabulary::demo()` (in `fixtures`) | Names the project, sandbox, terminal, gateway, GitHub, quit, stop and purge strings (`Vocabulary::openshell()` is the OpenShell set, with the capsule word `Capsule`; `DaemonWords` and `LifecycleWords` group the gateway and the stop and purge texts). The capsule words are fields too: the About, paste and close labels and bodies, the empty-clipboard, unavailable, starting and cleared statuses, the export result with its optional reveal and open notes, the optional host-log row, the optional config paths and save errors of the editor and Settings (with the shared `Next` line), the pane-menu reason for an empty clipboard, the optional `Sandbox ID` and `Invocation ID` rows of the Debug info (a `None` hides the row), and the status after an attach. `Vocabulary.pane` (`PaneWords`) holds what a terminal pane says: the note of each link state, the placeholders, the paste confirm, the clipboard statuses and the `Title` label. In the OpenShell set the Manager names the multiplexer `Capsule`, like the Capsule screens. A `None` path or note hides the row or the clause. No screen holds a demo literal; `tests/library_has_no_demo.rs` scans for them. |
| `Surfaces.version`, `Surfaces.cockpit_version`, `Surfaces.about` | `0.9.2`, `0.6.4 · preview`, the four demo rows | The version shown in the capsule Debug info; the cockpit Debug info shows `cockpit_version` and falls back to `version` when it is `None` (the live value); the rows of the About dialog. |
| `Surfaces.config_path` | `None` | The host's config file (`LiveFacts.config_path`). It fills `{config}` in the `Vocabulary` templates of the editor and Settings Scope rows (`workspace_config`, `global_config`) and of the two save-failure `Error` rows (`save_error_workspace`, `save_error_global`); `World::config_file` shows it with `~`. The demo templates use `{home}` and `{name}` and ignore it. |
| `Surfaces.account_form` | four providers, three sources | Provider and source lists, labels, the preselected source and the 1Password note. `AccountForm::openshell()` marks the local folder `(recommended)`, preselects it and sets the note. |
| `Account.launch_blocked`, `Account.edit_blocked` | `None` | `launch_blocked` is a `LaunchBlock` (`UsageOnlySource`); a blocked account is not offered for launch and the picker shows the text of `Fixed.launch_usage_only` (`LaunchBlock::text`, `Usability::label`). An account the form cannot represent keeps its other actions. `edit_blocked` is an `Option<EditBlocked>` (`text` and a required `short`): the status shows `text`, the menu border shows `short`. |
| `Surfaces.agent_blocked` | `None` | Every agent except one is a disabled launch row with a reason (`AgentBlocked::openshell()`: Codex launches). |
| `Backend::launch_content()` | the Docker stages | `domain::launch::LaunchContent`: the stage list and the launch words. `LaunchContent::openshell()` is the seven OpenShell stages and words, with an empty build log and failure detail for the backend to fill; the simulated backend adds its sample log and detail (`sim::launch::openshell_content`). |
| `Backend::launch(&World, &LaunchTarget)`, `LaunchFeed`, `LaunchEvent` | the simulated launch (`sim::launch::SimLaunch`) | The launch of an instance. The cockpit polls the `LaunchFeed` a backend returns on every step and draws its events (`StageChanged`, `StageTimed`, `Activity`, `BuildLine`, `ContainerReady`, `CredentialsResolved`, `CredentialError`, `Refused(Refusal)`, `NeedsInput(Vec<EnvQuestion>)`, `Failed(FailedLaunch)`, `Ready(ReadyInstance)`); `Failed` and `Ready` carry the instance record the backend made, which the cockpit adds with `World::admit`. The run id comes from `LaunchFeed::run_id`. `Refused` carries four texts the backend wrote (`title`, `found`, `state`, `next_step`): the cockpit draws them as an error dialog and creates nothing, and closing it returns to the manager. `NeedsInput` carries the questions of the launch (`name`, `prompt`, optional `default`): the cockpit opens one form with a field for each and sends the answers to `answer_env`, and Esc ends the launch through `cancel`. `x` and the Cancel button work while a stage is `Blocked`, and `cancel` is called then too. `retry_credentials(op)` and `use_plain_text()` answer the credential dialog (`use_plain_text` returns the credentials line the backend wants shown, or `None`), `telemetry()` names the collector of the debug info (`None` shows no line), a stage shows a duration only when the feed sends `StageTimed` for it, and `cancel` returns the record of an ended launch (a `None` adds nothing). The cockpit writes no instance, no run id and no `OnePassword` session of its own, and creates no daemon: `Go::Attach` builds the daemon of an instance that arrives from the cockpit without one, with one tab for each `PaneRequest` of `Backend::initial_panes(&Instance)` and the body from `Backend::spawn_pane` (an agent request without an account gets the account the launch chose, because `Instance` does not carry it). The simulator keeps the launch plan (`Clean`, `FailNetwork`, `CredentialsLocked`, `BlockedSidecar`, chosen from the scenario and the `OnePassword` session) and, only for `SimBackend::guarded()`, the opening checks (`sim::launch::opening`: a workspace that does not allow the role refuses it, and the prompted `env` entries of the role manifest in `Surfaces.roles` are asked) and the `1Password locked` text; no library module names them. |
| `Surfaces.launch_role` | `Demo` | `Configured` resolves the launch role from the default or the only allowed role, else the picker rows are disabled with a reason. |
| `Surfaces.wizard` | `None` | (`WizardDefaults::openshell()`) One fixed destination, no edit step, no subfolders, and the name from the source folder. The P9.3 design calls this `Fixed.wizard`; the code keeps `Surfaces.wizard`, and B4 does not move it. |
| `Surfaces.fixed` | `None` | `Fixed::openshell()` (`domain::fixed`) makes the workspace editor's `Keep awake`, `Git pull before launch`, `On dirty exit`, working directory `Choose…`, Project row and Add row, and the Accounts tab toggle `Control::Fixed`, each with its reason text, and refuses unchecking the last allowed role. In Settings it fixes the Co-author and DCO boxes (one shared reason line), the global Mounts Add row, the Trust toggle (`o` shows the source in a sticky status), every Agents row (the launchable agent and the others have their own reason and the two foot notes are data) and the global Role overrides (Add row, `a`, `s`, and a scope select that lists only `global`). In the env form it dims the 1Password and host variable sources under a plain-values note, refuses the `p` key, adds a note under a key name with `TOKEN`, `SECRET`, `PASSWORD` or `KEY`, and refuses a plain value that starts with `op://` or `openshell:resolve:` (any letter case, after leading space) with the typed key and value kept. Each screen refusal is posted by `App::on_route` through `screens::post_refusal`, and a refusal raised inside a modal by `App::post_layer_refusal`. The env form stays open on Save so an error shows, in every scenario. This is a deliberate change to the demo, limited to the error path: before B4b the demo form closed at once on any error raised by the save step (an empty value, a duplicate key), with no message and the typed input lost, and a valid Save is identical in both builds. The value input is required in every scenario (label `Value for the key *`, `Required` under it on an empty Save). A rename to a key already in the scope is rejected before anything is staged, so Esc after the error leaves no change. The OpenShell words for the three texts that would promise more than jackin does are `Vocabulary` data and the demo keeps its own: `env_op_label` (`1Password item / field`), `trust_hint` (`Show source`) and `agent_no_credentials` (`no credentials handed to the sandbox`). From B4c on it also fixes the Accounts screen (the global enable: the `t` key, the Disable button and the menu item, short `no disabled state`; the Plain-text API key option and the `Enter plain text instead` button, with the reason as a note under the radio; Edit… on an account with `edit_blocked` is a fixed button), the file browser of the wizard (the Read-only box, with its reason drawn beside it by `FileBrowser`, and the Git URL validator, which always answers `Fixed.git_url`), the wizard destination step (both options dim, the radio disabled, the reason as a line, `Next` the initial focus) and the capsule reveal and open rows (palette detail and File menu border `prints path only`; `Fixed.reveal_open.text` has no input route yet and is for the export status of J5). `domain::openshell::env_edit_block(var)` is the one definition of the Edit… block of a host-variable account, and `surfaces::fixed` sets it on those accounts. The scenario `openshell-fixed` renders it. |

## Fixed fields of the OpenShell world

`domain::fixed_fields` names the class of every field of the records a
jackin world is made of. Fixed: the field holds one OpenShell value in every
jackin world; `workspace_fixed`, `global_fixed`, `account_fixed`,
`role_fixed`, `instance_fixed` and `surface_fixed` read it, and the tie
compares it. Data: the host supplies it; the extractors name it `field: _`.
Console: a `World` field the running console owns (`clock`, `backend`, `op`
and the like) that projection never produces; it is also `field: _`. Every
record is destructured without `..`, so a new field does not build until it
has a class. `world_fixed(&World)` and `snapshot_fixed(&Snapshot)` give the
same `FixedFields` view of a world and of a projection.

| Record (fields) | Fixed | Data |
| --- | --- | --- |
| `Account` (21) | `enabled` (`true`), `origin` (`Registered`) | the other 19 |
| `Workspace` (12) | `workdir` (`/sandbox/project`), `mounts` as one `MountShape`, `keep_awake` (`false`), `git_pull` (`false`), `dirty_policy` (`Ask`), `accounts` (below) | `id`, `name`, `roles`, `env`, `role_env`, `docker` |
| `AccountPolicy` (4) | `disabled_defaults` (empty), `role_preferred` (empty), `enabled` (through `enabled_is_every_account`: every account is effective) | `preferred` |
| `Mount` (8) | `destination`, `readonly`, `isolation` (`Clone`), `kind`, `scope`, the source being a host path, `drift`, `running_isolated` | the source path |
| `GlobalConfig` (7) | `coauthor_trailer`, `dco_signoff` (`false`), `mounts` (empty), `role_env` (empty), `agent_modes` (every agent but `Fixed.launchable` is `Ignore`), `trust` through `TrustRow.trusted` | `env`, `agent_modes[launchable]` |
| `TrustRow` (4) | `trusted` (`true`) | `source`, `kind`, `roles` |
| `RoleEntry` (8) | `trusted` (`true`: a source is trusted because the operator configured it, D28) | `namespace`, `name`, `source`, `in_registry`, `description`, `load_error`, `fetch_error` |
| `Instance` (18) | `workdir` (`/sandbox/project`) | the other 17 |
| `AccountRegistry` (2) | `accounts`, each as an `Account` | `revision` |
| `Surfaces` (13) | `vocabulary`, `fixed`, `account_form`, `agent_blocked`, `wizard`, `launch_role` | `roles`, `docker`, `doctor`, `version`, `cockpit_version`, `config_path`, `about` |
| `World` (22) | `workspaces`, `roles`, `instances`, `global`, `accounts`, `surfaces` (through the extractors above) | `github`; Console: `clock`, `time`, `arbiter`, `home`, `cwd`, `daemons`, `op`, `backend`, `fs`, `clipboard`, `jobs`, `daemon_health`, `last_refresh_secs`, `session_accounts`, `discovery_pending` |
| `Snapshot` (8) | `workspaces`, `roles`, `instances`, `global`, `accounts`, `surfaces` | `github`, `index_error` |

The normalizers of `domain::openshell` (`account`, `workspace`, `global`,
`role`, `instance`, `surfaces`) hold the one definition of each Fixed value;
the `openshell-fixed` world (`fixtures::surfaces::fixed`) and jackin's
projection both call them, and `Surfaces::openshell` takes its six Fixed
fields from `SurfaceFixed::openshell()`. `PROJECT_DIR` (`/sandbox/project`)
is the working directory of workspaces, instances, the mount destination and
the wizard. The `openshell-fixed` world holds no `Discovered` account:
jackin has no host discovery (D25, D26), so its five demo `Discovered`
accounts are `Registered` accounts (four named `host login`, one `ci-bot`,
provenance `ConfiguredSource`), and the world takes the live `account_form`,
`agent_blocked`, `launch_role` and `wizard`. No approved baseline moved.

Known residual hole: a field that is wrong in this table and wrong in the
expected literal of a test passes every check. The destructuring forces a
class for each field, and `tests/fixed_fields.rs` pins each Fixed value with
a literal, but nothing derives the class from a second source. Only a review
of this table closes the hole; `Fixed::openshell()` is the second source to
read it against, because each of its reasons names a Fixed control.

## Rules that hold today

- Behaviour lives in the shared library (`src/core`, `src/shell`,
  `src/ui`, `src/widgets`), never duplicated per screen or per app.
  Apps provide routes, pages and configuration only.
- Every widget follows the same conventions: builders, events,
  outcomes, one keyboard-focus gutter (`Theme::gutter_symbol`),
  selection distinct from focus, light-dismiss owned by the popup.
- Behaviour is overridable in layers: app/profile policy, then
  per-widget setting, then user key remapping.
- A control with one true value is `Control::Fixed(Reason)` (`core::refusal`): `Checkbox`, `Select`, `Button`, one `RadioGroup` option and one `ListItem` take `.fixed(reason)` or `.control(..)`. It stays in the focus ring, draws the disabled look with the `▎` gutter when focused, and refuses an edit: the value is untouched, the widget returns `Consumed` (`Select`, `Button`: `(Consumed, ..)`) and raises the reason into a thread-local slot (`RefCell<Option<Reason>>`). The call site reads the slot with `take_refusal()` (which clears it) and posts it with `Cx::refused(Reason)`, a neutral status of the reason text alone that lasts 5 s. `shell::handle` clears the slot at the start and at the end of each input, and redraws whenever an input changed the status line (text or tone), so a refusal by key paints at once. `Cx::sticky` posts a neutral status that does not expire and clears on the next key, paste or mouse press (`StatusLine::set_sticky_with`); it is for text a user must read or copy. A `Dialog` routes its mnemonics and its confirm key through `Button::activate`, and every finish (Enter in a prompt field, the typed acknowledgement) through `Button::refuse`, so a fixed action refuses on every route and `Dialog::hints` does not offer it. A fixed `ListItem` keeps its checked state through every selection path. `Reason` limits: text 60 columns, `short` 20 (`Reason::within_limits`); `ContextMenu::reason_room` is the room of the menu border. The showcase page `fixedcontrols` shows it. `Table` rows have no fixed state yet.
- The showcase demonstrates every library capability; the coverage
  gate is described in
  [shared components plan](architecture/shared-components-plan.md).

## Testing today

- `cargo nextest run --locked --test functional --profile functional`
  — 93 tests, 0 skipped: every app end-to-end (launch → operate →
  verify consequence → reopen → clean exit), component families in
  Showcase and in a real consumer, colour matrix
  (truecolor/256/16/mono/`NO_COLOR`), normal and small sizes.
- Every test but the key tests of a refusal page ends in
  `App::visual("<id>")` (see [functional testing](functional-testing.md)), a `FrozenStore` check
  against a committed baseline in `tests/functional/baselines/`.
  Baselines are approved only via the Tuiscotti CLI `accept` command.
- Full account of the deleted legacy estate and its replacement
  mapping: [functional testing](functional-testing.md).

## Known limits

- `TerminalState::scrolled_off` is exact while the history is below its
  limit (it follows the growth of `scrollback_len`). At the limit it sums
  the primary-screen scroll operations that start at row 0, so a delete-line
  at row 0 over-counts. `ESC[2J` discards the visible rows
  (`EraseDisplayPolicy::Discard`), so it changes neither value.
- `termpane` is a git dependency pinned to commit `0707acca` (termpane
  `main`, which holds `EraseDisplayPolicy`). It becomes `=0.1.1` once
  0.1.1 is published on crates.io; see [dependencies](dependencies.md).
- The crate is a monolith: library and four binaries in one package.
  Splitting it is the refactoring programme in
  [architecture](architecture.md); no part of that split has landed.
- Applications simulate their backends (PostgreSQL, Docker,
  filesystem). No real external effect is claimed or tested.
- The `visual-baseline` git tag is frozen historical evidence. It is
  never moved, and it is not a substitute for the current design.
