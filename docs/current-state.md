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
| `src/bin/showcase/` | Showcase: the executable component catalog (54 sidebar pages, plus the hidden pages `terminalpane` and `fixedcontrols`). Thin consumer: pages add wiring, demo data and option tables only. |
| `src/bin/tablepro/` | TablePro: simulated database workbench (connections, explorer, query tabs, results, history). |
| `src/bin/holla/` | Holla: simulated launcher/finder, files, disk cleanup, services. Backends are simulated; tests assert the next in-app consequence. |
| `src/terminal.rs` | Terminal pane block (feature `terminal`): `TerminalState`, `TerminalView`, `encode_key`. It renders a vt100 session from a byte stream and does no I/O. |
| `apps/jackin/src/` | The `jackin-console` library (`jackin_console`): the jackin screens (manager, editor, accounts, usage, settings, cockpit, capsule, prelude, roles, doctor and the modals), the `domain` types, `rain` and the other jackin-only modules, over the plain-data `world::World` and the `backend::Backend` and `backend::OnePassword` interfaces. |
| `apps/jackin/src/fixtures/`, `apps/jackin/src/sim/` | `fixtures::Scenario`, `Surfaces::demo` and `Vocabulary::demo` and the 15 fixture worlds (`fixtures::world_for`; `openshell-fixed` is the one with `Surfaces.fixed` and the OpenShell words), the start table `fixtures::entry_for` and the one demo constructor `fixtures::app_for`, which calls `app::assemble`; the simulated `Backend` (it owns the launch plan, `Backend::launch_plan`), `OnePassword` and launch content. All behind the `demo` feature so the jackin repository can render the same worlds. The library `App` takes a `World`, an `app::Entry` (first page, outro when framed) and `app::AppOptions` (motion, frame, theme, `ClipboardMode`); it has no `Scenario`. |
| `apps/jackin/src/bin/jackin-preview/` | The jackin demo binary: the command line over `fixtures` and `sim`. Binary name is `jackin-preview` (`cargo run --bin jackin-preview --features demo`). |
| `apps/jackin/tests/*.rs` | Library tests of the `jackin-console` data API (`World`, `Snapshot`, the time source, external messages), run by `cargo nextest run --all-targets`. They are not part of the CI `functional` suite. |
| `apps/jackin/tests/functional/` | The jackin journeys and their approved baselines (`baselines/functional/jackin_*.tui.json`), on the shared harness `tests/functional/support.rs`. |
| `tests/functional/` | The root crate's test estate: live PTY functional/integration tests plus the frozen visual gate. See [functional testing](functional-testing.md). |
| `.velnor/config.toml` | Source of truth for generated CI (`.github/` is generated, never hand-edited). See [CI](ci.md). |

## Cargo features

| Feature | Default | Adds |
| --- | --- | --- |
| `cli` | yes | `clap`, the `cli` module and `Motion` as a `clap::ValueEnum`. The four binaries require it; `jackin-console` enables it on its `junie-tui` dependency. |
| `terminal` | no | `vt100` and the `terminal` module. |

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
`Surfaces::openshell(LiveFacts { version })` is the live value. A new field
is a compile error in both. The host adds its own data with the builders
`.roles(..)`, `.docker(..)`, `.doctor(..)` and `.about(..)`, never text.

| Hook | Default | What it changes |
| --- | --- | --- |
| `World::new(WorldParts)` | | Builds a `World` without a struct literal; `WorldParts::new` takes the clock, home, cwd, `OnePassword`, `Backend` and the `Surfaces`; later fields get defaults. |
| `World.time` | `TimeSource::Sim` | `Wall(WallSource)` follows the host clock and UTC offset in `World::tick`. |
| `World::apply(Snapshot)` | | Replaces workspaces, roles, instances, global config, accounts, surfaces and GitHub repos; keeps panes, jobs, clock and filesystem. |
| `App::with_external`, `Remote` | no receiver | Lets another thread send `Remote::Msg` or `Remote::Reloaded` to the app. |
| `Surfaces.vocabulary` | `Vocabulary::demo()` (in `fixtures`) | Names the project, sandbox, terminal, gateway, GitHub, quit, stop and purge strings (`Vocabulary::openshell()` is the OpenShell set, with the capsule word `Capsule`; `DaemonWords` and `LifecycleWords` group the gateway and the stop and purge texts). The capsule words are fields too: the About, paste and close labels and bodies, the empty-clipboard, unavailable, starting and cleared statuses, the export result with its optional reveal and open notes, the optional host-log row, and the optional config paths and save errors of the editor and Settings. A `None` path or note hides the row or the clause. No screen holds a demo literal; `tests/library_has_no_demo.rs` scans for them. |
| `Surfaces.version`, `Surfaces.cockpit_version`, `Surfaces.about` | `0.9.2`, `0.6.4 · preview`, the four demo rows | The version shown in the capsule Debug info; the cockpit Debug info shows `cockpit_version` and falls back to `version` when it is `None` (the live value); the rows of the About dialog. |
| `Surfaces.account_form` | four providers, three sources | Provider and source lists, labels, the preselected source and the 1Password note. `AccountForm::openshell()` marks the local folder `(recommended)`, preselects it and sets the note. |
| `Account.launch_blocked`, `Account.edit_blocked` | `None` | `launch_blocked` is a `LaunchBlock` (`UsageOnlySource`); a blocked account is not offered for launch and the picker shows the text of `Fixed.launch_usage_only` (`LaunchBlock::text`, `Usability::label`). An account the form cannot represent keeps its other actions. `edit_blocked` is an `Option<EditBlocked>` (`text` and a required `short`): the status shows `text`, the menu border shows `short`. |
| `Surfaces.agent_blocked` | `None` | Every agent except one is a disabled launch row with a reason (`AgentBlocked::openshell()`: Codex launches). |
| `Backend::launch_content()` | the Docker stages | `domain::launch::LaunchContent`: the stage list and the launch words. `LaunchContent::openshell()` is the seven OpenShell stages and words, with an empty build log and failure detail for the backend to fill; the simulated backend adds its sample log and detail (`sim::launch::openshell_content`). |
| `Surfaces.launch_role` | `Demo` | `Configured` resolves the launch role from the default or the only allowed role, else the picker rows are disabled with a reason. |
| `Surfaces.wizard` | `None` | (`WizardDefaults::openshell()`) One fixed destination, no edit step, no subfolders, and the name from the source folder. The P9.3 design calls this `Fixed.wizard`; the code keeps `Surfaces.wizard`, and B4 does not move it. |
| `Surfaces.fixed` | `None` | `Fixed::openshell()` (`domain::fixed`) makes the workspace editor's `Keep awake`, `Git pull before launch`, `On dirty exit`, working directory `Choose…`, Project row and Add row, and the Accounts tab toggle `Control::Fixed`, each with its reason text, and refuses unchecking the last allowed role. In Settings it fixes the Co-author and DCO boxes (one shared reason line), the global Mounts Add row, the Trust toggle (`o` shows the source in a sticky status), every Agents row (the launchable agent and the others have their own reason and the two foot notes are data) and the global Role overrides (Add row, `a`, `s`, and a scope select that lists only `global`). In the env form it dims the 1Password and host variable sources under a plain-values note, refuses the `p` key, adds a note under a key name with `TOKEN`, `SECRET`, `PASSWORD` or `KEY`, and refuses a plain value that starts with `op://` or `openshell:resolve:` (any letter case, after leading space) with the typed key and value kept. Each screen refusal is posted by `App::on_route` through `screens::post_refusal`, and a refusal raised inside a modal by `App::post_layer_refusal`. The env form stays open on Save so an error shows, in every scenario. This is a deliberate change to the demo, limited to the error path: before B4b the demo form closed at once on any error raised by the save step (an empty value, a duplicate key), with no message and the typed input lost, and a valid Save is identical in both builds. The value input is required in every scenario (label `Value for the key *`, `Required` under it on an empty Save). A rename to a key already in the scope is rejected before anything is staged, so Esc after the error leaves no change. The OpenShell words for the three texts that would promise more than jackin does are `Vocabulary` data and the demo keeps its own: `env_op_label` (`1Password item / field`), `trust_hint` (`Show source`) and `agent_no_credentials` (`no credentials handed to the sandbox`). The scenario `openshell-fixed` renders it. |

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

- The crate is a monolith: library and four binaries in one package.
  Splitting it is the refactoring programme in
  [architecture](architecture.md); no part of that split has landed.
- Applications simulate their backends (PostgreSQL, Docker,
  filesystem). No real external effect is claimed or tested.
- The `visual-baseline` git tag is frozen historical evidence. It is
  never moved, and it is not a substitute for the current design.
