# Specification: the new jackin console screens (P9.2)

Task P9.2 of the jackin OpenShell rewrite, step 1 of 2. Decisions D11, D13, D24, D26, D27 and D28 of `jackin/rewrite/DESIGN.md` apply. This file is the contract that `rust-cli-tui-engineer` builds in step 2. A `ux-specialist` reviews the rendered result before any new tuiscotti baseline is accepted.

Facts about the baseline below were read from the tree at `2565a0c` (branch `wt/P9.2`) and checked by running `jackin-preview` (`cargo run --bin jackin`). The Cargo binary is named `jackin`; this file calls it `jackin-preview`.

## 0. Ground rules

**R1. Existing output does not change.** The full suite (93 tests at `2565a0c`) and every baseline under `tests/functional/baselines/` stay byte-identical. Several existing baselines contain exactly the cells this work touches: `jackin_editor_discard` (workspace editor, General tab), `jackin_settings_leave` (manager with `daemon · 3 s ago`), `jackin_help_ownership` (the help overlay, `1–29 of 30`). So every new key, row, tab, menu item, help row and hint exists only when its data is present (section 1.1). With all new data absent, every existing scenario renders as it does today.

**R2. Existing components only.** Each screen is built from components that exist in the library today (index in section 9). Where no component fits, the screen section says so and lists it under section 8. No new widget is invented here. New items are screens, domain structs, `Effect`/`Msg` variants and showcase pages.

**R3. Demo words stay in demo data.** The library carries no OpenShell word. D28 content ("gateway", "OpenShell provider", "Codex") comes from data that `jackin-preview` (new scenarios) and `jackin-tui` (real) supply. Existing scenarios keep their Docker and Capsule words in their existing fixture data.

**R4. One implementation per behaviour.** The "Unsaved changes" dialog is built inline twice today (`editor.rs:505`, `settings.rs:370`). The role editor is the third user, so extract it once (`unsaved_dialog(id, changes)` in `console::screens::modals`) and migrate Editor and Settings in the same change. The dialog text and the baselines do not change.

**R5. Glyph and colour policy.** Status marks come from `StatusKind` (`theme.rs:1749`) and the theme glyph tokens, never a literal status glyph in new code, and always have a word beside the mark, so `NO_COLOR` and `--glyphs ascii` keep the meaning.

**R6. Copy.** Sentence case, active voice, plain words. An error names what failed, what is unchanged, and the next step. A button names its result ("Save…", "Run checks"); an ellipsis marks a button that opens another step, as in the demo ("Save…", "Add account…").

## 1. Shared design

### 1.1 The gate: `World.surfaces`

New file `console/domain/surfaces.rs`. `World` gets `pub surfaces: Surfaces` (default: all `None`). Each field turns on one family of screens.

```rust
#[derive(Debug, Clone, Default)]
pub struct Surfaces {
    pub roles: Option<RoleAdmin>,        // sections 2 and 3
    pub docker: Option<DockerSetting>,   // section 4
    pub doctor: Option<DoctorSurface>,   // section 5
}
```

`jackin-preview` existing scenarios leave it `Default`. `jackin-tui` fills it from the config, the role catalog and the doctor report. The identity gate (D27) runs jackin on the existing fixture data, so it exercises the `None` path; the new scenarios below exercise the `Some` path.

### 1.2 Routes, tabs and keys

| Item | Value | Notes |
| --- | --- | --- |
| `Route` (`app.rs:52`) | add `Roles`, `RoleEditor`, `Doctor` | `Page` and `Pages` (`pages.rs`) gain the same three variants. |
| Root tab | `TAB_ROLES = 3` (`pages.rs:14-18`) | Added only when `surfaces.roles.is_some()`. `pages::router()` takes `&World`. |
| `Go` (`screens/mod.rs:86`) | `Roles { select: Option<RoleName> }`, `RoleEditor { role: RoleName }`, `Doctor` | `Roles` selects the tab like `Go::Accounts`. `RoleEditor` and `Doctor` push a page like `Go::Editor` (`open_page`, `app.rs:252`). |
| Shell key | `R` opens Roles, `D` opens Doctor | New `JackinCmd::Roles` and `JackinCmd::Doctor`, allowed on Manager, Accounts, Usage and Roles. `shell_keys()` (`app.rs:2023`) takes `&World` and binds them only when the matching surface exists. |
| `Go` menu (`app.rs:585`) | `Roles  R`, `Doctor  D` | Same gate. |
| Footer hints | `R Roles  D Doctor` appended on Manager, Accounts, Usage | Same gate. |
| Preview `--help` text (`bin/jackin/main.rs`) | add `R Roles · D Doctor` and the new scenario names | Scenario names come from `Scenario::ALL`. |

Why capitals: the mnemonic lowercase letters are taken on at least one of Manager, Accounts and Usage (`r` is reconnect, refresh and refresh; `d` is delete and remove; `g h j k l` are motions). The lowercase letters still free on all three (`b f y z`) have no mnemonic for Roles or Doctor. Shift letters are plain keys in this shell (`Key::plain` ignores SHIFT, `core/event.rs:91`), and `KeyBinding::matches` ignores SHIFT for character keys (`core/keymap.rs:179-193`), so `KeyBinding::char('R')` matches a shifted `R`. Open question Q1.

Leave and deep links. A TUI has no URL, so the stable "deep link" of each route is its `--scenario` name plus `--motion paused --frame N`. Back is `Esc` (page `request_leave`, default `Go::Manager`); `RoleEditor` leaves to `Go::Roles { select: Some(role) }`. Reload (restart) restores state from the saved config and role files, not from page memory, so unsaved edits are lost by design and the unsaved dialog is the guard. Transient state (a picker, a dialog) lives in the layer stack and is never a route.

### 1.3 Effects and messages

All asynchronous work uses the existing pattern: the screen calls `World::perform(Effect)`; `Backend::perform` returns `Completion { after_ms, msg }` in the preview, and a real backend answers later with an external message (`world.rs:206`). New variants, each with a `job::` key for `cx.progress` (`world.rs:20`).

| Effect | Message back | Job key |
| --- | --- | --- |
| `CheckRole { role, doc: RoleManifestDoc }` | `RoleChecked { role, findings }` | `ROLE_CHECK` |
| `SaveRole { role, doc }` | `RoleSaved { role, ok, error: Option<String> }` | `ROLE_SAVE` |
| `BindRole { role, agent, account: Option<AccountId> }` | `RoleBound { role, agent, ok, error }` | `ROLE_BIND` |
| `AddRole { location }` | `RoleAdded { location, result: Result<Box<RoleRecord>, String> }` | `ROLE_ADD` |
| `RunDoctor` | `DoctorFinished(DoctorReport)` | `DOCTOR` |

Docker needs no effect: it is the `docker` field of `Workspace`, saved by the existing `Effect::SaveWorkspace`. `Msg` keeps `Clone + PartialEq + Eq`, so every payload does too.

### 1.4 Scenarios and tests added

Scenario names are added to `Scenario` and `Scenario::ALL` (`scenario.rs`), built in `world_for` (`fixtures.rs:1418`), and started in `App::start` (`app.rs`). Tests live in `tests/functional/jackin.rs` and `showcase.rs`, use the existing helpers (`tests/functional/support.rs`), run at 120x40 truecolor with `--motion paused` (and `--frame 40` unless a section names another frame), and end in `App::visual`. Baselines are approved only with `tuiscotti accept --expect-baseline missing` (`docs/functional-testing.md`).

| Item | Scenario (deep link) | Test function | New baseline id |
| --- | --- | --- | --- |
| Role editor | `role-editor` | `jackin_role_editor_check_and_save` | `functional/jackin_role_editor` |
| Role binding | `role-binding` | `jackin_role_binding_pick_and_clear` | `functional/jackin_role_binding` |
| Docker setting | `docker-setting` | `jackin_docker_setting_confirm_and_save` | `functional/jackin_docker_setting` |
| Docker, no engine | `docker-no-engine` | `jackin_docker_setting_no_engine` | `functional/jackin_docker_no_engine` |
| Doctor | `doctor` | `jackin_doctor_failure_and_rerun` | `functional/jackin_doctor` |
| Terminal pane page | showcase `--page terminalpane` | `showcase_page_terminalpane` (`page_test!`) | `functional/showcase_page_terminalpane` |
| Terminal pane replay | showcase `--page terminalpane` | `showcase_terminal_replay_mid_sequence` | `functional/showcase_terminal_replay` |
| OpenShell stages | `launch-openshell` | `jackin_launch_openshell_stages` | `functional/jackin_launch_openshell` |

The first baseline of each screen is the one the brief names: role editor, role binding, Docker setting, doctor, terminal pane (page), and the stage list. `docker-no-engine` and the terminal replay are extra because the rule "every functional test ends in `App::visual`" forces a baseline for any journey; each guards a state that the first test cannot reach. Eight new baseline files, zero changed.

### 1.5 Build order for step 2

Small verified portions, each ending with the full suite green and `git diff --stat -- tests/functional/baselines` listing only added files:

1. `Surfaces`, the `Workspace.docker` field and the gating plumbing, with no visible change.
2. Section 7 (stage list) and the R4 extraction. Existing baselines prove "byte-identical".
3. Section 4 (Docker), section 5 (Doctor).
4. Sections 2 and 3 (Roles tab, role editor, binding).
5. Section 6 (terminal pane page).
6. `src/catalog.rs` lines for every new public item; coverage allowlist lines for the app-specific blocks if the gate file exists (P9.1 decision; it is absent at `2565a0c`).

## 2. Role editor (D11)

### 2.1 Flow

Entry: Roles tab, role row, key `e`, or the `Edit manifest…` action in the detail (section 3). Exit: `Ctrl+S` then `Save` returns to the Roles tab with the status `Role sentinel saved`; `Esc` returns after the unsaved dialog when dirty. A role from a git source opens read-only. Hand-off: none; the editor never starts a launch.

Route: `Go::RoleEditor { role }`, crumb `Roles › sentinel › edit`, deep link `--scenario role-editor`.

### 2.2 Layout (page over the Roles tab, same chrome as `EditorScreen`)

- `Tabs` strip with `TabItem` per tab: `Manifest`, `Hooks`, `Environment`. `TabItem.dirty` marks a tab with staged edits; `TabItem.error` marks a tab that has a finding (as `editor.rs:2033`). Keys `←→`, `1–3`, `[` `]`.
- `Manifest` tab: a `StackScroll` of fields as in `render_general` (`editor.rs:531`): `Name *` (`TextInput`, required, role-name validator); `Dockerfile *` and `Policy *` (`TextInput` plus a `Choose…` `Button` that opens `modals::file_browser` rooted at the role directory); `Codex model` (`TextInput`, optional, placeholder `Codex default`); `Docker` (`Checkbox`, label `Docker for every launch of this role`, faint hint `Off by default · adds a Docker client and one engine rule`). Turning it on opens the same risk confirmation as section 4.2, from the same function, titled `Turn on Docker for role sentinel?`, when `surfaces.docker` exists.
- `Hooks` tab: `Setup` and `Preflight`, each a `TextInput` plus `Choose…`, with a faint line under each: setup `Runs once for each launch as user sandbox, with no terminal, in /sandbox/project`; preflight `Runs before each Codex start`.
- `Environment` tab: one row per `[env]` entry (name, default, prompt) in the list pattern of `ConfigTabs` (`config.rs`, the Settings Environments tab): cursor row, a group header with a count (as `Role overrides` in Settings), a final `+ Add entry…` row, change marks `+ • −` (`Change`), per-row problem text. A `Notice` of kind `Info` above the list: `Only non-secret values belong here. Put secrets in an account.` An entry form is a `FormDialog` with `TextInput` fields `Name *`, `Default`, `Prompt` (patterned on `open_env_form`, `config.rs:2054`).
- Under the tab body, a `Panel` titled `Check` (3 rows, scrolls) shows the result of `jackin role check`: `Check passed` (tone success) or one `Notice` of kind `Error` per finding, with `Notice.hint` carrying the fix text.
- Bottom row: `Cancel` and `Save…` `Button`s (`row_layout_right`), as `editor.rs:2075`. `Save…` is disabled with no changes, while checking, and for read-only roles.
- Preview on `Ctrl+S`: `Dialog::facts` titled `Save role`, facts `Role`, `Scope` (`role manifest · ~/roles/sentinel/jackin.role.toml`), `Changes` (`3 changes`), one `Prop` per changed field (`Codex model  none → codex-fixture`, `Docker  off → on`), code lines as `~ codex.model none → codex-fixture`. Same shape as `open_preview` (`editor.rs:334`).
- Leave: `Stakes::Unsaved` through the shared dialog of R4 (`Cancel`, `Discard`, `Save`).

### 2.3 Validation

Field level, synchronous, in the `TextInput` validator, so input stays in place with its error:

| Field | Rule | Message |
| --- | --- | --- |
| Name | non-empty, letters, digits, `-`, `_` | `Use letters, digits, - and _ in the role name.` |
| Dockerfile, Policy, hook paths | relative, no `..`, not empty (hooks may be empty) | `Use a path inside the role repository, for example hooks/setup.sh.` |
| Env name | the `env_key_error` rule (`workspace.rs`) with jackin's reserved names (`ReservedEnvName`); the demo list is a stand-in | `JACKIN_X is reserved by jackin. Choose another name.` |
| Env name | unique in the role | `An entry named FOO already exists. Edit it or choose another name.` |

File-level and policy-level checks need the repository, so they run through `Effect::CheckRole` on open, on `F5`, and before the preview opens. The panel prints each finding exactly as `jackin role check` prints it (`RoleError` Display from `jackin-session`); the console does not reword it. The fix text is separate data.

`jackin role check` stops at the first failing step, except that provider-host closure lists every violating rule (`API.md`, `RoleCatalog::load_path`). So `Done(findings)` holds one finding or several closure findings. After the user fixes one error and presses `F5`, the next may appear. See Q4.

Save is blocked while any finding exists. `Ctrl+S` with findings opens `InfoDialog` (`.error()`) titled `Check failed · sentinel` listing them, with `State: your edits are intact · nothing was written` and `Next: fix the entries marked !, then press F5`.

### 2.4 States

| State | What the user sees | Recovery |
| --- | --- | --- |
| Loading | Fields disabled; `Panel` body `Reading jackin.role.toml…` with a spinner (`spinner_frame`) | Wait; `Esc` leaves |
| Empty (Environment) | `EmptyState` `No environment entries` with primary action `Add entry…` | `a` or the action |
| Filled | All fields, `Check passed` | Edit |
| Partial | Form filled, one tab marked `!`, finding in the Check panel | Fix the field, `F5` |
| Recoverable error | Inline field error; Check findings; save failure `InfoDialog` `Save failed · sentinel` with `Error`, `State: your edits are intact · nothing was written`, `Next` | Fix and `Save…` again (pattern `editor.rs:462`) |
| Fatal error | Manifest does not parse: fields hidden, `Notice` `Error` with the verbatim message (for example ``jackin.role.toml: unknown field `toolz` at line 4``) and hint `Fix the file in your editor, then press F5.` Save disabled | Edit the file outside; `F5` reloads |
| Stale | Save returns `RoleSaved { ok: false }` with `jackin.role.toml changed on disk. Press F5 to reload; your edits stay until you do.` | `F5`, then reapply |
| Permission denied | Read-only role (git source): inputs disabled, `Notice` `Info` `This role comes from git. Edit its repository and pull.` A local file that is not writable fails at save with the stale path and the text `jackin.role.toml is not writable. Make it writable and save again.` | As stated |
| Offline | Not applicable: local files only; git roles are never fetched by this screen | None |
| Long content | Paths and prompts clip with an ellipsis (`ui::text::truncate`); the preview wraps them (`Prop::wrap`); the Environment list scrolls with a `scrollbar` | Open the row or the preview |
| Narrow (80x24) | `StackScroll` scrolls; the button row wraps (`row_lines`); the Check panel shrinks to one line `1 error · F5` under 28 rows | Scroll; `F5` |

### 2.5 Key map (consistent with the workspace editor and Settings)

`←→ Tab` tab, `1–3` jump, `[ ]` switch tab, `Enter` edit field, `Space` toggle Docker, `a` add, `e` or `Enter` edit, `d` remove, `u` undo row (Environment, as Settings), `F5` run the check (the demo's "refresh now" key), `Ctrl+S` save with preview, `Esc` back (asks when dirty), `?` help. Character keys do nothing while a text field has focus.

Help overlay: a `Role editor` section for `Route::RoleEditor`. Host menu File: `Save…`, `Cancel` (as Editor).

### 2.6 Data interface (new, `console/domain/role.rs`)

```rust
pub struct RoleAdmin { pub agents: Vec<Agent>, pub roles: Vec<RoleRecord> }
pub struct RoleRecord {
    pub name: RoleName,
    pub source: RoleSource,                           // existing: Git or Local
    pub manifest: Result<RoleManifestDoc, String>,    // Err = the verbatim role check message
    pub check: RoleCheckState,
    pub bindings: BTreeMap<Agent, AccountId>,         // section 3
}
pub enum RoleCheckState { Unchecked, Running, Done(Vec<RoleFinding>) }   // Done(vec![]) = passed
pub struct RoleFinding {
    pub area: FindingArea,        // Manifest, Dockerfile, Policy, Hook, Provider
    pub key: Option<String>,      // field path, for example "role.dockerfile", "env.FOO.default"
    pub message: String,          // as jackin role check prints it
    pub fix: String,              // next step, owned by jackin-tui
}
pub struct RoleManifestDoc {
    pub name: String, pub dockerfile: String, pub policy: String,
    pub codex_model: Option<String>, pub docker: bool,
    pub hook_setup: Option<String>, pub hook_preflight: Option<String>,
    pub env: Vec<RoleEnvEntry>,   // sorted by name
}
pub struct RoleEnvEntry { pub name: String, pub default: Option<String>, pub prompt: Option<String> }
impl RoleRecord { pub fn writable(&self) -> bool }   // Local source only
```

The fields mirror `RoleManifest` (D11, `API.md` "Role manifest"). `key` places an inline error on the field or tab that owns it.

### 2.7 Demo data and scenario `role-editor`

`fixtures.rs` adds `surfaces.roles` for the new scenarios only; `role-editor` also sets `surfaces.docker` as in section 4.6, so the shared risk confirmation is reachable. Roles: `sentinel` (local `~/roles/sentinel`; manifest: Dockerfile `Dockerfile`, policy `policy.yaml`, model none, docker off, hooks `hooks/setup.sh` and `hooks/preflight.sh`, env `SENTINEL_FAIL_PREFLIGHT` default `0` prompt `Fail the preflight hook on purpose`, and `SENTINEL_NOTE` no default prompt `Note for the run`); `the-architect` (git, read-only); `reviewer` (git, docker on); `data-eng` (local, check fails); `writer` (local, manifest does not parse). The backend answers `CheckRole` with a pure rule table over a fixture file list (`Dockerfile`, `Dockerfile.old`, `policy.yaml`, `hooks/setup.sh`, `hooks/preflight.sh`):

| Staged value | Finding message | Fix |
| --- | --- | --- |
| Dockerfile `Dockerfile.old` | `Dockerfile.old:12: the final stage must start FROM construct, found "ubuntu:24.04"` | `Change the last FROM line to FROM construct, then press F5.` |
| Policy `missing.yaml` | `missing.yaml: file not found in the role repository` | `Create the file or choose an existing one with Choose….` |
| Hook `hooks/other.sh` | `hooks/other.sh: file not found in the role repository` | same |
| Env name `JACKIN_X` | rejected in the form (field level) | n/a |

`SaveRole` completes in 900 ms.

### 2.8 Functional test `jackin_role_editor_check_and_save`

1. Open `role-editor`. Wait for `Roles › sentinel › edit`. Assert `sentinel`, `policy.yaml`, `Check passed`, tabs `Manifest  Hooks  Environment 2`.
2. Tab to `Codex model`, type `codex-fixture`. Tab to Docker, `Space`. Assert `Turn on Docker for role sentinel?` and the four risk texts; `Right` (focus `Turn on`), `Enter`. Assert the `Manifest` tab shows the dirty mark and the strip shows `2 changes`.
3. `Ctrl+S`. Assert dialog `Save role`, `Codex model  none → codex-fixture`, `Docker  off → on`. `Esc`. Assert no `Role sentinel saved` text and the page still dirty (forbidden side effect: nothing saved).
4. Replace the Dockerfile value by `Dockerfile.old`, press `F5`. Wait for `final stage must start FROM construct`; assert the `Manifest` tab carries the error mark and the input still holds `Dockerfile.old`.
5. `Ctrl+S`. Assert `Check failed · sentinel` and `nothing was written`. `Esc`.
6. `visual("functional/jackin_role_editor")`. Final frame: dirty tab, error finding, Docker checked.
7. Narrow: `resize(80, 24)`; assert fields reachable and `1 error` visible; no baseline.

### 2.9 Acceptance

- Every D11 field is editable once and appears in the preview diff once. An unknown manifest key never reaches the form (the file fails to parse, fatal state).
- The panel text equals the `Display` of the fixture error; the console adds no words to it.
- Typed input survives every error path (validator, check failure, save failure).
- Save is unreachable with a finding or while checking; the dirty guard is the shared dialog.
- Hover, focus and disabled states of `Button`, `Checkbox`, `TextInput` come from the components; `NO_COLOR` and ASCII renders show `!` markers as words (`error`) in the tab label.
- Existing baselines unchanged.

## 3. Role binding (D26)

### 3.1 Flow

The Roles tab is the screen. Left: the list of roles. Right: the detail of the selected role with its **bindings**: one row per agent. Entry: `R`, `Go › Roles`, or `Go::Roles { select }`. Pick: `Enter` on an agent row opens a `Picker` of eligible accounts. Clear: `d` on the row. Exit: `Esc` to Manager. Hand-off: `e` to the role editor (section 2); the Picker's empty state links to Accounts (`Go::Accounts`).

D26 order, shown to the user in a faint line under the list: `A launch uses --account, then the workspace binding, then this role binding, then the provider default, then the only account.`

Route: `Route::Roles` (root tab 3), deep link `--scenario role-binding`.

### 3.2 Layout

`MasterDetail` (`ui/master_detail.rs`), the pattern of Manager and Accounts, so the split, drawer and summary modes and the narrow behaviour come from the component.

- Master: `Panel` titled `Roles`, meta `5 roles · 1 !`. Rows: `StatusKind` mark of the check, role name, source chip (`path` or `git`), binding summary (`Codex · Primary` or faint `no binding`). Last row `+ Add role…`, which opens a `FormDialog` titled `Add role` with one required field `Location` (a directory such as `~/roles/sentinel` or a git URL) and sends `Effect::AddRole`. A refusal opens `InfoDialog` `Could not add role` (`State: no role was registered`), and the form reopens with the typed location. Filter with `/` as Accounts. Row layout (shared `ListDetail`): the source chip column is reserved for every row, and the binding summary is truncated first (`Codex · Experimen…`). The check state shows through the mark only.
- Detail: header `Role · sentinel` with meta `local · check passed`. A `props` sheet (`Prop`): `Source`, `Dockerfile`, `Policy`, `Codex model`, `Docker` (`on` in warning tone when the manifest enables it), `Hooks`, `Environment` (`2 entries`), `Check`. Then the section `Account bindings` with one row per `RoleAdmin.agents` entry:

```
Account bindings                                    default account of this role
 ›  Codex        Codex · Primary          role binding
    Claude Code  none · uses Claude · Personal ★   provider default
```

  The second column is the bound account title, or `none` plus the account that D26 would pick without a role binding (from `resolve_account` with the D26 order, section 3.6). The third column is the rule that applies. A bound account that is disabled or needs login shows a `Warn` mark and its reason (`disabled globally`, `needs login`), reusing `Usability::label`.
- Actions row (`Button`, as Accounts): `Edit manifest…`, `Bind account…`, `Clear binding`, `Check`, `Remove…`.
- `Picker` (`Bind account · sentinel · Codex`): items are the enabled accounts whose provider is `agent.provider()` (the D26 invariant, so the picker cannot offer a wrong provider). Item label: account title; detail: `★ default for OpenAI`, `ready` or the `Usability` reason; the current choice carries the chosen mark (`Theme::chosen_in`).
- Dialogs: `Dialog::destructive` for `Remove role`; `InfoDialog` (`.error()`) for a refused removal.

### 3.3 States

| State | What the user sees | Recovery |
| --- | --- | --- |
| Loading | List rows with a spinner until `RoleAdmin` is present; per-role check `Running` shows a `Running` mark | Wait |
| Empty | `EmptyState` `No roles yet` / `A role holds the Dockerfile, policy and manifest of an agent. Add one to launch it.` Primary action `Add role…` | `a` |
| Filled | As the layout | Pick, clear |
| Picker empty | `EmptyState` `No Codex accounts` / `Add an OpenAI account first.` Actions `Add account…` (goes to `Go::Accounts`) and `Cancel` | `Add account…` |
| Saving | The agent row shows a spinner and the other rows stay usable; status `Binding sentinel…` | Wait |
| Recoverable error | Binding save fails: status error `Could not save the binding: <reason>. Nothing changed. Try again.`; the row keeps its old value | `Enter` again |
| Role problem | Role with a failing check: row `!`, detail `Notice` `Warning` `The manifest has errors. Launches of this role fail until you fix them.` Binding still works | `e` to edit, or fix the files |
| Broken manifest | Row `!`; detail shows the verbatim message; `Edit manifest…` opens the editor in its fatal state | Fix outside, `v` re-checks |
| Stale | A bound account that no longer exists cannot occur (`RemoveAccount` refuses, `AccountInUse`). A disabled bound account shows `Warn` | Re-pick or clear |
| Permission denied | Config not writable: the save-error text above plus `Make ~/.config/jackin/config.toml writable.` | Fix permission, retry |
| Offline | Not applicable: local config only | None |
| Long content | Names and account titles clip (`fit`/`truncate`); the detail scrolls | `Tab` into the detail, `PageDown` |
| Narrow | `MasterDetail` drawer: `Enter` opens the detail, `Esc` closes it | As Manager |

Clearing is not confirmed because it is reversible: the status reads `Cleared the Codex binding of sentinel · u undoes` (the row already says what the launch uses, and a longer status pushes the key hints out of the footer) and `u` restores the previous value once, while the screen is open. Removing a role is confirmed (`Dialog::destructive`, body `jackin forgets the role. The files in ~/roles/sentinel stay.`); a role that a workspace names as default is refused with `InfoDialog`: `Role sentinel is the default role of payments-platform. Choose another default role, then remove it.` (`RoleInUse`).

### 3.4 Key map

List focus: `↑↓ j k` move, `PageUp PageDown`, `Home g`, `End G`, `Enter` or `Tab` details, `e` edit manifest, `a` add role, `d` remove, `v` check (the demo's `v` validate), `/` filter, `F5` check all, `Esc` back. Detail focus on a binding row: `↑↓` move, `Enter` pick, `d` clear, `u` undo, `←` or `Esc` back to the list, `Tab` to the action row. `?` help. These mirror Accounts (`Enter` details, `r` refresh, `a` add, `/` filter, `Space` default, `d` remove); `Space` is not used because the binding needs a choice, not a toggle.

Host menu File for `Route::Roles`: `Add role…  a`, `Edit manifest…  e`, `Check  v`, `Check all  F5`, `Filter…  /` (as Accounts).

### 3.5 Data interface

`RoleRecord.bindings` and `RoleAdmin.agents` (section 2.6); `AccountRegistry` and `Usability` are existing. `agents` is data because the library enum `Agent::ALL` has six agents and jackin runs one (Codex); the screen never reads `Agent::ALL`. The picker lists accounts from the existing `AccountRegistry` (`console/domain/account.rs`) filtered by provider.

### 3.6 Precedence

The demo order is session, role (through the workspace), workspace, provider default, discovered (`precedence.rs`). D26 is flag, workspace, role, provider default, only account. The two differ in the order of role and workspace. Add `AccountOrder { Demo, Jackin }` to `RoleAdmin` (default `Demo` elsewhere). `resolve_account` takes it; with `Demo` it is byte-identical to today. The detail uses `Jackin`. Q3.

### 3.7 Demo data and scenario `role-binding`

Reuse the demo accounts: OpenAI `Primary` (default) and `Experiments`; Anthropic `Personal` (default) and `Work`. `agents = [Codex, ClaudeCode]` so the list pattern is visible (jackin passes `[Codex]`). Bindings: `sentinel` Codex to `Primary`; `the-architect` none; `reviewer` Codex to `Experiments`. `BindRole` completes in 900 ms; the first bind for `sentinel` fails once with `could not write config.toml: permission denied (EACCES)`. The scenario opens on the Roles tab with `sentinel` selected.

### 3.8 Functional test `jackin_role_binding_pick_and_clear`

1. Open `role-binding`. Assert the list rows, `Codex  Codex · Primary  role binding` and the D26 order line.
2. `Tab`, `Enter` on Codex. Assert the `Picker` shows `Primary` as chosen and `Experiments`. `Esc`. Assert the binding is unchanged.
3. `Enter`, `Down`, `Enter` (pick `Experiments`); `advance_clock(900)`. Assert the error status and the old value. (Forbidden side effect: the row must not show `Experiments`.)
4. Repeat the pick; assert `Experiments` after `advance_clock(900)`.
5. `d`; assert `none · uses Codex · Primary ★` and `u undoes`; `u`; assert `Experiments`; `d` again.
6. `visual("functional/jackin_role_binding")`.
7. `resize(80, 24)`; assert `Enter` still opens the detail (drawer).

### 3.9 Acceptance

- Only same-provider, enabled accounts are offered. No code path writes a binding the picker did not offer.
- Cancelling the picker writes nothing; a failed write leaves the previous value on screen.
- The effective-account text matches `Config::select_account` order D26 for a table of cases (verified in P9.3).
- A bound account shows its usability; a role with errors shows the warning but remains bindable.
- Existing baselines unchanged; with `surfaces.roles = None` there is no tab, no `R`, no menu item, no hint, no help row.

## 4. Docker setting of a workspace (D24)

### 4.1 Flow

Entry: workspace editor (`e` on a workspace in Manager) → `General` tab → `Docker` row. The row sits after `Git pull before launch` and before `On dirty exit`. Turning it on opens a confirmation with the four risks. Save goes through the existing preview (`Ctrl+S`). Exit: `Save` returns to Manager, whose workspace detail shows `docker on` in the `Policies` line. Route: unchanged (`Go::Editor`); deep link `--scenario docker-setting`.

The same data feeds two launch surfaces (D24: "`jackin load` and the console show a warning for each launch"). They are small and gated: the launch picker's detail line gains `Docker is on for this launch` when the effective capability is on (workspace flag or role manifest), and the cockpit shows one `Notice` of kind `Warning` under the credentials block, `Docker is on · the agent can read the engine key · i shows all risks`, while the `i` info dialog (`open_info`, `cockpit.rs`) gains the four risk `Prop`s. Both read `DockerSetting` and `Workspace.docker`. Q6 asks where a persistent warning belongs if the owner prefers another place.

### 4.2 Layout

The `General` `Stack` (`editor.rs:531`) gets one more fixed child only when `surfaces.docker.is_some()`. In the gated-off case the child list is the current eight, so the screen is unchanged.

```
   [✓] Keep awake               macOS only
   [✓] Git pull before launch
   [ ] Docker for the agent     Off by default
       On dirty exit ...
```

- `Checkbox` (`widgets/choice.rs`), label `Docker for the agent`, with a faint trailing hint like `macOS only` on Keep awake: `Off by default`; when checked `On · jackin-local · host.openshell.internal:18476`; when a role in this workspace turns it on itself `Always on for role reviewer`.
- Checked state adds a one-line `Notice` of kind `Warning` below the row: `4 risks · press i to read them`.
- Turning on opens `Dialog::facts` with `title_status(StatusKind::Warn)`, title `Turn on Docker for payments-platform?`, the four risks as `Prop`s (label and wrapped text), buttons `Cancel` and `Turn on`, initial focus on `Cancel` (`Dialog.initial_focus`). `Esc` cancels (the dialog's cancel action). Turning off needs no confirmation. One function in `console::screens::modals` builds this dialog for the workspace editor and the role editor.
- `i` opens the same facts read-only (one `Close` button).
- Save preview (`open_preview`): a `Prop` `Docker  off → on` with warning tone and a code line `~ docker false → true`. `Workspace::change_count` counts the field.
- Manager detail, `Policies` row: `... · docker on`, only when on and only when the surface exists.

Risk texts (D24, four facts, one `DockerRiskText` each, in `DockerRisk::ALL` order):

| Label | Text |
| --- | --- |
| Client key | The agent can read the engine client key. |
| Network | Containers bypass the sandbox network policy. |
| Bind mounts | A bind mount names a path on the engine host, not in the sandbox. |
| Lifetime | The engine outlives the sandbox. Sandboxes that share one engine see each other's containers, and you remove them. |

### 4.3 States

| State | What the user sees | Recovery |
| --- | --- | --- |
| Off (default) | Unchecked row, `Off by default` | `Space` |
| On, engine configured | Checked row with the engine endpoint, warning line | `i` risks, `Space` turn off |
| On, no engine | Checked row; `Notice` `Warning`: `Gateway jackin-local has no Docker engine. Launches fail until you add one with jackin gateway add.` The preview keeps `Save…` enabled (the flag is valid config) | Add the engine outside; the next launch works |
| On by a role | Row shows `Always on for role reviewer`; unchecking the workspace flag does not remove it | Edit the role (section 2) |
| Loading | Not applicable: the setting is part of the loaded workspace | None |
| Empty | Not applicable | None |
| Recoverable error | Save failure: the existing `InfoDialog` `Save failed` | Save again (pattern `editor.rs:462`) |
| Dirty | Tab `General` shows the dirty mark; strip shows the change count | Save or discard |
| Permission denied | As save failure | As stated |
| Long content | Endpoint and gateway clip (`fit`); the facts wrap | `i` |
| Narrow | The row hint drops first; the `Notice` wraps | As the General tab |

### 4.4 Key map

`Space` toggles (turn on asks first), `i` risks, `Ctrl+S` save with preview, `Esc` back (unsaved dialog). `Tab`/`Shift+Tab` order includes the row between Git pull and On dirty exit.

### 4.5 Data interface (new, `console/domain/docker.rs`)

```rust
pub struct DockerSetting {
    pub gateway: String,                   // the gateway the launch uses
    pub engine: EngineState,
    pub risks: Vec<DockerRiskText>,        // 4, in DockerRisk::ALL order, mapped by jackin-tui
    pub role_enabled: BTreeSet<RoleName>,  // roles whose manifest has [docker] enabled = true
}
pub enum EngineState { Configured { endpoint: String }, Missing }
pub struct DockerRiskText { pub label: String, pub text: String }
```

`Workspace` gains `pub docker: bool` (default `false` in `Workspace::new`); `change_count` and `general_dirty` include it.

### 4.6 Demo data and scenarios

`docker-setting`: `engine = Configured { endpoint: "host.openshell.internal:18476" }`, gateway `jackin-local`, workspace `infra-control-plane` has `docker = true`, role `reviewer` is in `role_enabled` and allowed in `payments-platform`. `docker-no-engine`: the same with `EngineState::Missing`. Both start on Manager as `returning` does.

### 4.7 Functional tests

`jackin_docker_setting_confirm_and_save` (`docker-setting`):

1. `Down`, `e`; assert `Docker for the agent` unchecked and `Off by default`.
2. Press `Tab` until the row has focus, then `Space`. Assert `Turn on Docker for payments-platform?` and the four risk texts. `Esc`. Assert the box is still unchecked and the tab is clean.
3. `Space`, `Right` (focus `Turn on`), `Enter`. Assert checked, `4 risks`, the dirty mark.
4. `Ctrl+S`. Assert `Docker  off → on`. `Enter` (Save). Wait for the Manager and `docker on` in `Policies`.
5. `e` again; assert the box is checked and the page is clean. `visual("functional/jackin_docker_setting")`.

`jackin_docker_setting_no_engine` (`docker-no-engine`): steps 1 to 3, then assert `has no Docker engine` and `jackin gateway add`; `visual("functional/jackin_docker_no_engine")`.

### 4.8 Acceptance

- Off by default; no risk text anywhere while the flag is off and no role enables it.
- The four D24 facts appear verbatim in the confirm dialog, the `i` view, and the cockpit info dialog.
- Cancel at the confirm leaves the pending value unchanged.
- The setting persists across save and reopen (the test).
- With `surfaces.docker = None` the editor, preview, Manager detail and help are byte-identical (existing `jackin_editor_discard` and `jackin_settings_leave` pass).

## 5. Doctor panel (D13, D28)

### 5.1 Flow

The demo's "daemon health" is a block, not a screen: the `Daemon` section of the wide roster (`manager.rs:1759`), the `daemon · 3 s ago` meta on `Instances` (`manager.rs:1353`), and the `▲ daemon stale` strip segment (`app.rs:711`, `app.rs:1768`), driven by `World.daemon_health` (`world.rs:160`). D28 says the screen shows the OpenShell gateway health instead. So the doctor panel is the detail behind those three surfaces.

Entry: `D` on Manager, Accounts, Usage or Roles, or `Go › Doctor`. Exit: `Esc` to Manager. Hand-off: a launch failure that names the gateway offers `Open doctor` (Q7). The panel never repairs anything (D13): it states the problem and the fix.

When `surfaces.doctor` is `Some`, the three surfaces read the report instead of `daemon_health`: block title and meta use `DoctorSurface.noun` (`Gateway`, `gateway · 3 s ago`), the strip says `▲ gateway failing` or `▲ gateway stale`. When `None`, they are unchanged.

Route: `Go::Doctor`, crumb `Doctor`, deep link `--scenario doctor`.

### 5.2 Layout

`MasterDetail` again, so narrow terminals get the drawer for free.

- Master: `Panel` titled `Doctor`, meta `2 failing` or `all checks pass`. One row per check: `StatusKind` mark (`Ok`, `Warn`, `Fail`, `Skipped`, and `Running` while a run is in progress), label, one-line summary. Rows come from the report, in report order (the seven D13 items: `OpenShell binary`, `SSH binary`, `Git binary`, `Gateway registered`, `Gateway health`, `Pinned version`, `Engine socket`).
- Detail: header with the check label and its status word; `props` sheet (`Prop`): `Status`, `Found`, `Expected`, `Gateway`; a `Notice` of kind `Error` or `Warning` carrying the fix (`Notice.message` is the problem, `Notice.hint` is the fix); buttons `Run checks` and `Copy report`.
- Header line above the list (as Accounts "Health"): `degraded · 1 failing · 1 warning · checked 4 s ago`, using `health_spans` tones (`screens/mod.rs`).
- Never-run: `EmptyState` `Checks have not run yet` with primary action `Run checks`.

### 5.3 States

| State | What the user sees | Recovery |
| --- | --- | --- |
| Never run | `EmptyState` above | `r`, `F5` or the action |
| Running | Every row shows `Running` and keeps its last summary in faint text; status `Running checks…` | Wait; `Esc` leaves, the run finishes in the background |
| All pass | Header `all checks pass · checked 3 s ago`, all rows `Ok` | None |
| Warning | Row `Warn`, for example `OpenShell 0.1.7 is newer than the pinned 0.1.6`; fix text | Install the pinned version |
| Failure | Row `Fail` with the found value and the fix, for example `Engine socket · /var/run/docker.sock not found` / fix `Start Docker or OrbStack, then press r.` | Fix outside, `r` |
| Gateway not chosen | `Gateway registered` and `Gateway health` are `Skipped`: `No gateway is configured. Run jackin gateway add <name>.` Other checks still run | As stated |
| Gateway failed to connect | `Gateway health` `Fail` with the error; the console is still open | Start the gateway (operator task, D13), `r` |
| Stale | Report older than 60 s: header `stale · checked 4 min ago` in warning tone | `r` |
| Permission denied | An engine socket the user cannot read is a `Fail` with `permission denied`; fix `Add your user to the Docker group, then press r.` | As stated |
| Offline | A gateway on another host is unreachable: same as failed to connect | As stated |
| Long content | Paths and version strings wrap in the detail (`Prop::wrap`), clip in the list | `Enter` details |
| Narrow | `MasterDetail` drawer | `Enter`, `Esc` |

### 5.4 Key map

`↑↓ j k` move, `Home g`, `End G`, `Enter` or `Tab` details, `r` or `F5` run checks (F5 is the demo's "refresh now"), `y` copy the report (`cx.copy`), `Esc` back, `?` help, `q` leave. Host menu File: `Run checks  r`, `Copy report  y`.

### 5.5 Data interface (new, `console/domain/doctor.rs`)

```rust
pub struct DoctorSurface { pub noun: String, pub state: DoctorState }   // noun: "gateway" in jackin
pub enum DoctorState { NeverRun, Running { previous: Option<DoctorReport> }, Done(DoctorReport) }
pub struct DoctorReport { pub checks: Vec<DoctorCheck>, pub at_secs: i64 }
pub struct DoctorCheck {
    pub label: String,
    pub status: StatusKind,        // Ok, Warn, Fail or Skipped
    pub summary: String,           // one line
    pub found: Option<String>,
    pub expected: Option<String>,
    pub fix: Option<String>,       // owned by jackin-tui, one per CheckId
}
```

`jackin-tui` maps `jackin_session::DoctorReport` (`CheckId`, `CheckStatus`, `detail`) one-to-one and adds `label` and `fix` per `CheckId` with an exhaustive `match`. The row count is the report's length, so a new `CheckId` needs no baseline change.

### 5.6 Demo data and scenario `doctor`

`surfaces.doctor = Some(DoctorSurface { noun: "gateway", state: Done(run 1) })`. The backend's `RunDoctor` is scripted by call count: run 1 fails `Engine socket` (`/var/run/docker.sock not found`) and warns `Pinned version` (`openshell 0.1.7, pinned 0.1.6`); run 2 passes all; run 3 fails `Gateway health` (`connection refused: jackin-local at 127.0.0.1:8080`). Each takes 900 ms.

### 5.7 Functional test `jackin_doctor_failure_and_rerun`

1. Open `doctor` (starts on Manager). Press `D`. Assert the seven labels, the header `1 failing · 1 warning`, and the engine row failing.
2. `Down` to `Engine socket`; assert the fix text `Start Docker or OrbStack`.
3. `r`; assert `Running checks…`, then `all checks pass`.
4. `r` again; assert `Gateway health` fails with `connection refused` and that the other rows keep their result (one failure does not hide the rest).
5. `visual("functional/jackin_doctor")`.
6. `Esc`; assert Manager shows `gateway failing` in the strip (the replaced surface).
7. `resize(80, 24)`; `D`; assert `Enter` opens the detail.

### 5.8 Acceptance

- Exactly the report's checks are shown; the panel never hides a failing check because another failed.
- No action changes the host or the gateway (D13): the buttons are `Run checks` and `Copy report`.
- Every `Fail` and `Warn` has a fix text; `Skipped` says why.
- With `surfaces.doctor = None` the three health surfaces and the strip are byte-identical.

## 6. Terminal pane showcase page (feature `terminal`)

### 6.1 Page

New page `terminalpane` (label `Terminal pane`, group `Text & code`, after `Terminal`; the existing page `terminal` is the task-output demo and stays). Files: `src/bin/showcase/pages/terminal_pane.rs`, `PageId::TerminalPane`, a `NAV_ENTRIES` row (deviation, accepted by the review: the page sits in `HIDDEN_ENTRIES` and is reachable only with `--page terminalpane`, because a sidebar row changes `Components 54` and the sidebar of every showcase baseline; the row moves to `NAV_ENTRIES` at the next intended showcase re-baseline, see `docs/audits/2026-11-02-showcase-nav-and-listdetail-page.md`), alias `terminalpane`, options in `options.rs`, and `page_test!(showcase_page_terminalpane, "terminalpane", "Terminal pane")`.

The page is wiring and demo data only (AGENTS.md showcase rule): it feeds bytes with `TerminalState::feed`, draws `TerminalView`, and prints `Modes` and `encode_key` results. It implements no emulation, no key encoding, no state logic.

Cargo: the showcase binary must build with the feature on. Make `terminal` part of `default` (`default = ["cli", "terminal"]`); `--no-default-features --features terminal` and `--no-default-features` still build, and jackin keeps its dependency choice. Q5.

The template's three sections:

- **Playground**: a `Panel` titled with the fixture name holding a `TerminalView` (focused flag follows the page focus), a control row (`Button`s `Step`, `Play all`, `Replay`, `Reset`), a `props` sheet of `Modes` (`app cursor`, `bracketed paste`, `alt screen`, `hide cursor`) and the screen `size` and `cursor`, and a **key echo** line: the last key and the bytes `encode_key` returns, in hex and caret form (`Up → ESC O A`), or `not encoded`.
- **Options** (`page_options`, one control per option): `Fixture` (`session`, `fullscreen`, `split`, `midstream`), `Size` (`80x24`, `40x12`, `20x6`), `Colour` (`on`, `off` → `TerminalView::color`), `Focused` (`on`, `off` → `TerminalView::focused`), `Replay start` (`stream start`, `inside a CSI sequence`, `inside an OSC sequence`, `inside a UTF-8 character`).
- **States** (`StateGrid` with `SpecimenState`): `Rest` (output, unfocused), `Focused` (cursor shown reversed), `Empty` (fresh state), `Overflow` (a 120 column line into 40 columns), `Narrow` (20 columns).

### 6.2 Fixtures (byte arrays in the page file, split into chunks)

```rust
struct Fixture { name: &'static str, rows: u16, cols: u16, chunks: &'static [&'static [u8]] }
```

The page owns a `TerminalState` and a cursor into `chunks`; that is all of its state.

| Fixture | Content | Proves |
| --- | --- | --- |
| `session` | prompt, `ls` with 16, 256 and RGB colours, bold, dim, italic, underline, inverse, wide characters (`日本語`), CRLF | `feed`, SGR, wide cells |
| `fullscreen` | `ESC[?1049h ESC[?25l ESC[?1h ESC[?2004h`, a box-drawn frame, a status line, then `ESC[?1049l` | `Modes` for all four flags; leaving the alternate screen restores the primary one |
| `split` | one SGR split across chunks (`ESC` / `[38;2;7` / `2;224;84mgreen`), a UTF-8 box character split between bytes (`E2 94` / `82`), an OSC title split before its `BEL` | `feed` is stateful across calls |
| `midstream` | four chunks. c1 `ESC[38;2;72;224;84mtail ESC[0m CRLF`. c2 `ESC]0;title BEL after CRLF`. c3 `E2 94 82 x CRLF` (the box character `│` and `x`). c4, the redraw: `ESC[2J ESC[H ready $ ` | replay from the inside of a sequence |

`Replay` resets the state, then feeds the rest of one starting chunk from a byte offset. `Replay start` picks the chunk and the offset: `stream start` is (c1, 0); `inside a CSI sequence` is (c1, 9), after `ESC[38;2;72`; `inside an OSC sequence` is (c2, 2), after `ESC]`; `inside a UTF-8 character` is (c3, 2), on the continuation byte `82`. `Step` feeds the next chunk (the redraw chunk is last); `Play all` feeds the rest of the chunks and stops before a fixture's leave chunk (`ESC[?1049l`), so the modes still read `on`; one more `Step` feeds the leave chunk. The counter in the panel meta counts the leave chunk (`fullscreen`: `3 of 4 chunks` after `Play all`, `4 of 4` after the `Step`); `Reset` calls `TerminalState::reset`. Size changes call `TerminalState::resize`.

Expected results, verified against `vt100 0.18.1` in a scratch program on 2 Nov 2026 (a fresh 5x30 parser, the remainder of the starting chunk fed in one `feed` call):

| Replay start | After the starting chunk | After the redraw chunk |
| --- | --- | --- |
| stream start | `tail` in colour RGB 72,224,84 | row 0 shows only the prompt |
| inside a CSI sequence | row 0 shows the literal text `;224;84mtail`, no colour | row 0 shows only the prompt |
| inside an OSC sequence | row 0 shows `0;titleafter` (the `BEL` is dropped) | row 0 shows only the prompt |
| inside a UTF-8 character | row 0 shows `x`; the stray byte is dropped | row 0 shows only the prompt |
| `split` fixture fed whole | `green` in colour RGB 72,224,84, `│x`, title consumed | not applicable |

This is the behaviour D28 relies on: each attach builds a fresh parser, and the redraw nudge (D09) repairs a stream that starts inside a sequence. The page documents it in one faint line under the options: `A stream that starts inside a sequence prints its tail as text until the next full redraw.`

### 6.3 Capture and keys

The page captures keys only while the pane is active, so there is no keyboard trap (WCAG 2.2). `Enter` on the pane starts capture; the page reports `is_editing()` (the showcase's `capturing()`, `app.rs:2258`), every key goes through `encode_key` and is echoed, nothing is sent anywhere, and `Ctrl+G` releases. Hints: `Enter Capture keys` and, while captured, `Ctrl+G Release`. Outside capture: `Tab` moves focus, `Step`/`Play all`/`Replay`/`Reset` are buttons with the usual keys.

### 6.4 States

| State | What the user sees | Recovery |
| --- | --- | --- |
| Empty | `Empty` specimen; before any `Step` the pane is blank with the cursor | `Step` or `Play all` |
| Filled | Rendered fixture | `Reset` |
| Hidden cursor | `fullscreen` after the flags: no cursor cell even when focused | `Reset` |
| Overflow | Long lines wrap by the parser; the view clips what does not fit its `Rect` | Change `Size` |
| Colour off | Attributes stay, colours drop (`NO_COLOR` look) | `Colour on` |
| Not encoded | A key without bytes echoes `not encoded` | None |
| Narrow | Controls wrap; a 20 column pane fits | Resize |
| Loading, error, offline, permission denied | Not applicable: the block does no I/O | None |

### 6.5 Functional test `showcase_terminal_replay_mid_sequence`

Run `--page terminalpane --motion paused` at 120x40.

1. Wait for `Terminal pane`; assert the `session` fixture is drawn and all four mode flags read `off`.
2. Set `Colour` to `off`; assert the coloured sample cells lose their colour while bold cells stay bold (`crop` compare against the rest frame). Set it back to `on`.
3. Set `Fixture` to `fullscreen`, `Play all`. Assert `alt screen on`, `hide cursor on`, `app cursor on`, `bracketed paste on`.
4. `Enter` on the pane, press `Up`. Assert the echo `ESC O A`. `Ctrl+G`. `Reset`; capture again and press `Up`; assert `ESC [ A` (application cursor is off after reset). `Ctrl+G`.
5. Set `Fixture` to `midstream` and `Replay start` to `inside a CSI sequence`; `Replay`. Assert row 0 contains `;224;84mtail`, and that those cells carry no foreground colour.
6. `Step` three times (c2, c3, then the redraw chunk). Assert the literal text is gone and the prompt shows.
7. `Replay` again and stop after the starting chunk. `visual("functional/showcase_terminal_replay")`: the frame with the literal tail.

The page test `showcase_page_terminalpane` visits the page and ends in `visual`.

### 6.6 Acceptance

- Every public item of the feature is on the page: `TerminalState::{new, feed, resize, reset, size, modes, cursor}`, `Modes`, `TerminalView::{new, focused, color}`, `encode_key`. Their `catalog.rs` lines (`catalog.rs:3649-3661`) are met by the page, so no allowlist entry is needed.
- The three orphan starts and the stream start produce exactly the table above.
- The page works with `--color 16` and `none`; no keyboard trap; hints list only deliverable keys.
- Gap: `TerminalState` exposes no scrollback access and `TerminalView` draws only the live screen, while the demo's capsule panes scroll and copy (section 8).

## 7. Data-driven cockpit stage list

### 7.0 Entry, route, keys, states

Entry and route are unchanged: `Go::Launch` opens `Route::Cockpit` (`app.rs`), scenarios `launch-running` and `launch-failure` for the demo and the new `launch-openshell`. Keys are unchanged (`b` or `Enter` build log, `i` info, `x` cancel, `t` debug, `Ctrl+Q` quit, `Esc` explained by a status line, `cockpit.rs:600-660`) except that the hint `i Container info` reads `i {unit} info`. States are the existing `StepState` set (queued, running, done, skipped, failed, blocked) plus the cockpit's credential hold, failure dialog, cancel and quit dialogs; none is added or removed. The data edge cases to prove:

| Case | Behaviour | Recovery |
| --- | --- | --- |
| One stage | Rail of one row; `stage 1 of 1` | None needed |
| More stages than rows | `rail_h` clamps to the height (`cockpit.rs:784`); the frontier stays visible | Resize |
| Long label | Clipped to the 44 column rail | Resize |
| Failure in the `BuildLog` stage | Dialog title is `LaunchWords.build_failed`; the log opens with `b` | Fix the cause, launch again |
| Failure elsewhere | Title `Launch failed` | As the dialog's `Next step` |
| Credential hold | Pause dialog names the `Credentials` stage by its label | `Retry`, `Enter plain text instead`, `Cancel launch` |
| Empty list | Not representable: `StageList` is non-empty by construction | None |

### 7.1 What changes

The cockpit takes its stage list from `LaunchContent`, which `Backend::launch_content()` already returns (`launch.rs:129`, `backend.rs:307`). The enum `Stage` (`launch.rs:6`) and every `Stage::ALL`, `Stage::label`, `Stage::index`, `Stage::activity`, and every fixed `[T; 11]` array in `LaunchRun` go away.

```rust
pub struct StageList(Vec<StageSpec>);        // non-empty
pub struct StageSpec {
    pub id: String,                // stable key; LaunchPlan targets use it ("network", "sidecar")
    pub label: String,             // rail label, "stage i of N · label", failure Prop
    pub activity: String,          // "{agent}" is replaced with Agent::label
    pub ticks: u64,                // demo driver duration (was LaunchRun.durations)
    pub kind: StageKind,           // Plain, BuildLog, Credentials
    pub skipped: bool,             // demo driver ends this stage Skipped
    pub container_at: Option<u64>, // demo driver emits ContainerReady at this tick
}
pub enum StageKind { Plain, BuildLog, Credentials { hold_at: u64 } }
pub struct LaunchWords {
    pub unit: String,          // "Container": Prop label, chip, hint "i {unit} info"
    pub unit_missing: String,  // "not created"
    pub log_title: String,     // "Docker build"; with " · building…" while running
    pub log_waiting: String,   // "(waiting for docker build output…)"
    pub build_failed: String,  // "Docker build failed"
    pub ready_activity: String // "Hardline open · handing off to the Capsule"
}
pub struct LaunchContent { pub stages: StageList, pub words: LaunchWords, pub build_log: Vec<String>, pub network_failure: LaunchFailure }
```

`hold_at` is the demo driver tick of the locked-credentials pause (the literal `10` in `LaunchRun::advance`). `LaunchEvent::StageChanged(usize, StepState)`, `LaunchFailure.stage: usize`, `LaunchRun.states: Vec<StepState>`. `StageList::position(id) -> Option<usize>`. A `LaunchPlan` whose target id is not in the list behaves as `Clean`. These strings replace the literals at `cockpit.rs:293` (`ready_activity`), `372` and `417` (`unit`, `unit_missing`), `377` (`build_failed`), `478-480` (`log_title`), `510` (`log_waiting`), `1022` (`i {unit} info`). The rail height, `stage i of N`, and the strip `d/N stages` use `stages.len()` (`cockpit.rs:784`, `869`, `1047`). The `Credentials` pause (`cockpit.rs:242`, `266`) and its dialog text use the stage whose `kind` is `Credentials` and its `label`.

### 7.2 Demo data

`src/bin/jackin/sim/launch.rs` `content()` builds the 11 existing stages with the exact current labels, activities, durations `[14, 18, 26, 30, 8, 92, 22, 20, 18, 34, 16]`, `kind` (`BuildLog` on `Derived Image`, `Credentials { hold_at: 10 }` on `Credentials`), `skipped` on `Agent Binaries`, `container_at: Some(6)` on `Construct`, and the current `LaunchWords`. Labels stay in the library's old Title Case in this data (`Derived Image`, `Agent Binaries`) because they must be byte-identical.

Scenario `launch-openshell` uses `SimBackend::openshell()` whose content is:

| id | label | activity | ticks | kind |
| --- | --- | --- | --- | --- |
| `image` | Image build | `Building the role image…` | 30 | BuildLog |
| `provider` | OpenShell provider | `Preparing the OpenShell provider…` | 10 | Plain |
| `sandbox` | Sandbox | `Creating the sandbox…` | 22 | Plain, `container_at: Some(6)` |
| `credentials` | Credentials | `Resolving credentials…` | 14 | Credentials { hold_at: 10 } |
| `upload` | Upload | `Uploading the project…` | 30 | Plain |
| `hooks` | Hooks | `Running the setup and preflight hooks…` | 18 | Plain |
| `codex` | Codex | `Starting {agent}…` | 16 | Plain |

Words: `unit` `Sandbox`, `unit_missing` `not created`, `log_title` `Image build`, `log_waiting` `(waiting for build output…)`, `build_failed` `Image build failed`, `ready_activity` `Codex is running · opening the session`. Build log: a short BuildKit transcript for a Codex role (about 12 lines, no Claude lines). Agent: Codex. Plan: `Clean`.

The seven rows are the brief's six stages plus `Credentials`, which the demo flow (1Password hold and retry) keeps. How the nine `LaunchStep`s collapse into rows is `jackin-tui`'s mapping (Q8).

### 7.3 Test `jackin_launch_openshell_stages`

Scenario `launch-openshell --motion paused --frame 90` (cumulative stage ticks 30, 40, 62, 76, 106, so frame 90 is inside `Upload`). Assert the seven labels, `stage 5 of 7 · Upload`, the strip `4/7 stages`, the chip `jackin-payment-b5df` (D07: `jackin-<slug7>-<suffix4>`, at most 19 characters) under the `Sandbox` word, a credentials block that lists the Codex account only and is not clipped, and none of `Docker`, `Capsule`, `Construct`, `Derived Image` below line 0. The shell draws the first screen line (the chrome strip, `inside the Construct` or `entering the Construct`, `app.rs:1919`). `Construct` is the base image name and the baseline's brand wording, not a Docker or Capsule word, so the test skips line 0 and forbids `Construct` on every other line. Press `b`; assert the log title `Image build`. `visual("functional/jackin_launch_openshell")`.

Regression proof (not a new test): `jackin_launch_reconnect`, `jackin_launch_failure` and `showcase_page_launchcockpit` pass unchanged, which shows the demo labels are byte-identical.

### 7.4 Acceptance

- No `Stage` enum and no array of length 11 remain: `grep -rn '\bStage::' src` finds nothing.
- A list of 1 or 20 stages renders without panic; the rail clips to the height as today (`rail_h`, `cockpit.rs:784`).
- The demo content gives the current output byte for byte (the three tests above).
- Gap recorded in section 8: the cockpit still advances through the library simulator `LaunchRun::advance`. Real `LaunchEvent`s need a driver seam.

## 8. Where the demo has no matching pattern, and deviations to flag

| # | Finding | Impact | Proposal |
| --- | --- | --- | --- |
| G1 | Nested pages. `open_page` pops to the tab root before it pushes (`app.rs:252`, `show_tab`). A page cannot open a second page and return to it. | The role editor cannot be opened from the workspace editor or Settings without dropping unsaved edits. | Open the role editor only from the Roles tab (this spec). Do not add a second entry from dirty pages. |
| G2 | Account precedence differs from D26 (section 3.6). | Role and workspace swap order. | `AccountOrder` data. |
| G3 | Demo role preference lives inside a workspace (`AccountPolicy.role_preferred`, `workspace.rs`). D26's role binding is global per role and agent. | Two places could say "role binding". | The new binding is global (`RoleRecord.bindings`). The per-workspace map stays for the demo; jackin leaves it empty. Q2. |
| G4 | Removing an account in the demo is allowed and falls back (`accounts.rs:985`). D26/`API.md` refuse with `AccountInUse` while a binding names it. | The remove flow differs. | With `surfaces.roles`, a refusal `InfoDialog` lists the bindings and says `Clear these bindings first.` Not specified further here. |
| G5 | No pre-launch confirmation or persistent warning pattern. | D24 needs a warning per launch. | Section 4.1: a detail line in the launch picker and a `Notice` in the cockpit. |
| G6 | The cockpit is driven only by `LaunchRun::advance`, a simulator in the library. There is no seam for real `LaunchEvent`s. | Stage labels from data are not enough for P9.3. | A `LaunchDriver` trait behind `Backend` (events in, `seek` stays in the sim). Needs its own task before P9.3. |
| G7 | The capsule panes draw `AgentProcess` lines in a `TextViewport`, not a `TerminalView`. `TerminalState` has no scrollback accessor. | Pane parity (wheel, copy, scroll) is open for P9.3. | Out of scope here. Q9. |
| G8 | The Settings `Agents` tab and the launch picker iterate `Agent::ALL` (six agents). jackin runs one. | They would list agents jackin cannot run. | Replace by `RoleAdmin.agents`-style data in a later change; not required for these screens. |
| G9 | `Go::Accounts` from Settings drops the Settings page, including unsaved edits (`app.rs`, `show_tab`). | Existing behaviour, not caused here. | Report to the owner; not changed. |
| G10 | Demo role error text names `role.toml` (`fixtures.rs:1451-1455`); D11 names the file `jackin.role.toml`. | Copy differs. | New fixtures use `jackin.role.toml`; the existing fixture is untouched. |

## 9. Component index (all exist in the library at `2565a0c`)

Layout and shell: `ui::master_detail::MasterDetail`, `ui::layout::{Stack, StackScroll, Split}`, `widgets::panel::Panel`, `widgets::tabs::{Tabs, TabItem}`, `shell::Router`, `widgets::segments::Segment`, `widgets::keyhint::{hint, Hint}`, `widgets::menu::{MenuBar, MenuItem}`, `widgets::help::{HelpOverlay, HelpSection}`, `widgets::too_small::TooSmall`.
Inputs: `widgets::input::TextInput`, `widgets::choice::{Checkbox, RadioGroup}`, `widgets::select::Select`, `widgets::button::{Button, ButtonRow, row_layout_right}`, `widgets::picker::Picker`, `widgets::option_list::Item`.
Content: `widgets::props::{Prop, PropsList}`, `widgets::notice::{Notice, NoticeKind}`, `widgets::empty::{EmptyState, EmptyAction}`, `widgets::steps::{Step, StepRail, StepState}`, `widgets::viewport::{TextViewport, Span}`, `widgets::scrollbar`, `theme::StatusKind`, `ui::text::{fit, truncate, plural}`.
Overlays: `widgets::dialog::Dialog` (`confirm`, `destructive`, `facts`, `title_status`, `actions`), `widgets::info_dialog::InfoDialog`, `widgets::form_dialog::{FormDialog, FormField}`, `console::screens::modals::file_browser`, `ui::leave::{LeaveGuard, Stakes}`.
Console blocks: `console::screens::config::ConfigTabs` (list pattern), `console::screens::editor::EditorScreen`, `console::screens::accounts::AccountsScreen`, `console::screens::cockpit::CockpitScreen`, `console::screens::rowmenu::RowMenu`, `console::world::World`, `console::backend::{Backend, Effect}`, `console::launch::{LaunchContent, LaunchRun}`.
Terminal feature: `terminal::{TerminalState, TerminalView, Modes, encode_key}`.
Showcase: `Template`, `Section`, `page_options`, `ui::specimen::{StateGrid, SpecimenState}`.

New blocks introduced by this spec (they do not exist yet and are the deliverable): `RolesScreen`, `RoleEditorScreen`, `DoctorScreen`, `Surfaces` and its data structs, `StageList`, `StageSpec`, `LaunchWords`, `TerminalPanePage`, and the shared `unsaved_dialog` function.

## 10. Open questions

- **Q1. Keys.** `R` and `D` (capitals) for Roles and Doctor. Alternative: no global letter, menu and Manager actions only. Owner choice.
- **Q2. One role binding or two.** Keep the per-workspace `role_preferred` map the demo has, or remove it for jackin and make the global binding the only role-level choice (D26)? The spec keeps both and leaves jackin's map empty.
- **Q3. Precedence order.** `AccountOrder::Jackin` changes only new screens. Should the launch picker also use D26's order in jackin? It must, for P9.4 to show the same account that the launch uses.
- **Q4. `jackin role check` reports one error at a time.** The panel can only show what the command shows. Should `jackin-session` return all manifest and file errors in one pass? That shortens the fix loop.
- **Q5. Default feature set.** The showcase needs `terminal`; the spec adds it to `default`. Alternative: a showcase-only feature, which means the functional suite must build with `--features terminal`.
- **Q6. Docker warning placement** in the launch picker and cockpit (section 4.1) is a proposal; D24 only requires "a warning for each launch".
- **Q7. Launch failures that name the gateway** should offer `Open doctor`. It needs a field on `LaunchFailure`; not specified here.
- **Q8. Rail rows.** Six, seven, or nine (`LaunchStep::ALL`) rows in jackin? The spec shows seven in the preview.
- **Q9. Scrollback in `TerminalState`.** P9.3 needs wheel scroll and copy in panes. Add an accessor in a follow-up baseline task, before P9.3.
- **Q10. Creating a role from nothing** (scaffold files) has no CLI command in the table and no screen here; `Add role…` registers an existing directory or git URL only.
- **Q11. Manifest write path.** `API.md` lists `AddRole`, `RemoveRole`, `BindRoleAccount` as the role edits; it has no call that writes `jackin.role.toml` or checks a staged manifest. `Effect::SaveRole` and `Effect::CheckRole { doc }` need `jackin-session` functions that do (the check cannot read the staged doc from disk).

## 11. Gate checklist

Each cell names the section that holds the content.

| Item | Entry and route | Layout and components | States | Key map | Data interface | Demo data and scenario | Functional test | Acceptance |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| 1 Role editor | 2.1 | 2.2, 2.3 | 2.4 | 2.5 | 2.6 | 2.7 | 2.8 | 2.9 |
| 2 Role binding | 3.1 | 3.2 | 3.3 | 3.4 | 3.5, 3.6 | 3.7 | 3.8 | 3.9 |
| 3 Docker setting | 4.1 | 4.2 | 4.3 | 4.4 | 4.5 | 4.6 | 4.7 | 4.8 |
| 4 Doctor panel | 5.1 | 5.2 | 5.3 | 5.4 | 5.5 | 5.6 | 5.7 | 5.8 |
| 5 Terminal pane page | 6.1 | 6.1 | 6.4 | 6.3 | 6.2 | 6.2 | 6.5 | 6.6 |
| 6 Stage list | 7.0 | 7.1 | 7.0 | 7.0 | 7.1 | 7.2 | 7.3 | 7.4 |
