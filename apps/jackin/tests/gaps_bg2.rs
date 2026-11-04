//! Step BG2: the gaps the jackin persistence step (J2) found. Each test states
//! one gap in terms of what the operator sees or what the baseline words. The
//! gaps and their API are in `docs/P9.3-red-green.md`, section BG2.
#![cfg(feature = "demo")]

mod common;

use std::sync::{Arc, Mutex};

use common::*;
use jackin_console::app::App;
use jackin_console::backend::{Backend, Completion, Effect, Reply};
use jackin_console::domain::account::{AccountRegistry, Lifecycle};
use jackin_console::domain::agent::Agent;
use jackin_console::domain::fixed::Fixed;
use jackin_console::domain::instance::{Instance, RepoRef};
use jackin_console::domain::launch::LaunchContent;
use jackin_console::domain::role::{FindingArea, RoleCheckState, RoleFinding};
use jackin_console::domain::vocabulary::{Rejection, SaveField, Unbuilt, Vocabulary, fill_in};
use jackin_console::domain::workspace::Workspace;
use jackin_console::fixtures::{Scenario, app_with_world, world_for};
use jackin_console::launch::{LaunchFeed, LaunchTarget};
use jackin_console::panes::{PaneBody, PaneRequest, PaneSpawn};
use jackin_console::sim::{self, SimBackend};
use jackin_console::world::{Msg, World};
use junie_tui::core::event::{Input, Key};
use junie_tui::motion::Motion;
use junie_tui::runtime::Application;
use junie_tui::theme::{ColorLevel, Theme};
use ratatui::crossterm::event::{KeyCode, KeyModifiers};

type Answer = fn(&Effect, &World) -> Option<Msg>;

/// The simulator, except that one function may answer an effect first. Every
/// effect and every launch target is recorded.
#[derive(Debug)]
struct Recording {
    inner: Arc<dyn Backend>,
    answer: Answer,
    seen: Mutex<Vec<Effect>>,
    launched: Mutex<Vec<LaunchTarget>>,
}

impl Recording {
    fn new(answer: Answer) -> Arc<Self> {
        Arc::new(Self {
            inner: SimBackend::shared(),
            answer,
            seen: Mutex::new(vec![]),
            launched: Mutex::new(vec![]),
        })
    }

    fn seen(&self) -> Vec<Effect> {
        let mut all = self.seen.lock().expect("seen").clone();
        all.retain(|e| *e != Effect::Refresh);
        all
    }

    fn launched(&self) -> Vec<LaunchTarget> {
        self.launched.lock().expect("launched").clone()
    }
}

impl Backend for Recording {
    fn perform(&self, effect: &Effect, world: &World) -> Option<Completion> {
        self.seen.lock().expect("seen").push(effect.clone());
        match (self.answer)(effect, world) {
            Some(msg) => Some(Completion::ready(0, msg)),
            None => self.inner.perform(effect, world),
        }
    }

    fn settle(&self, effect: &Effect, world: &World) -> Option<Msg> {
        self.inner.settle(effect, world)
    }

    fn launch_content(&self) -> Arc<LaunchContent> {
        self.inner.launch_content()
    }

    fn launch(&self, world: &World, target: &LaunchTarget) -> Box<dyn LaunchFeed> {
        self.launched.lock().expect("launched").push(target.clone());
        self.inner.launch(world, target)
    }

    fn spawn_pane(&self, request: &PaneRequest, spawn: PaneSpawn) -> Box<dyn PaneBody> {
        self.inner.spawn_pane(request, spawn)
    }

    fn initial_panes(&self, instance: &Instance) -> Vec<PaneRequest> {
        self.inner.initial_panes(instance)
    }
}

fn no_answer(_: &Effect, _: &World) -> Option<Msg> {
    None
}

fn timed(scenario: Scenario, world: World) -> App {
    app_with_world(
        scenario,
        world,
        Motion::Reduced,
        10_000,
        Theme::for_level(ColorLevel::TrueColor),
    )
}

fn settle(app: &mut App) -> String {
    for _ in 0..40 {
        Application::handle(app, Input::Tick);
    }
    screen(app)
}

fn status(app: &App) -> String {
    app.shell.status.text().unwrap_or_default().to_owned()
}

fn keys_for(text: &str) -> String {
    text.chars()
        .map(|c| c.to_string())
        .collect::<Vec<_>>()
        .join(" ")
}

fn has(shown: &str, text: &str) {
    assert!(shown.contains(text), "missing `{text}` in:\n{shown}");
}

fn lacks(shown: &str, text: &str) {
    assert!(!shown.contains(text), "unexpected `{text}` in:\n{shown}");
}

// Gap 1: Fixed.save_hint, the editor save status that asks for a default role.

/// The workspace with no default role is the fourth saved one (`customer-portal`).
const NO_DEFAULT_ROLE: &str = "down down down down";

fn save_renamed(world: World, name: &str) -> String {
    let mut app = timed(Scenario::OpenshellFixed, world);
    press(
        &mut app,
        &format!("{NO_DEFAULT_ROLE} e enter {} ctrl+s", keys_for(name)),
    );
    press(&mut app, "right enter");
    settle(&mut app);
    status(&app)
}

fn hint_for(name: &str) -> String {
    fill_in(&Fixed::openshell().save_hint, &[("name", name)])
}

#[test]
fn editor_save_status_asks_for_a_default_role() {
    let hint = Fixed::openshell().save_hint;
    assert!(
        hint.contains("{name}") && hint.contains("default role"),
        "{hint}"
    );
    assert!(
        junie_tui::ui::text::width(&fill_in(&hint, &[("name", &"x".repeat(20))])) <= 60,
        "the hint fits the status line at the longest name"
    );

    let world = world_for(Scenario::OpenshellFixed);
    assert!(world.roles.len() > 1);
    assert_eq!(save_renamed(world, "x"), hint_for("x"));

    let long = save_renamed(
        world_for(Scenario::OpenshellFixed),
        "abcdefghijklmnopqrstuvwxy",
    );
    assert_eq!(long, hint_for("abcdefghijklmnopqrs…"));
    assert!(junie_tui::ui::text::width(&long) <= 56, "{long}");

    let mut one_role = world_for(Scenario::OpenshellFixed);
    one_role.roles.truncate(1);
    assert_eq!(save_renamed(one_role, "x"), "Workspace x saved");

    let mut with_default = world_for(Scenario::OpenshellFixed);
    with_default.workspaces[3].roles.default = Some("writer".into());
    assert_eq!(save_renamed(with_default, "x"), "Workspace x saved");
}

#[test]
fn the_demo_save_status_is_unchanged() {
    let mut app = timed(Scenario::Returning, world_for(Scenario::Returning));
    press(
        &mut app,
        &format!("{NO_DEFAULT_ROLE} e enter x ctrl+s right enter"),
    );
    settle(&mut app);
    assert_eq!(status(&app), "Workspace x saved");
}

// Gap 2: Workspace.github, so a saved workspace with no instance opens its repo.

fn repo(name: &str) -> RepoRef {
    RepoRef {
        full_name: name.into(),
        url: format!("https://github.com/{name}"),
    }
}

#[test]
fn a_saved_workspace_with_no_instance_opens_its_repo() {
    let mut world = world_for(Scenario::Returning);
    let mut solo = Workspace::new(77, "solo", "/workspace/solo");
    solo.github = Some(repo("acme/solo"));
    let bare = Workspace::new(78, "bare", "/workspace/bare");
    world.workspaces.push(solo);
    world.workspaces.push(bare);
    assert!(world.instances.iter().all(|i| i.workspace != Some(77)));

    assert_eq!(world.workspace_repo(77), Some(&repo("acme/solo")));
    assert_eq!(world.workspace_repo(78), None);

    let opened = fill_in(
        &world.surfaces.vocabulary.github_workspace,
        &[("url", "https://github.com/acme/solo")],
    );
    let mut app = timed(Scenario::Returning, world);
    press(&mut app, "down down down down down o");
    assert_eq!(status(&app), opened);
    press(&mut app, "down o");
    assert_eq!(status(&app), "No GitHub source for bare");
}

#[test]
fn an_instance_no_longer_supplies_the_workspace_repo() {
    let mut world = world_for(Scenario::Returning);
    let id = world.workspaces[0].id;
    world.workspaces[0].github = None;
    assert!(
        world
            .instances
            .iter()
            .any(|i| i.workspace == Some(id) && i.github.is_some())
    );
    assert_eq!(world.workspace_repo(id), None);
}

#[test]
fn every_demo_workspace_keeps_the_repo_its_instances_gave() {
    for scenario in Scenario::ALL {
        let world = world_for(scenario);
        for ws in &world.workspaces {
            let from_instances = world
                .instances
                .iter()
                .filter(|i| i.workspace == Some(ws.id))
                .find_map(|i| i.github.as_ref());
            assert_eq!(
                world.workspace_repo(ws.id),
                from_instances,
                "{} · {}",
                scenario.name(),
                ws.name
            );
        }
    }
}

// Gap 3: the wizard creates a workspace the Fixed checkbox can show.

fn wizard_world(fixed: bool) -> World {
    let mut world = world_for(Scenario::Returning);
    world.surfaces.vocabulary = Vocabulary::openshell();
    world.surfaces.wizard = Some(jackin_console::domain::workspace::WizardDefaults::openshell());
    world.surfaces.fixed = fixed.then(Fixed::openshell);
    world
}

fn created_git_pull(world: World) -> bool {
    let backend = Recording::new(no_answer);
    let mut world = world;
    world.backend = backend.clone();
    let mut app = timed(Scenario::Returning, world);
    press(&mut app, "n down space enter enter enter ctrl+s");
    press(&mut app, "right enter");
    settle(&mut app);
    let seen = backend.seen();
    let [Effect::SaveWorkspace { id, workspace, .. }] = seen.as_slice() else {
        panic!("one SaveWorkspace expected, got {seen:?}");
    };
    assert_eq!(*id, None);
    workspace.git_pull
}

#[test]
fn the_fixed_wizard_creates_a_workspace_with_git_pull_off() {
    assert!(
        !created_git_pull(wizard_world(true)),
        "the wizard made a workspace with git_pull on, the Fixed box says always off"
    );
}

#[test]
fn the_demo_wizard_keeps_git_pull_on() {
    assert!(
        created_git_pull(wizard_world(false)),
        "the demo wizard keeps git_pull on"
    );
}

// Gap 4: account ids come from the backend.

const NEW_ACCOUNT: &str = "c a enter S p a r e tab tab tab down down tab enter s k - a n t - v a l i d - a b c d e f 1 2 3 4 tab";
const GIVEN: &str = "backend-given-7";

/// Stores the registry the way a backend would: the account the form sent
/// without an id gets one, and the reply names it.
fn give_an_id(effect: &Effect, world: &World) -> Option<Msg> {
    let Effect::SaveAccounts { registry, .. } = effect else {
        return None;
    };
    let mut snapshot = sim::snapshot(world);
    snapshot.accounts = (**registry).clone();
    let mut added = None;
    for account in &mut snapshot.accounts.accounts {
        if account.is_unsaved() {
            account.id = GIVEN.into();
            added = Some(GIVEN.to_owned());
        }
    }
    Some(Msg::AccountsSaved {
        added,
        result: Ok(Box::new(snapshot)),
    })
}

fn names(registry: &AccountRegistry) -> Vec<(&str, &str)> {
    registry
        .accounts
        .iter()
        .map(|a| (a.id.as_str(), a.display_name.as_str()))
        .collect()
}

#[test]
fn an_account_gets_its_id_from_the_backend() {
    let backend = Recording::new(give_an_id);
    let mut world = world_for(Scenario::AccountsMixed);
    world.backend = backend.clone();
    let mut app = timed(Scenario::AccountsMixed, world);
    press(&mut app, NEW_ACCOUNT);
    click_text(&mut app, "Save");
    settle(&mut app);

    let seen = backend.seen();
    let sent = seen
        .iter()
        .find_map(|e| match e {
            Effect::SaveAccounts { registry, .. } => Some(registry.clone()),
            _ => None,
        })
        .expect("one SaveAccounts");
    let new: Vec<_> = sent
        .accounts
        .iter()
        .filter(|a| a.display_name == "Spare")
        .collect();
    assert_eq!(new.len(), 1, "{:?}", names(&sent));
    assert!(
        new[0].is_unsaved(),
        "the form must not mint an id, it sent `{}`",
        new[0].id
    );

    assert!(
        names(&app.world.accounts).contains(&(GIVEN, "Spare")),
        "{:?}",
        names(&app.world.accounts)
    );
    assert_eq!(status(&app), "Saved Claude · Spare");
    press(&mut app, "r");
    settle(&mut app);
    let refreshed: Vec<_> = backend
        .seen()
        .into_iter()
        .filter_map(|e| match e {
            Effect::RefreshAccount { account, .. } => Some(account),
            _ => None,
        })
        .collect();
    assert_eq!(refreshed, [GIVEN], "the saved account is the selected one");
}

// Gap 5: no account is preselected when the user has to choose.

fn two_ready_claude_accounts() -> World {
    let mut world = world_for(Scenario::Returning);
    for account in &mut world.accounts.accounts {
        account.default_for_provider = false;
    }
    let ws = &mut world.workspaces[0];
    ws.accounts.enabled.insert("acct-claude-personal".into());
    ws.accounts.enabled.insert("acct-claude-work".into());
    ws.accounts.role_preferred.clear();
    world
}

#[test]
fn two_ready_accounts_with_no_default_preselect_none() {
    let world = two_ready_claude_accounts();
    let ws = &world.workspaces[0];
    let offer = world.offer_for(Agent::ClaudeCode, Some(ws), Some("the-architect"));
    assert_eq!(
        offer.accounts,
        ["acct-claude-personal", "acct-claude-work"],
        "both stay offered"
    );
    assert_eq!(offer.preselected, None);
    assert_eq!(offer.blocked, None);

    let mut default = two_ready_claude_accounts();
    default
        .accounts
        .accounts
        .iter_mut()
        .find(|a| a.id == "acct-claude-work")
        .expect("work")
        .default_for_provider = true;
    let ws = &default.workspaces[0];
    let offer = default.offer_for(Agent::ClaudeCode, Some(ws), Some("the-architect"));
    assert_eq!(offer.preselected.as_deref(), Some("acct-claude-work"));
    assert_eq!(offer.accounts[0], "acct-claude-work");

    let mut preferred = two_ready_claude_accounts();
    preferred.workspaces[0].accounts.preferred.insert(
        jackin_console::domain::agent::Provider::Anthropic,
        "acct-claude-work".into(),
    );
    let ws = &preferred.workspaces[0];
    let offer = preferred.offer_for(Agent::ClaudeCode, Some(ws), Some("the-architect"));
    assert_eq!(offer.preselected.as_deref(), Some("acct-claude-work"));

    let mut only = two_ready_claude_accounts();
    only.workspaces[0]
        .accounts
        .enabled
        .remove("acct-claude-personal");
    let ws = &only.workspaces[0];
    let offer = only.offer_for(Agent::ClaudeCode, Some(ws), Some("the-architect"));
    assert_eq!(offer.accounts, ["acct-claude-work"]);
    assert_eq!(offer.preselected.as_deref(), Some("acct-claude-work"));
}

#[test]
fn an_unusable_explicit_account_is_not_replaced_by_the_one_ready_account() {
    let mut world = two_ready_claude_accounts();
    world.workspaces[0].accounts.preferred.insert(
        jackin_console::domain::agent::Provider::Anthropic,
        "acct-claude-work".into(),
    );
    world
        .accounts
        .accounts
        .iter_mut()
        .find(|a| a.id == "acct-claude-work")
        .expect("work")
        .lifecycle = Lifecycle::NeedsLogin;
    let ws = &world.workspaces[0];
    let offer = world.offer_for(Agent::ClaudeCode, Some(ws), Some("the-architect"));

    assert_eq!(
        offer.preselected, None,
        "the operator chose another account"
    );
    let why = offer
        .blocked
        .expect("the chosen account is shown as blocked");
    let work = world
        .accounts
        .get("acct-claude-work")
        .expect("work")
        .title();
    assert!(why.starts_with(&work), "{why}");
}

#[test]
fn no_demo_world_offers_several_accounts_without_a_choice() {
    for scenario in Scenario::ALL {
        let world = world_for(scenario);
        let mut roles: Vec<Option<String>> = vec![None];
        roles.extend(world.roles.iter().take(8).map(|r| Some(r.name.clone())));
        let mut spaces = vec![None];
        spaces.extend(world.workspaces.iter().map(Some));
        for ws in spaces {
            for role in &roles {
                for (agent, offer) in world.offered_agents(ws, role.as_deref()) {
                    if offer.accounts.len() > 1 {
                        assert!(
                            offer.preselected.is_some(),
                            "{} · {:?} · {role:?} · {agent:?}",
                            scenario.name(),
                            ws.map(|w| &w.name)
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn the_user_picks_the_account_when_none_is_preselected() {
    let backend = Recording::new(no_answer);
    let mut world = two_ready_claude_accounts();
    world.backend = backend.clone();
    let mut app = timed(Scenario::Returning, world);
    press(&mut app, "down enter");
    let picker = screen(&mut app);
    has(&picker, "Claude Code");
    has(&picker, "2 accounts · choose at start");
    lacks(&picker, "no usable account");

    press(&mut app, "enter");
    let accounts = screen(&mut app);
    has(&accounts, "Account for Claude Code");
    has(&accounts, "Personal");
    has(&accounts, "Work");
    lacks(&accounts, "preselected");
    assert!(
        backend.launched().is_empty(),
        "nothing starts before the pick"
    );

    press(&mut app, "down enter");
    settle(&mut app);
    let launched = backend.launched();
    let [target] = launched.as_slice() else {
        panic!("one launch expected, got {launched:?}");
    };
    assert_eq!(target.agent, Agent::ClaudeCode);
    assert_eq!(target.account.as_deref(), Some("acct-claude-work"));
}

#[test]
fn a_preselected_account_launches_without_a_second_pick() {
    let backend = Recording::new(no_answer);
    let mut world = world_for(Scenario::Returning);
    world.backend = backend.clone();
    let mut app = timed(Scenario::Returning, world);
    press(&mut app, "down enter enter");
    settle(&mut app);
    let launched = backend.launched();
    let [target] = launched.as_slice() else {
        panic!("one launch expected, got {launched:?}");
    };
    assert_eq!(target.account.as_deref(), Some("acct-claude-personal"));
}

// Gap 6: RoleFinding.fix and key are optional.

fn finding(key: Option<&str>, message: &str, fix: Option<&str>) -> RoleFinding {
    RoleFinding {
        area: FindingArea::Policy,
        key: key.map(str::to_owned),
        message: message.into(),
        fix: fix.map(str::to_owned),
    }
}

fn without_fixes(effect: &Effect, world: &World) -> Option<Msg> {
    found(
        effect,
        world,
        vec![
            finding(None, "first problem", None),
            finding(Some("role.policy"), "second problem", None),
            finding(None, "third problem", None),
        ],
    )
}

fn with_a_fix(effect: &Effect, world: &World) -> Option<Msg> {
    found(
        effect,
        world,
        vec![finding(
            Some("role.policy"),
            "only problem",
            Some("repair the policy"),
        )],
    )
}

fn found(effect: &Effect, world: &World, findings: Vec<RoleFinding>) -> Option<Msg> {
    let Effect::CheckRole { ticket, role, .. } = effect else {
        return None;
    };
    let mut snapshot = sim::snapshot(world);
    let rec = snapshot.surfaces.roles.as_mut()?.role_mut(role)?;
    rec.check = RoleCheckState::Done(findings);
    Some(Msg::RoleChecked {
        ticket: *ticket,
        role: role.clone(),
        result: Ok(Box::new(snapshot)),
    })
}

fn row_of(shown: &str, text: &str) -> usize {
    shown
        .lines()
        .position(|l| l.contains(text))
        .unwrap_or_else(|| panic!("missing `{text}` in:\n{shown}"))
}

fn checked(answer: Answer) -> String {
    let mut world = world_for(Scenario::RoleEditor);
    world.backend = Recording::new(answer);
    let mut app = timed(Scenario::RoleEditor, world);
    press(&mut app, "enter");
    Application::handle(
        &mut app,
        Input::Key(Key {
            code: KeyCode::F(5),
            mods: KeyModifiers::NONE,
        }),
    );
    settle(&mut app)
}

#[test]
fn a_finding_draws_its_fix_only_when_it_has_one() {
    let bare = checked(without_fixes);
    let first = row_of(&bare, "! first problem");
    assert_eq!(
        row_of(&bare, "! second problem"),
        first + 1,
        "no empty hint row under a finding without a fix:\n{bare}"
    );
    assert_eq!(row_of(&bare, "! third problem"), first + 2, "{bare}");

    let fixed = checked(with_a_fix);
    let only = row_of(&fixed, "! only problem");
    assert_eq!(row_of(&fixed, "repair the policy"), only + 1, "{fixed}");
}

// Gap 7: the short error sentences jackin wrote itself.

#[test]
fn the_rejection_sentences_are_the_vocabularys() {
    let live = Vocabulary::openshell();
    let cases = [
        (
            Rejection::CannotSave(SaveField::Workdir),
            "workdir cannot be saved",
        ),
        (
            Rejection::CannotSave(SaveField::KeepAwake),
            "keep_awake cannot be saved",
        ),
        (
            Rejection::CannotSave(SaveField::GlobalMounts),
            "global mounts cannot be saved",
        ),
        (Rejection::NoProject, "a workspace needs a project folder"),
        (
            Rejection::NotAFolder {
                folder: "~/src/gone".into(),
            },
            "~/src/gone is not a folder jackin can use",
        ),
        (Rejection::NoRoleLeft, "no allowed role is left"),
        (
            Rejection::NotInRoster {
                instance: "jk-ab12".into(),
            },
            "jk-ab12 is not in the roster",
        ),
        (
            Rejection::UnknownRole {
                role: "ghost".into(),
            },
            "unknown role ghost",
        ),
        (
            Rejection::RoleNotLoaded,
            "the role manifest is read when the role is loaded",
        ),
    ];
    for (rejection, text) in cases {
        assert_eq!(live.rejection(&rejection), text, "{rejection:?}");
    }

    let demo = Vocabulary::demo();
    assert_eq!(
        demo.rejection(&Rejection::CannotSave(SaveField::Workdir)),
        "workdir is not stored by the preview"
    );
    assert_eq!(
        demo.rejection(&Rejection::UnknownRole {
            role: "ghost".into()
        }),
        "ghost is not in the registry"
    );
}

#[test]
fn every_saveable_field_has_a_label() {
    use SaveField::*;
    let all = [
        Workdir,
        Mounts,
        KeepAwake,
        GitPull,
        DirtyPolicy,
        DisabledDefaults,
        EnabledAccounts,
        RoleAccounts,
        CoauthorTrailer,
        DcoSignoff,
        GlobalMounts,
        GlobalRoleVariables,
        AgentModes,
        Trust,
        AccountName,
        AccountProvider,
        Purpose,
        Endpoint,
        EnvSource,
        ApiKey,
        Agent,
        RoleSource,
        RoleName,
    ];
    let labels: std::collections::BTreeSet<_> = all.iter().map(|f| f.label()).collect();
    assert_eq!(labels.len(), all.len(), "each field has its own label");
    assert!(labels.iter().all(|l| !l.is_empty()));
}

#[test]
fn adding_a_role_from_a_git_url_is_an_unbuilt_sentence() {
    assert_eq!(
        Vocabulary::openshell().unbuilt(Unbuilt::AddGitRole),
        "Adding a role from a Git URL is not available in this build yet"
    );
    assert_eq!(
        Vocabulary::demo().unbuilt(Unbuilt::AddGitRole),
        "Adding a role from a Git URL is not simulated in the preview"
    );
}

fn simulated(effect: &Effect, world: &World) -> Msg {
    let sim = SimBackend::shared();
    match sim.perform(effect, world).expect("a completion").reply {
        Reply::Ready(msg) => msg,
        Reply::Settle => sim.settle(effect, world).expect("a settled reply"),
    }
}

#[test]
fn the_simulator_words_a_missing_instance_or_role_from_the_vocabulary() {
    let mut world = world_for(Scenario::Returning);
    world.surfaces.vocabulary.not_in_roster = "{id} left the roster".into();
    world.surfaces.vocabulary.unknown_role = "no role called {role}".into();
    let restore = Effect::Restore {
        instance: "jk-nope".into(),
    };
    let Msg::Restored { result, .. } = simulated(&restore, &world) else {
        panic!("a Restored reply expected");
    };
    assert_eq!(result.err().as_deref(), Some("jk-nope left the roster"));

    let trust = Effect::TrustRole {
        role: "ghost".into(),
    };
    let Msg::RoleTrusted { result, .. } = simulated(&trust, &world) else {
        panic!("a RoleTrusted reply expected");
    };
    assert_eq!(result.err().as_deref(), Some("no role called ghost"));

    let remove = Effect::RemoveRole {
        role: "ghost".into(),
    };
    let Msg::RoleRemoved { result, .. } = simulated(&remove, &world) else {
        panic!("a RoleRemoved reply expected");
    };
    assert_eq!(result.err().as_deref(), Some("no role called ghost"));
}

// Gap 8: the purge refusal names the forced purge as the next step.

fn refused_purge(world: World) -> String {
    let mut app = timed(Scenario::Returning, world);
    screen(&mut app);
    app.deliver(&Msg::Purged {
        instance: "jk-c41e".into(),
        result: Err("2 unpushed commits would be lost".into()),
    });
    status(&app)
}

#[test]
fn the_openshell_purge_refusal_names_the_forced_purge() {
    let mut world = world_for(Scenario::Returning);
    world.surfaces.vocabulary = Vocabulary::openshell();
    let shown = refused_purge(world);
    assert!(
        shown.starts_with("2 unpushed commits would be lost"),
        "{shown}"
    );
    assert!(shown.contains("jackin purge --force"), "{shown}");
    assert_eq!(
        Vocabulary::demo().lifecycle.purge_refused_next,
        None,
        "the demo offers no forced purge"
    );
}

#[test]
fn the_demo_purge_refusal_is_the_reason_alone() {
    assert_eq!(
        refused_purge(world_for(Scenario::Returning)),
        "2 unpushed commits would be lost"
    );
}
