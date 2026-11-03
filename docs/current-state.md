# Current state

This repository is the design reference implementation: a Cargo workspace
of two crates (edition 2024, Rust 1.99.1). `junie-tui` (v0.1.0) holds the
terminal component library and the showcase, tablepro and holla demo
applications. `jackin-console` (`apps/jackin`) holds the jackin demo: a
library of the jackin console screens plus the `jackin` binary. It is
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
| `src/bin/showcase/` | Showcase: the executable component catalog (54 pages). Thin consumer: pages add wiring, demo data and option tables only. |
| `src/bin/tablepro/` | TablePro: simulated database workbench (connections, explorer, query tabs, results, history). |
| `src/bin/holla/` | Holla: simulated launcher/finder, files, disk cleanup, services. Backends are simulated; tests assert the next in-app consequence. |
| `src/terminal.rs` | Terminal pane block (feature `terminal`): `TerminalState`, `TerminalView`, `encode_key`. It renders a vt100 session from a byte stream and does no I/O. |
| `apps/jackin/src/` | The `jackin-console` library (`jackin_console`): the jackin screens (manager, editor, accounts, usage, settings, cockpit, capsule, prelude, roles, doctor and the modals), the `domain` types, `rain` and the other jackin-only modules, over the plain-data `world::World` and the `backend::Backend` and `backend::OnePassword` interfaces. |
| `apps/jackin/src/fixtures/`, `apps/jackin/src/sim/` | The 14 fixture worlds (`fixtures::world_for`) and the simulated `Backend`, `OnePassword` and launch content, in the library behind the `fixtures` feature so the jackin repository can render the same worlds. |
| `apps/jackin/src/bin/jackin/` | The jackin demo binary: the command line over `fixtures` and `sim`. Binary name is `jackin` (`cargo run --bin jackin`). |
| `apps/jackin/tests/*.rs` | Library tests of the `jackin-console` data API (`World`, `Snapshot`, the time source, external messages), run by `cargo nextest run --all-targets`. They are not part of the CI `functional` suite. |
| `apps/jackin/tests/functional/` | The jackin journeys and their approved baselines (`baselines/functional/jackin_*.tui.json`), on the shared harness `tests/functional/support.rs`. |
| `tests/functional/` | The only test estate: live PTY functional/integration tests plus the frozen visual gate. See [functional testing](functional-testing.md). |
| `.velnor/config.toml` | Source of truth for generated CI (`.github/` is generated, never hand-edited). See [CI](ci.md). |

## Cargo features

| Feature | Default | Adds |
| --- | --- | --- |
| `cli` | yes | `clap`, the `cli` module and `Motion` as a `clap::ValueEnum`. The four binaries require it; `jackin-console` enables it on its `junie-tui` dependency. |
| `terminal` | no | `vt100` and the `terminal` module. |

`jackin-console` has one feature, `fixtures` (default): the `fixtures`
and `sim` modules and the `jackin` binary. A consumer that supplies its
own `World` and `Backend` turns it off.

## jackin-console data hooks

A consumer changes content, never structure, through `World` fields and
`Surfaces` (`domain::surfaces`). Every default is the fixture behaviour,
so the approved baselines do not move.

| Hook | Default | What it changes |
| --- | --- | --- |
| `World::new(WorldParts)` | | Builds a `World` without a struct literal; later fields get defaults. |
| `World.time` | `TimeSource::Sim` | `Wall(WallSource)` follows the host clock and UTC offset in `World::tick`. |
| `World::apply(Snapshot)` | | Replaces workspaces, roles, instances, global config, accounts, surfaces and GitHub repos; keeps panes, jobs, clock and filesystem. |
| `App::with_external`, `Remote` | no receiver | Lets another thread send `Remote::Msg` or `Remote::Reloaded` to the app. |
| `Surfaces.vocabulary` | demo words | Names the project, sandbox, terminal and GitHub strings (`Vocabulary::openshell()` is the OpenShell set). |
| `Surfaces.account_form` | four providers, three sources | Provider and source lists, labels, the preselected source and the 1Password note. |
| `Account.launch_blocked`, `Account.edit_blocked` | `None` | A usage-only account is not offered for launch; an account the form cannot represent keeps its other actions. |
| `Surfaces.agent_blocked` | `None` | Every agent except one is a disabled launch row with a reason. |
| `Surfaces.launch_role` | `Demo` | `Configured` resolves the launch role from the default or the only allowed role, else the picker rows are disabled with a reason. |
| `Surfaces.wizard` | `None` | One fixed destination, no edit step, no subfolders, and the name from the source folder. |

## Rules that hold today

- Behaviour lives in the shared library (`src/core`, `src/shell`,
  `src/ui`, `src/widgets`), never duplicated per screen or per app.
  Apps provide routes, pages and configuration only.
- Every widget follows the same conventions: builders, events,
  outcomes, one keyboard-focus gutter (`Theme::gutter_symbol`),
  selection distinct from focus, light-dismiss owned by the popup.
- Behaviour is overridable in layers: app/profile policy, then
  per-widget setting, then user key remapping.
- The showcase demonstrates every library capability; the coverage
  gate is described in
  [shared components plan](architecture/shared-components-plan.md).

## Testing today

- `cargo nextest run --locked --test functional --profile functional`
  — 93 tests, 0 skipped: every app end-to-end (launch → operate →
  verify consequence → reopen → clean exit), component families in
  Showcase and in a real consumer, colour matrix
  (truecolor/256/16/mono/`NO_COLOR`), normal and small sizes.
- Every test ends in `App::visual("<id>")`, a `FrozenStore` check
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
