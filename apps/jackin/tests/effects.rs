#![cfg(feature = "demo")]

mod common;

use std::sync::{Arc, Mutex};

use common::*;
use jackin_console::app::{App, Remote, Route};
use jackin_console::backend::{Backend, Completion, Effect, FileSource, Refresh};
use jackin_console::domain::account::{
    AccountRegistry, CredentialSource, DetectedKind, ValidationState,
};
use jackin_console::domain::agent::Agent;
use jackin_console::domain::instance::{Instance, InstanceStatus};
use jackin_console::domain::launch::LaunchContent;
use jackin_console::domain::reason::{IssueReason, RefreshOutcome};
use jackin_console::domain::role::{FindingArea, RoleAdmin, RoleCheckState, RoleFinding};
use jackin_console::domain::usage::{Freshness, FreshnessInfo};
use jackin_console::domain::workspace::RoleSource;
use jackin_console::fixtures::{Scenario, app_with_world, world_for};
use jackin_console::launch::{LaunchFeed, LaunchTarget};
use jackin_console::panes::{PaneBody, PaneRequest, PaneSpawn};
use jackin_console::screens::Go;
use jackin_console::sim::{self, SimBackend};
use jackin_console::world::{GlobalConfig, Msg, SaveFailure, World};
use junie_tui::core::event::{External, Input};
use junie_tui::motion::Motion;
use junie_tui::runtime::Application;
use junie_tui::theme::{ColorLevel, Theme};
use junie_tui::widgets::file_browser::FileEntry;

type Answer = fn(&Effect, &World) -> Option<Msg>;

/// A backend that records every effect it receives and answers with its own
/// function, the way a real backend would. Effects it does not answer go to
/// the simulator.
#[derive(Debug)]
struct TestBackend {
    inner: Arc<dyn Backend>,
    seen: Mutex<Vec<Effect>>,
    answer: Answer,
    unanswered: fn(&Effect) -> bool,
    files: Option<fn() -> Box<dyn FileSource>>,
}

impl TestBackend {
    fn new(answer: Answer) -> Arc<Self> {
        Arc::new(Self {
            inner: SimBackend::shared(),
            seen: Mutex::new(vec![]),
            answer,
            unanswered: |_| false,
            files: None,
        })
    }

    /// Every effect received except the manager's periodic refresh.
    fn seen(&self) -> Vec<Effect> {
        let mut all = self.seen.lock().expect("seen").clone();
        all.retain(|e| *e != Effect::Refresh);
        all
    }

    fn refreshes(&self) -> usize {
        let all = self.seen.lock().expect("seen");
        all.iter().filter(|e| **e == Effect::Refresh).count()
    }
}

impl Backend for TestBackend {
    fn perform(&self, effect: &Effect, world: &World) -> Option<Completion> {
        self.seen.lock().expect("seen").push(effect.clone());
        if (self.unanswered)(effect) {
            return None;
        }
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
        self.inner.launch(world, target)
    }

    fn spawn_pane(&self, request: &PaneRequest, spawn: PaneSpawn) -> Box<dyn PaneBody> {
        self.inner.spawn_pane(request, spawn)
    }

    fn initial_panes(&self, instance: &Instance) -> Vec<PaneRequest> {
        self.inner.initial_panes(instance)
    }

    fn file_source(&self, world: &World) -> Box<dyn FileSource> {
        match self.files {
            Some(files) => files(),
            None => self.inner.file_source(world),
        }
    }
}

fn backed(scenario: Scenario, answer: Answer) -> (App, Arc<TestBackend>) {
    let backend = TestBackend::new(answer);
    let mut world = world_for(scenario);
    world.backend = backend.clone();
    (timed(scenario, world), backend)
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

fn settle(app: &mut App) {
    for _ in 0..40 {
        Application::handle(app, Input::Tick);
    }
}

fn status(app: &App) -> String {
    app.shell.status.text().unwrap_or_default().to_owned()
}

const CANONICAL: &str = "/workspace/backend-canonical";

/// Stores the payload the way a backend would: the record keeps the id it was
/// given or 42, and the backend normalises the working directory.
fn store_workspace(effect: &Effect, world: &World) -> Option<Msg> {
    let Effect::SaveWorkspace { id, workspace, .. } = effect else {
        return None;
    };
    let mut stored = (**workspace).clone();
    stored.id = id.unwrap_or(42);
    stored.workdir = CANONICAL.into();
    let mut snapshot = sim::snapshot(world);
    match snapshot.workspaces.iter_mut().find(|w| w.id == stored.id) {
        Some(slot) => *slot = stored.clone(),
        None => snapshot.workspaces.push(stored.clone()),
    }
    Some(Msg::WorkspaceSaved {
        id: Some(stored.id),
        result: Ok(Box::new(snapshot)),
    })
}

fn fail_workspace(effect: &Effect, _: &World) -> Option<Msg> {
    let Effect::SaveWorkspace { id, .. } = effect else {
        return None;
    };
    Some(Msg::WorkspaceSaved {
        id: *id,
        result: Err(SaveFailure {
            error: "disk full".into(),
            next: None,
        }),
    })
}

#[test]
fn save_workspace_stores_the_payload() {
    let (mut app, backend) = backed(Scenario::Returning, store_workspace);
    let original = app.world.workspaces[0].clone();
    press(&mut app, "down e enter x ctrl+s");
    press(&mut app, "right enter");
    settle(&mut app);

    let seen = backend.seen();
    let [
        Effect::SaveWorkspace {
            id,
            base,
            workspace,
        },
    ] = seen.as_slice()
    else {
        panic!("one SaveWorkspace expected, got {seen:?}");
    };
    assert_eq!(*id, Some(original.id));
    assert_eq!(workspace.name, "x");
    assert_eq!(
        base.as_deref().map(|b| b.name.as_str()),
        Some(original.name.as_str())
    );

    assert_eq!(status(&app), "Workspace x saved");
    let roster: Vec<(&str, &str)> = app
        .world
        .workspaces
        .iter()
        .map(|w| (w.name.as_str(), w.workdir.as_str()))
        .collect();
    assert!(roster.contains(&("x", CANONICAL)), "{roster:?}");
    assert!(
        !roster.contains(&("x", original.workdir.as_str())),
        "the roster must be the reply snapshot, not the pending copy: {roster:?}"
    );
}

#[test]
fn failed_save_keeps_the_input() {
    let (mut app, _) = backed(Scenario::Returning, fail_workspace);
    let before = app.world.workspaces.clone();
    press(&mut app, "down e enter x ctrl+s");
    press(&mut app, "right enter");
    settle(&mut app);

    let shown = screen(&mut app);
    assert!(shown.contains("Save failed"), "{shown}");
    assert!(shown.contains("disk full"), "{shown}");
    assert!(
        !shown.contains("EACCES") && !shown.contains("not writable"),
        "the dialog must show the reply text, not the canned one:\n{shown}"
    );
    assert!(shown.contains("your edits are intact"), "{shown}");
    assert_eq!(app.world.workspaces, before);
    press(&mut app, "esc");
    let editor = screen(&mut app);
    assert!(
        editor.contains("Name *") && editor.contains(" x "),
        "{editor}"
    );
}

#[test]
fn new_workspace_id_comes_from_the_backend() {
    let (mut app, backend) = backed(Scenario::Returning, store_workspace);
    press(&mut app, "n space enter enter - t w o enter");
    press(&mut app, "ctrl+s");
    press(&mut app, "right enter");
    settle(&mut app);

    let seen = backend.seen();
    let [
        Effect::SaveWorkspace {
            id,
            base,
            workspace,
        },
    ] = seen.as_slice()
    else {
        panic!("one SaveWorkspace expected, got {seen:?}");
    };
    assert_eq!(*id, None, "a new workspace has no id yet");
    assert!(base.is_none(), "a new workspace has no base");
    let created: Vec<_> = app
        .world
        .workspaces
        .iter()
        .filter(|w| w.name == workspace.name)
        .collect();
    assert_eq!(created.len(), 1, "{:?}", app.world.workspaces);
    assert_eq!(created[0].id, 42, "the id is the backend's");
    assert!(
        app.world.workspaces.iter().all(|w| w.id != 100),
        "no screen may allocate an id: {:?}",
        app.world
            .workspaces
            .iter()
            .map(|w| w.id)
            .collect::<Vec<_>>()
    );
}

/// Stores the payload and normalises one flag the editor never changed, so a
/// screen that writes its pending copy is told from one that applies the
/// snapshot.
fn store_global(effect: &Effect, world: &World) -> Option<Msg> {
    let Effect::SaveGlobal { global, .. } = effect else {
        return None;
    };
    let mut snapshot = sim::snapshot(world);
    snapshot.global = (**global).clone();
    snapshot.global.dco_signoff = false;
    Some(Msg::GlobalSaved {
        result: Ok(Box::new(snapshot)),
    })
}

fn fail_global(effect: &Effect, _: &World) -> Option<Msg> {
    matches!(effect, Effect::SaveGlobal { .. }).then(|| Msg::GlobalSaved {
        result: Err(SaveFailure {
            error: "disk full".into(),
            next: None,
        }),
    })
}

fn only_global(seen: &[Effect]) -> (&GlobalConfig, &GlobalConfig) {
    let [Effect::SaveGlobal { base, global }] = seen else {
        panic!("one SaveGlobal expected, got {seen:?}");
    };
    (base, global)
}

#[test]
fn save_global_stores_the_payload() {
    let (mut app, backend) = backed(Scenario::Returning, store_global);
    assert!(app.world.global.coauthor_trailer && app.world.global.dco_signoff);
    press(&mut app, "s enter space ctrl+s");
    press(&mut app, "right enter");
    settle(&mut app);

    let seen = backend.seen();
    let (base, global) = only_global(&seen);
    assert!(base.coauthor_trailer && !global.coauthor_trailer);
    assert!(global.dco_signoff, "the editor did not touch the DCO flag");

    assert_eq!(status(&app), "Settings saved");
    assert!(!app.world.global.coauthor_trailer);
    assert!(
        !app.world.global.dco_signoff,
        "the world must be the reply snapshot, not the pending copy"
    );
}

#[test]
fn trust_rows_reach_the_roles_only_through_the_snapshot() {
    let (mut app, backend) = backed(Scenario::Returning, store_global);
    // A source written with a scheme still names the trust row's repository.
    let role = app
        .world
        .roles
        .iter_mut()
        .find(|r| r.name == "data-eng")
        .expect("the untrusted role");
    role.source = RoleSource::Git {
        url: "https://github.com/acme-labs/roles-experimental".into(),
        branch: Some("next".into()),
    };
    let untrusted = |w: &World| {
        w.roles
            .iter()
            .filter(|r| !r.trusted)
            .map(|r| r.name.clone())
            .collect::<Vec<_>>()
    };
    let before = untrusted(&app.world);
    assert_eq!(before, ["data-eng"]);
    press(&mut app, "s ] ] ] ] enter down space ctrl+s");
    press(&mut app, "right enter");
    settle(&mut app);

    let seen = backend.seen();
    let (base, global) = only_global(&seen);
    assert_eq!(base.change_count(global), 1);
    assert!(global.trust[1].trusted && !base.trust[1].trusted);
    assert_eq!(
        untrusted(&app.world),
        before,
        "a screen must not copy trust rows onto the roles"
    );
}

#[test]
fn failed_global_save_keeps_the_input() {
    let (mut app, _) = backed(Scenario::Returning, fail_global);
    let before = app.world.global.clone();
    press(&mut app, "s enter space ctrl+s");
    press(&mut app, "right enter");
    settle(&mut app);

    let shown = screen(&mut app);
    assert!(shown.contains("Settings error"), "{shown}");
    assert!(shown.contains("disk full"), "{shown}");
    assert!(
        !shown.contains("EACCES") && !shown.contains("not writable"),
        "the dialog must show the reply text, not the canned one:\n{shown}"
    );
    assert!(shown.contains("your edits are intact"), "{shown}");
    assert_eq!(app.world.global, before);
}

const NEW_ACCOUNT: &str = "c a enter S p a r e tab tab tab down down tab enter s k - a n t - v a l i d - a b c d e f 1 2 3 4 tab";

fn store_accounts(effect: &Effect, world: &World) -> Option<Msg> {
    let Effect::SaveAccounts { registry, .. } = effect else {
        return None;
    };
    let mut snapshot = sim::snapshot(world);
    snapshot.accounts = (**registry).clone();
    for account in &mut snapshot.accounts.accounts {
        if account.display_name == "Spare" {
            account.purpose = Some("kept by the backend".into());
        }
    }
    Some(Msg::AccountsSaved {
        added: None,
        result: Ok(Box::new(snapshot)),
    })
}

fn fail_accounts(effect: &Effect, _: &World) -> Option<Msg> {
    matches!(effect, Effect::SaveAccounts { .. }).then(|| Msg::AccountsSaved {
        added: None,
        result: Err("disk full".into()),
    })
}

#[test]
fn save_accounts_stores_the_payload() {
    let (mut app, backend) = backed(Scenario::AccountsMixed, store_accounts);
    press(&mut app, NEW_ACCOUNT);
    click_text(&mut app, "Save");
    settle(&mut app);

    let seen = backend.seen();
    let [
        Effect::CheckSource { .. },
        Effect::SaveAccounts { base, registry },
    ] = seen.as_slice()
    else {
        panic!("one CheckSource, then one SaveAccounts expected, got {seen:?}");
    };
    let named = |r: &AccountRegistry| r.accounts.iter().any(|a| a.display_name == "Spare");
    assert!(named(registry) && !named(base));

    assert_eq!(status(&app), "Saved Claude · Spare");
    let spare = app
        .world
        .accounts
        .accounts
        .iter()
        .find(|a| a.display_name == "Spare")
        .expect("the saved account");
    assert_eq!(
        spare.purpose.as_deref(),
        Some("kept by the backend"),
        "the registry must be the reply snapshot, not the pending copy"
    );
    assert!(!screen(&mut app).contains("New account"));
}

#[test]
fn failed_accounts_save_keeps_the_form() {
    let (mut app, _) = backed(Scenario::AccountsMixed, fail_accounts);
    let before = app.world.accounts.clone();
    press(&mut app, NEW_ACCOUNT);
    click_text(&mut app, "Save");
    settle(&mut app);

    let shown = screen(&mut app);
    assert!(shown.contains("New account"), "{shown}");
    assert!(shown.contains("disk full"), "{shown}");
    assert!(shown.contains("Spare"), "{shown}");
    assert_eq!(app.world.accounts, before);
}

/// Removes the workspace and, unlike the simulator, keeps the instances'
/// link to it, so the roster can only have come from this snapshot.
fn delete_workspace(effect: &Effect, world: &World) -> Option<Msg> {
    let Effect::DeleteWorkspace { id } = effect else {
        return None;
    };
    let mut snapshot = sim::snapshot(world);
    snapshot.workspaces.retain(|w| w.id != *id);
    Some(Msg::WorkspaceDeleted {
        id: *id,
        result: Ok(Box::new(snapshot)),
    })
}

fn refuse_delete(effect: &Effect, _: &World) -> Option<Msg> {
    let Effect::DeleteWorkspace { id } = effect else {
        return None;
    };
    Some(Msg::WorkspaceDeleted {
        id: *id,
        result: Err("the workspace is open in another session".into()),
    })
}

#[test]
fn delete_workspace_is_an_effect() {
    let (mut app, backend) = backed(Scenario::Returning, delete_workspace);
    let doomed = app.world.workspaces[0].clone();
    let linked: Vec<_> = app
        .world
        .instances
        .iter()
        .filter(|i| i.workspace == Some(doomed.id))
        .map(|i| i.id.clone())
        .collect();
    assert!(!linked.is_empty());
    press(&mut app, "down d");
    press(&mut app, "right enter");
    settle(&mut app);

    assert_eq!(backend.seen(), [Effect::DeleteWorkspace { id: doomed.id }]);
    assert!(app.world.workspaces.iter().all(|w| w.id != doomed.id));
    assert_eq!(
        status(&app),
        "Deleted workspace payments-platform · instances and files kept"
    );
    for id in linked {
        assert_eq!(
            app.world.instance(&id).map(|i| i.workspace),
            Some(Some(doomed.id)),
            "the instances are the reply snapshot's, not the screen's"
        );
    }
}

#[test]
fn refused_delete_leaves_the_row() {
    let (mut app, _) = backed(Scenario::Returning, refuse_delete);
    let ids = |w: &World| w.workspaces.iter().map(|w| w.id).collect::<Vec<_>>();
    let before = ids(&app.world);
    press(&mut app, "down d");
    press(&mut app, "right enter");
    settle(&mut app);

    assert_eq!(ids(&app.world), before);
    assert_eq!(status(&app), "the workspace is open in another session");
    assert!(screen(&mut app).contains("payments-platform"));
}

const RESTORED_AT: i64 = 12_345;

fn restore(effect: &Effect, world: &World) -> Option<Msg> {
    let Effect::Restore { instance } = effect else {
        return None;
    };
    let mut snapshot = sim::snapshot(world);
    for i in snapshot.instances.iter_mut().filter(|i| i.id == *instance) {
        i.status = InstanceStatus::Running;
        i.last_seen_secs = RESTORED_AT;
    }
    Some(Msg::Restored {
        instance: instance.clone(),
        result: Ok(Box::new(snapshot)),
    })
}

fn refuse_restore(effect: &Effect, _: &World) -> Option<Msg> {
    let Effect::Restore { instance } = effect else {
        return None;
    };
    Some(Msg::Restored {
        instance: instance.clone(),
        result: Err("the sandbox was removed".into()),
    })
}

#[test]
fn restore_is_an_effect() {
    let (mut app, backend) = backed(Scenario::Returning, restore);
    press(&mut app, "down right down down enter");

    assert_eq!(
        backend.seen(),
        [Effect::Restore {
            instance: "jk-c41e".into()
        }]
    );
    let restored = app.world.instance("jk-c41e").expect("the instance");
    assert_eq!(restored.status, InstanceStatus::Running);
    assert_eq!(
        restored.last_seen_secs, RESTORED_AT,
        "the instance is the reply snapshot's, not the screen's"
    );
    assert_eq!(app.route(), Route::Capsule);
    assert!(app.world.daemons.contains_key("jk-c41e"));
}

#[test]
fn refused_restore_stays_on_the_row() {
    let (mut app, _) = backed(Scenario::Returning, refuse_restore);
    press(&mut app, "down right down down enter");
    settle(&mut app);

    let kept = app.world.instance("jk-c41e").expect("the instance");
    assert_eq!(kept.status, InstanceStatus::PreservedDirty);
    assert_eq!(app.route(), Route::Manager);
    assert!(!app.world.daemons.contains_key("jk-c41e"));
    assert_eq!(status(&app), "the sandbox was removed");
}

const LOADED: &str = "loaded by the backend";

fn load_role(effect: &Effect, world: &World) -> Option<Msg> {
    let Effect::LoadRole { role } = effect else {
        return None;
    };
    let mut snapshot = sim::snapshot(world);
    for r in snapshot.roles.iter_mut().filter(|r| r.name == *role) {
        r.in_registry = true;
        r.trusted = true;
        r.load_error = None;
        r.description = LOADED.into();
    }
    Some(Msg::RoleLoaded {
        role: role.clone(),
        result: Ok(Box::new(snapshot)),
    })
}

fn fail_role_load(effect: &Effect, _: &World) -> Option<Msg> {
    let Effect::LoadRole { role } = effect else {
        return None;
    };
    Some(Msg::RoleLoaded {
        role: role.clone(),
        result: Err("manifest not found".into()),
    })
}

fn role<'a>(w: &'a World, name: &str) -> &'a jackin_console::domain::workspace::RoleEntry {
    w.roles.iter().find(|r| r.name == name).expect("the role")
}

#[test]
fn role_load_comes_from_the_snapshot() {
    let (mut app, backend) = backed(Scenario::Returning, load_role);
    assert!(!role(&app.world, "writer").in_registry);
    press(&mut app, "down e ] ] down a w r i t e r enter");
    settle(&mut app);

    assert_eq!(
        backend.seen(),
        [Effect::LoadRole {
            role: "writer".into()
        }]
    );
    assert_eq!(
        role(&app.world, "writer").description,
        LOADED,
        "the role is the reply snapshot's, not the screen's"
    );
    assert!(role(&app.world, "writer").in_registry);
    assert_eq!(status(&app), "Role writer loaded and allowed");
}

#[test]
fn failed_role_load_names_the_reply_error() {
    let (mut app, _) = backed(Scenario::Returning, fail_role_load);
    press(&mut app, "down e ] ] down a w r i t e r enter");
    settle(&mut app);

    let shown = screen(&mut app);
    assert!(shown.contains("Role writer failed to load"), "{shown}");
    assert!(shown.contains("manifest not found"), "{shown}");
    assert!(!role(&app.world, "writer").in_registry);
    assert_eq!(
        role(&app.world, "writer").load_error.as_deref(),
        Some("manifest not found"),
        "the world keeps the error of a failed load"
    );
}

/// Trusts the role in its own way: the role, and nothing in the global trust
/// rows. A screen that finds a trust row itself would flip that row too.
fn trust_role(effect: &Effect, world: &World) -> Option<Msg> {
    let Effect::TrustRole { role } = effect else {
        return None;
    };
    let mut snapshot = sim::snapshot(world);
    for r in snapshot.roles.iter_mut().filter(|r| r.name == *role) {
        r.trusted = true;
    }
    Some(Msg::RoleTrusted {
        role: role.clone(),
        result: Ok(Box::new(snapshot)),
    })
}

fn refuse_trust(effect: &Effect, _: &World) -> Option<Msg> {
    let Effect::TrustRole { role } = effect else {
        return None;
    };
    Some(Msg::RoleTrusted {
        role: role.clone(),
        result: Err("the trust store is read-only".into()),
    })
}

const TRUST_DIALOG: &str = "down e ] ] down down down down down enter";

#[test]
fn trust_dialog_sends_trust_role() {
    let (mut app, backend) = backed(Scenario::Returning, trust_role);
    let rows = app.world.global.trust.clone();
    press(&mut app, TRUST_DIALOG);
    press(&mut app, "right enter");
    settle(&mut app);

    assert_eq!(
        backend.seen(),
        [
            Effect::TrustRole {
                role: "data-eng".into()
            },
            Effect::LoadRole {
                role: "data-eng".into()
            },
        ]
    );
    assert!(role(&app.world, "data-eng").trusted);
    assert_eq!(
        app.world.global.trust, rows,
        "the screen must not find a trust row by the name of a source"
    );
}

#[test]
fn refused_trust_leaves_the_role_untrusted() {
    let (mut app, backend) = backed(Scenario::Returning, refuse_trust);
    press(&mut app, TRUST_DIALOG);
    press(&mut app, "right enter");
    settle(&mut app);

    assert_eq!(
        backend.seen(),
        [Effect::TrustRole {
            role: "data-eng".into()
        }],
        "an untrusted role is not loaded"
    );
    assert!(!role(&app.world, "data-eng").trusted);
    assert_eq!(status(&app), "the trust store is read-only");
}

const MOUNT_FORM: &str = "down e ] enter a";

#[test]
fn mount_form_keeps_its_error() {
    let mut app = app(Scenario::Returning);
    press(&mut app, MOUNT_FORM);
    press(&mut app, "tab tab r e l a t i v e enter");
    click_text(&mut app, "Save   │");

    let shown = screen(&mut app);
    assert!(shown.contains("New mount"), "the form closed:\n{shown}");
    assert!(
        shown.contains("Destination must be an absolute path"),
        "{shown}"
    );
    assert!(shown.contains("relative"), "the input was lost:\n{shown}");
    assert!(
        !shown.contains("/workspace/relative"),
        "nothing is added on an error:\n{shown}"
    );
}

#[test]
fn mount_form_saves_a_valid_mount_once() {
    let mut app = app(Scenario::Returning);
    press(&mut app, MOUNT_FORM);
    press(
        &mut app,
        "down tab g i t h u b . c o m / o / r tab / w o r k s p a c e / o k",
    );
    click_text(&mut app, "Save   │");

    let shown = screen(&mut app);
    assert!(!shown.contains("New mount"), "{shown}");
    let rows = shown
        .lines()
        .filter(|l| l.contains("/workspace/ok") && l.contains("github.com/o/r"))
        .count();
    assert_eq!(rows, 1, "{shown}");
    assert!(
        shown.contains("Mounts •") && shown.contains("1 change"),
        "{shown}"
    );
    assert!(
        shown.contains("Workspaces › payments-platform › edit"),
        "{shown}"
    );
}

const PERSONAL: &str = "acct-claude-personal";

fn first_window_pct(w: &World, id: &str) -> Option<u8> {
    w.accounts.get(id)?.usage.windows.first()?.used_pct
}

/// Refreshes the account the way a backend that read the provider would:
/// the first window reads 40% used, whatever it read before.
fn refresh_account(effect: &Effect, world: &World) -> Option<Msg> {
    let Effect::RefreshAccount {
        account, source, ..
    } = effect
    else {
        return None;
    };
    let now = world.now_secs();
    let mut usage = world.accounts.get(account)?.usage.clone();
    usage.freshness = FreshnessInfo::current(now);
    usage.windows[0].used_pct = Some(40);
    Some(Msg::AccountRefreshed {
        account: account.clone(),
        source: Box::new(source.clone()),
        result: Ok(Box::new(Refresh {
            usage,
            issue: None,
            last_refresh_secs: now,
            outcome: RefreshOutcome::NeedsAttention,
        })),
    })
}

fn refresh_personal(app: &mut App) {
    press(app, "c down down r");
    for _ in 0..40 {
        Application::handle(app, Input::Tick);
    }
}

#[test]
fn refresh_result_comes_from_the_backend() {
    let (mut app, backend) = backed(Scenario::AccountsMixed, refresh_account);
    refresh_personal(&mut app);

    assert!(matches!(
        backend.seen().as_slice(),
        [Effect::RefreshAccount { account, .. }] if account == PERSONAL
    ));
    assert_eq!(first_window_pct(&app.world, PERSONAL), Some(40));
    let status = status(&app);
    assert!(
        status.starts_with("Refreshed Claude · Personal · needs attention · "),
        "{status}"
    );
}

#[test]
fn the_sim_refreshes_as_it_did() {
    let mut app = timed(Scenario::AccountsMixed, world_for(Scenario::AccountsMixed));
    assert_eq!(first_window_pct(&app.world, PERSONAL), Some(38));
    refresh_personal(&mut app);

    assert_eq!(first_window_pct(&app.world, PERSONAL), Some(39));
    assert_eq!(
        status(&app),
        "Refreshed Claude · Personal · current · 4 windows"
    );
}

#[test]
fn a_validation_reply_for_a_replaced_source_does_not_touch_the_edited_account() {
    let mut app = timed(Scenario::AccountsMixed, world_for(Scenario::AccountsMixed));
    let old = app
        .world
        .accounts
        .get(PERSONAL)
        .expect("account")
        .source
        .clone();
    press(&mut app, "c down down v");
    assert!(matches!(
        app.world
            .accounts
            .get(PERSONAL)
            .expect("account")
            .validation,
        ValidationState::Validating { .. }
    ));

    let mut snapshot = sim::snapshot(&app.world);
    let edited = snapshot.accounts.get_mut(PERSONAL).expect("account");
    edited.source = CredentialSource::HostEnv {
        var: "REPLACEMENT_KEY".into(),
        detected: DetectedKind::ClaudeApiKeyEnv,
    };
    edited.validation = ValidationState::NeverValidated;
    let edited = edited.clone();
    Application::handle(
        &mut app,
        Input::External(External::new(Remote::Reloaded(Box::new(snapshot)))),
    );
    settle(&mut app);

    let kept = app.world.accounts.get(PERSONAL).expect("account");
    assert_ne!(kept.source, old);
    assert_eq!(kept.validation, ValidationState::NeverValidated);
    assert_eq!(*kept, edited);
    let shown = status(&app);
    assert!(!shown.contains(&kept.title()), "{shown}");
    assert_eq!(shown, "Validation discarded · the account changed");
}

#[test]
fn a_refresh_reply_for_a_replaced_source_does_not_touch_the_edited_account() {
    let mut app = timed(Scenario::AccountsMixed, world_for(Scenario::AccountsMixed));
    let old = app
        .world
        .accounts
        .get(PERSONAL)
        .expect("account")
        .source
        .clone();
    press(&mut app, "c down down r");
    assert_eq!(
        app.world
            .accounts
            .get(PERSONAL)
            .expect("account")
            .usage
            .freshness
            .phase,
        Freshness::Refreshing
    );

    let mut snapshot = sim::snapshot(&app.world);
    let edited = snapshot.accounts.get_mut(PERSONAL).expect("account");
    edited.source = CredentialSource::HostEnv {
        var: "REPLACEMENT_KEY".into(),
        detected: DetectedKind::ClaudeApiKeyEnv,
    };
    let edited = edited.clone();
    Application::handle(
        &mut app,
        Input::External(External::new(Remote::Reloaded(Box::new(snapshot)))),
    );
    settle(&mut app);

    let kept = app.world.accounts.get(PERSONAL).expect("account");
    assert_ne!(kept.source, old);
    assert_eq!(*kept, edited);
    let shown = status(&app);
    assert!(!shown.starts_with("Refreshed"), "{shown}");
}

fn failure_app() -> App {
    let mut app = timed(Scenario::Returning, world_for(Scenario::Returning));
    screen(&mut app);
    app
}

#[test]
fn a_failed_stop_shows_the_reply_error() {
    let mut app = failure_app();
    app.deliver(&Msg::Stopped {
        instance: "jk-7f3a".into(),
        result: Err("docker is not answering".into()),
    });
    assert_eq!(status(&app), "docker is not answering");
    let kept = app.world.instance("jk-7f3a").expect("the instance");
    assert_eq!(kept.status, InstanceStatus::Running);
    assert!(app.world.daemons.contains_key("jk-7f3a"));
}

#[test]
fn a_refused_purge_reports_its_reason() {
    let mut app = failure_app();
    let reason = "purge refused: 2 unpushed commits would be lost";
    app.deliver(&Msg::Purged {
        instance: "jk-c41e".into(),
        result: Err(reason.into()),
    });
    assert_eq!(status(&app), reason);
    let kept = app.world.instance("jk-c41e").expect("the instance");
    assert_ne!(kept.status, InstanceStatus::Purged);
}

#[test]
fn a_failed_prewarm_shows_the_reply_error() {
    let mut app = failure_app();
    app.deliver(&Msg::Prewarmed {
        workspace: 1,
        result: Err("the image build failed".into()),
    });
    assert_eq!(status(&app), "the image build failed");
}

#[test]
fn a_failed_validation_shows_the_reply_error() {
    let mut app = failure_app();
    let source = app
        .world
        .accounts
        .get(PERSONAL)
        .expect("account")
        .source
        .clone();
    app.deliver(&Msg::AccountValidated {
        account: PERSONAL.into(),
        source: Box::new(source),
        result: Err("the provider did not answer".into()),
    });
    assert!(
        status(&app).contains("the provider did not answer"),
        "{}",
        status(&app)
    );
}

#[test]
fn the_sim_keeps_the_last_good_values_when_the_broker_is_down() {
    let mut app = timed(Scenario::HardCases, world_for(Scenario::HardCases));
    refresh_personal(&mut app);

    assert_eq!(
        status(&app),
        "Refreshed Claude · Personal · broker unreachable · last good kept · 4 windows"
    );
    let a = app.world.accounts.get(PERSONAL).expect("the account");
    assert_eq!(a.usage.freshness.phase, Freshness::Stale);
    assert_eq!(
        a.issue.as_ref().map(|i| &i.reason),
        Some(&IssueReason::BrokerDown)
    );
}

/// Answers a refresh with the roster it already has: a backend that keeps
/// no heartbeat of its own and never fails the index.
fn unchanged_roster(effect: &Effect, world: &World) -> Option<Msg> {
    matches!(effect, Effect::Refresh).then(|| Msg::Refreshed {
        result: Ok(Box::new(sim::snapshot(world))),
    })
}

/// Ticks the app for `ms` of world time and returns the status line after
/// each tick.
fn run_for(app: &mut App, ms: i64) -> Vec<String> {
    let end = app.world.now_ms() + ms;
    let mut seen = vec![];
    while app.world.now_ms() < end {
        Application::handle(app, Input::Tick);
        seen.push(status(app));
    }
    seen
}

fn last_seen(w: &World) -> Vec<(String, i64)> {
    w.instances
        .iter()
        .map(|i| (i.id.clone(), i.last_seen_secs))
        .collect()
}

#[test]
fn a_backend_owns_last_seen() {
    let (mut app, backend) = backed(Scenario::HardCases, unchanged_roster);
    let before = last_seen(&app.world);
    run_for(&mut app, 60_000);

    assert!(
        backend.refreshes() >= 10,
        "the manager asks the backend for the roster"
    );
    assert_eq!(
        last_seen(&app.world),
        before,
        "the screen kept no heartbeat"
    );
}

#[test]
fn the_sim_fails_the_first_refresh_then_bumps_last_seen() {
    let mut app = timed(Scenario::HardCases, world_for(Scenario::HardCases));
    let before = last_seen(&app.world);
    let start = app.world.now_secs();
    let live = |w: &World| {
        w.instances
            .iter()
            .filter(|i| i.status.is_live())
            .map(|i| (i.id.clone(), i.last_seen_secs))
            .collect::<Vec<_>>()
    };

    run_for(&mut app, 4_000);
    assert_eq!(
        last_seen(&app.world),
        before,
        "the first refresh fails and bumps nothing"
    );

    run_for(&mut app, 8_000);
    let bumped = live(&app.world);
    assert!(!bumped.is_empty());
    for (id, seen) in bumped {
        assert!(seen >= start + 4, "{id} last seen {seen}, started {start}");
    }
}

const TAKEOVER: &str = "tty004 · MacBook attached to this instance";

fn attached_for(app: &mut App, ms: i64) -> Vec<String> {
    app.go(Go::Attach {
        instance: "jk-7f3a".into(),
        pane: None,
    });
    run_for(app, ms)
}

#[test]
fn a_backend_opens_no_takeover() {
    let (mut app, _) = backed(Scenario::HardCases, unchanged_roster);
    let seen = attached_for(&mut app, 60_000);
    assert!(
        seen.iter()
            .all(|s| !s.contains("attached to this instance")),
        "{seen:?}"
    );
}

#[test]
fn the_sim_opens_the_second_client_at_45_seconds() {
    let mut app = timed(Scenario::HardCases, world_for(Scenario::HardCases));
    let seen = attached_for(&mut app, 60_000);
    assert!(seen.iter().any(|s| s == TAKEOVER), "{seen:?}");
    let first = seen.iter().position(|s| s == TAKEOVER).expect("a takeover");
    assert!(first > 30, "not before its time: tick {first}");
}

struct ListedSource;

impl FileSource for ListedSource {
    fn list(&self, dir: &str) -> Result<Vec<FileEntry>, String> {
        match dir {
            "/Users/alexey/src/shared-libs" => Ok(vec![
                FileEntry::dir("/Users/alexey/src/shared-libs/zeta").meta("zeta meta"),
                FileEntry::dir("/Users/alexey/src/shared-libs/alpha").meta("alpha meta"),
                FileEntry::file("/Users/alexey/src/shared-libs/notes.md"),
            ]),
            _ => Ok(vec![]),
        }
    }

    fn is_dir(&self, _: &str) -> bool {
        true
    }
}

fn workdir_picker(world: World, files: Option<fn() -> Box<dyn FileSource>>) -> String {
    let backend = Arc::new(TestBackend {
        inner: SimBackend::shared(),
        seen: Mutex::new(vec![]),
        answer: |_, _| None,
        unanswered: |_| false,
        files,
    });
    let mut world = world;
    world.backend = backend;
    let mut app = timed(Scenario::Returning, world);
    press(&mut app, "down e");
    click_text(&mut app, "Choose…");
    screen(&mut app)
}

#[test]
fn the_working_directory_picker_lists_the_source() {
    let mut world = world_for(Scenario::Returning);
    world.fs.clear();
    let shown = workdir_picker(world, Some(|| Box::new(ListedSource)));
    let line = |dest: &str, meta: &str| shown.lines().any(|l| l.contains(dest) && l.contains(meta));
    assert!(line("/workspace/libs/zeta", "zeta meta"), "{shown}");
    assert!(line("/workspace/libs/alpha", "alpha meta"), "{shown}");
    assert!(
        !shown.contains("notes.md"),
        "a file is not a directory: {shown}"
    );
}

#[test]
fn the_working_directory_picker_lists_what_the_demo_listed() {
    let shown = workdir_picker(world_for(Scenario::Returning), None);
    let at = |meta: &str| {
        shown
            .find(meta)
            .unwrap_or_else(|| panic!("{meta}\n{shown}"))
    };
    assert!(
        at("6 items") < at("adr") && at("adr") < at("3 items"),
        "crates, docs and scripts in the order of the demo files: {shown}"
    );
    assert!(!shown.contains("rust"), "only direct children: {shown}");
}

#[test]
fn two_stops_due_in_one_tick_both_land() {
    let mut app = failure_app();
    let running: Vec<_> = ["jk-7f3a", "jk-9b02"]
        .iter()
        .map(|id| {
            let i = app.world.instance(id).expect("the instance");
            assert_eq!(i.status, InstanceStatus::Running, "{id}");
            i.id.clone()
        })
        .collect();
    for instance in &running {
        app.world.perform(Effect::Stop {
            instance: instance.clone(),
        });
    }
    settle(&mut app);

    for id in &running {
        let stopped = app.world.instance(id).expect("the instance");
        assert_ne!(
            stopped.status,
            InstanceStatus::Running,
            "the second reply must not put {id} back"
        );
        assert!(!app.world.daemons.contains_key(id.as_str()), "{id}");
    }
}

fn admin(w: &World) -> &RoleAdmin {
    w.surfaces.roles.as_ref().expect("the roles admin")
}

const MARKER: &str = "acct-claude-work";

/// Binds what was asked and, on its own, a second agent the screen never
/// touches, so a screen that writes its pending copy is told from one that
/// applies the snapshot.
fn bind_role(effect: &Effect, world: &World) -> Option<Msg> {
    let Effect::BindRole {
        role,
        agent,
        account,
        ..
    } = effect
    else {
        return None;
    };
    let mut snapshot = sim::snapshot(world);
    let rec = snapshot
        .surfaces
        .roles
        .as_mut()
        .and_then(|a| a.role_mut(role))?;
    rec.bindings.insert(*agent, account.clone()?);
    rec.bindings.insert(Agent::ClaudeCode, MARKER.into());
    Some(Msg::RoleBound {
        role: role.clone(),
        agent: *agent,
        result: Ok(Box::new(snapshot)),
    })
}

fn refuse_bind(effect: &Effect, _: &World) -> Option<Msg> {
    let Effect::BindRole { role, agent, .. } = effect else {
        return None;
    };
    Some(Msg::RoleBound {
        role: role.clone(),
        agent: *agent,
        result: Err("config.toml is locked by another jackin".into()),
    })
}

const PICK_EXPERIMENTS: &str = "tab enter down enter";

#[test]
fn role_bind_comes_from_the_snapshot() {
    let (mut app, backend) = backed(Scenario::RoleBinding, bind_role);
    press(&mut app, PICK_EXPERIMENTS);
    settle(&mut app);

    assert_eq!(
        backend.seen(),
        [Effect::BindRole {
            role: "sentinel".into(),
            agent: Agent::Codex,
            base: Some("acct-codex-primary".into()),
            account: Some("acct-codex-experiments".into()),
        }]
    );
    let bindings = &admin(&app.world)
        .role("sentinel")
        .expect("sentinel")
        .bindings;
    assert_eq!(
        bindings.get(&Agent::Codex).map(String::as_str),
        Some("acct-codex-experiments")
    );
    assert_eq!(
        bindings.get(&Agent::ClaudeCode).map(String::as_str),
        Some(MARKER),
        "the role is the reply snapshot's, not the screen's"
    );
}

#[test]
fn failed_role_bind_keeps_the_binding() {
    let (mut app, _) = backed(Scenario::RoleBinding, refuse_bind);
    let before = app.world.surfaces.roles.clone();
    press(&mut app, PICK_EXPERIMENTS);
    settle(&mut app);

    assert_eq!(app.world.surfaces.roles, before);
    let shown = status(&app);
    assert!(
        shown.contains("config.toml is locked by another jackin")
            && shown.contains("Nothing changed"),
        "{shown}"
    );
}

/// Names the role the way the backend does, not the way the location reads.
fn add_role(effect: &Effect, world: &World) -> Option<Msg> {
    let Effect::AddRole { location } = effect else {
        return None;
    };
    let record = sim::surfaces::add_role("~/roles/registered-by-backend", |_| false).ok()?;
    let name = record.name.clone();
    let mut snapshot = sim::snapshot(world);
    snapshot.surfaces.roles.as_mut()?.roles.push(record);
    Some(Msg::RoleAdded {
        location: location.clone(),
        result: Ok((name, Box::new(snapshot))),
    })
}

fn refuse_add(effect: &Effect, _: &World) -> Option<Msg> {
    let Effect::AddRole { location } = effect else {
        return None;
    };
    Some(Msg::RoleAdded {
        location: location.clone(),
        result: Err("the registry is read-only".into()),
    })
}

const ADD_NEW_ROLE: &str = "a ~ / r o l e s / n e w - r o l e enter tab tab enter";

#[test]
fn role_add_comes_from_the_snapshot() {
    let (mut app, backend) = backed(Scenario::RoleBinding, add_role);
    press(&mut app, ADD_NEW_ROLE);
    settle(&mut app);

    assert_eq!(
        backend.seen(),
        [Effect::AddRole {
            location: "~/roles/new-role".into()
        }]
    );
    let names: Vec<_> = admin(&app.world)
        .roles
        .iter()
        .map(|r| r.name.as_str())
        .collect();
    assert!(names.contains(&"registered-by-backend"), "{names:?}");
    assert!(!names.contains(&"new-role"), "{names:?}");
    assert_eq!(status(&app), "Role registered-by-backend added");
}

#[test]
fn failed_role_add_keeps_the_location() {
    let (mut app, _) = backed(Scenario::RoleBinding, refuse_add);
    let before = app.world.surfaces.roles.clone();
    press(&mut app, ADD_NEW_ROLE);
    settle(&mut app);

    assert_eq!(app.world.surfaces.roles, before);
    let shown = screen(&mut app);
    assert!(shown.contains("Could not add role"), "{shown}");
    assert!(shown.contains("the registry is read-only"), "{shown}");
    press(&mut app, "esc");
    let form = screen(&mut app);
    assert!(form.contains("register a role"), "{form}");
    assert!(form.contains("~/roles/new-role"), "{form}");
}

fn finding() -> RoleFinding {
    RoleFinding {
        area: FindingArea::Policy,
        key: Some("role.policy".into()),
        message: "found by the backend".into(),
        fix: Some("nothing to fix".into()),
    }
}

/// Reports a finding the simulator never raises for this manifest, and a
/// binding the screen never writes.
fn check_role(effect: &Effect, world: &World) -> Option<Msg> {
    let Effect::CheckRole { ticket, role, .. } = effect else {
        return None;
    };
    let mut snapshot = sim::snapshot(world);
    let rec = snapshot.surfaces.roles.as_mut()?.role_mut(role)?;
    rec.check = RoleCheckState::Done(vec![finding()]);
    rec.bindings.insert(Agent::ClaudeCode, MARKER.into());
    Some(Msg::RoleChecked {
        ticket: *ticket,
        role: role.clone(),
        result: Ok(Box::new(snapshot)),
    })
}

fn fail_check(effect: &Effect, _: &World) -> Option<Msg> {
    let Effect::CheckRole { ticket, role, .. } = effect else {
        return None;
    };
    Some(Msg::RoleChecked {
        ticket: *ticket,
        role: role.clone(),
        result: Err("the checker did not start".into()),
    })
}

#[test]
fn role_check_comes_from_the_snapshot() {
    let (mut app, backend) = backed(Scenario::RoleBinding, check_role);
    press(&mut app, "v");
    settle(&mut app);

    let seen = backend.seen();
    let [Effect::CheckRole { role, .. }] = seen.as_slice() else {
        panic!("one CheckRole expected, got {seen:?}");
    };
    assert_eq!(role, "sentinel");
    assert_eq!(
        admin(&app.world).role("sentinel").expect("sentinel").check,
        RoleCheckState::Done(vec![finding()]),
        "the check is the reply snapshot's"
    );
    assert_eq!(
        admin(&app.world).binding("sentinel", Agent::ClaudeCode),
        None,
        "a check reply changes the check only, never the bindings"
    );
    assert_eq!(status(&app), "Role sentinel: 1 error");
}

#[test]
fn failed_role_check_leaves_the_record() {
    let (mut app, _) = backed(Scenario::RoleBinding, fail_check);
    let before = app.world.surfaces.roles.clone();
    press(&mut app, "v");
    settle(&mut app);

    assert_eq!(app.world.surfaces.roles, before);
    assert_eq!(status(&app), "Role sentinel: the checker did not start");
    let shown = screen(&mut app);
    assert!(!shown.contains("checking…"), "{shown}");
}

/// Removes the role asked for and, on its own, a second one.
fn remove_role(effect: &Effect, world: &World) -> Option<Msg> {
    let Effect::RemoveRole { role } = effect else {
        return None;
    };
    let mut snapshot = sim::snapshot(world);
    snapshot
        .surfaces
        .roles
        .as_mut()?
        .roles
        .retain(|r| r.name != *role && r.name != "data-eng");
    Some(Msg::RoleRemoved {
        role: role.clone(),
        result: Ok(Box::new(snapshot)),
    })
}

fn refuse_remove(effect: &Effect, _: &World) -> Option<Msg> {
    let Effect::RemoveRole { role } = effect else {
        return None;
    };
    Some(Msg::RoleRemoved {
        role: role.clone(),
        result: Err("config.toml is read-only".into()),
    })
}

const REMOVE_WRITER: &str = "down down down down d right enter";

#[test]
fn role_remove_comes_from_the_snapshot() {
    let (mut app, backend) = backed(Scenario::RoleBinding, remove_role);
    press(&mut app, REMOVE_WRITER);
    settle(&mut app);

    assert_eq!(
        backend.seen(),
        [Effect::RemoveRole {
            role: "writer".into()
        }]
    );
    let names: Vec<_> = admin(&app.world)
        .roles
        .iter()
        .map(|r| r.name.as_str())
        .collect();
    assert_eq!(
        names,
        ["sentinel", "the-architect", "reviewer"],
        "the roster is the reply snapshot's, not the screen's"
    );
    assert_eq!(status(&app), "Removed role writer");
}

#[test]
fn failed_role_remove_keeps_the_role() {
    let (mut app, _) = backed(Scenario::RoleBinding, refuse_remove);
    let before = app.world.surfaces.roles.clone();
    press(&mut app, REMOVE_WRITER);
    settle(&mut app);

    assert_eq!(app.world.surfaces.roles, before);
    let shown = status(&app);
    assert!(
        shown.contains("config.toml is read-only") && shown.contains("Nothing changed"),
        "{shown}"
    );
}

const PINNED: &str = "backend-pinned";

/// Stores the manifest and pins the model, which the editor did not set.
fn save_role(effect: &Effect, world: &World) -> Option<Msg> {
    let Effect::SaveRole { role, doc, .. } = effect else {
        return None;
    };
    let mut stored = (**doc).clone();
    stored.codex_model = Some(PINNED.into());
    let mut snapshot = sim::snapshot(world);
    let rec = snapshot.surfaces.roles.as_mut()?.role_mut(role)?;
    rec.manifest = Ok(stored);
    Some(Msg::RoleSaved {
        role: role.clone(),
        result: Ok(Box::new(snapshot)),
    })
}

fn fail_role_save(effect: &Effect, _: &World) -> Option<Msg> {
    let Effect::SaveRole { role, .. } = effect else {
        return None;
    };
    Some(Msg::RoleSaved {
        role: role.clone(),
        result: Err("jackin.role.toml is on a read-only disk".into()),
    })
}

const EDIT_AND_SAVE: &str = "enter tab tab tab tab tab p i n enter ctrl+s";

fn save_in_the_editor(app: &mut App) {
    press(app, EDIT_AND_SAVE);
    settle(app);
    press(app, "right enter");
    settle(app);
}

#[test]
fn role_save_comes_from_the_snapshot() {
    let (mut app, backend) = backed(Scenario::RoleEditor, save_role);
    let original = admin(&app.world)
        .role("sentinel")
        .expect("sentinel")
        .manifest
        .clone();
    save_in_the_editor(&mut app);

    let seen = backend.seen();
    let [
        Effect::CheckRole { .. },
        Effect::SaveRole { role, base, doc },
    ] = seen.as_slice()
    else {
        panic!("a check and a SaveRole expected, got {seen:?}");
    };
    assert_eq!(role, "sentinel");
    assert_eq!(
        Ok(&**base),
        original.as_ref(),
        "the base is the opened manifest"
    );
    assert_eq!(doc.codex_model.as_deref(), Some("pin"));
    let saved = admin(&app.world).role("sentinel").expect("sentinel");
    assert_eq!(
        saved.manifest.as_ref().map(|m| m.codex_model.as_deref()),
        Ok(Some(PINNED)),
        "the manifest is the reply snapshot's, not the screen's"
    );
    assert_eq!(app.route(), Route::Roles);
    assert_eq!(status(&app), "Role sentinel saved");
}

#[test]
fn failed_role_save_keeps_the_edits() {
    let (mut app, _) = backed(Scenario::RoleEditor, fail_role_save);
    let before = app.world.surfaces.roles.clone();
    save_in_the_editor(&mut app);

    assert_eq!(app.world.surfaces.roles, before);
    assert_eq!(app.route(), Route::RoleEditor);
    let shown = screen(&mut app);
    assert!(shown.contains("Save failed"), "{shown}");
    assert!(
        shown.contains("jackin.role.toml is on a read-only disk"),
        "{shown}"
    );
    press(&mut app, "esc");
    let editor = screen(&mut app);
    assert!(
        editor.contains("pin") && editor.contains("1 change"),
        "{editor}"
    );
}

#[test]
fn failed_editor_check_unlocks_the_editor() {
    let (mut app, backend) = backed(Scenario::RoleEditor, fail_check);
    press(&mut app, EDIT_AND_SAVE);
    settle(&mut app);

    assert_eq!(status(&app), "the checker did not start");
    let shown = screen(&mut app);
    assert!(
        !shown.contains("checking") && !shown.contains("Save role"),
        "{shown}"
    );
    press(&mut app, "ctrl+s");
    settle(&mut app);
    let checks = backend
        .seen()
        .iter()
        .filter(|e| matches!(e, Effect::CheckRole { .. }))
        .count();
    assert_eq!(checks, 2, "the editor takes a second check");
}

#[test]
fn a_restored_instance_gets_the_host_pane_policy_when_it_is_created() {
    use jackin_console::pages::Pages;
    use jackin_console::panes::ClipboardWrite;

    let (mut app, _) = backed(Scenario::Returning, restore);
    let id = "jk-c41e";
    let msg = Msg::Restored {
        instance: id.into(),
        result: Ok(Box::new(sim::snapshot(&app.world))),
    };
    let route = app.route();
    let App {
        shell,
        router,
        world,
        ..
    } = &mut app;
    let (_, requests) = shell.deliver(true, |cx| match router.screen_mut(route) {
        Some(screen) => screen.on_msg(&msg, world, cx),
        None => junie_tui::core::event::Outcome::Ignored,
    });
    assert!(!requests.is_empty(), "the screen asks to attach");

    let daemon = app.world.daemons.get(id).expect("the restored daemon");
    assert_eq!(
        daemon.policy.clipboard_write,
        ClipboardWrite::Deny,
        "the host policy is on the daemon before the attach request runs"
    );
    assert!(!daemon.panes.is_empty());
}

fn held(unanswered: fn(&Effect) -> bool) -> (App, Arc<TestBackend>) {
    let backend = Arc::new(TestBackend {
        inner: SimBackend::shared(),
        seen: Mutex::new(vec![]),
        answer: restore,
        unanswered,
        files: None,
    });
    let mut world = world_for(Scenario::Returning);
    world.backend = backend.clone();
    (timed(Scenario::Returning, world), backend)
}

fn restores(backend: &TestBackend) -> usize {
    backend
        .seen()
        .iter()
        .filter(|e| matches!(e, Effect::Restore { .. }))
        .count()
}

#[test]
fn a_second_enter_on_a_restoring_instance_does_not_restore_it_again() {
    let (mut app, backend) = held(|e| matches!(e, Effect::Restore { .. }));
    press(&mut app, "down right down down enter");
    press(&mut app, "enter");

    assert_eq!(restores(&backend), 1, "one restore for two Enters");
    assert_eq!(status(&app), "Restore already running");

    let reply = restore(
        &Effect::Restore {
            instance: "jk-c41e".into(),
        },
        &app.world,
    )
    .expect("the reply");
    app.deliver(&reply);
    assert_eq!(app.route(), Route::Capsule);
    assert!(app.world.daemons.contains_key("jk-c41e"));
}

#[test]
fn a_refresh_waits_for_the_one_in_flight() {
    let (mut app, backend) = held(|e| matches!(e, Effect::Refresh));
    run_for(&mut app, 30_000);
    assert_eq!(backend.refreshes(), 1, "no second refresh while one is out");

    app.deliver(&Msg::Refreshed {
        result: Ok(Box::new(sim::snapshot(&app.world))),
    });
    run_for(&mut app, 6_000);
    assert_eq!(backend.refreshes(), 2, "the next one goes after the reply");
}

#[test]
fn a_refresh_that_lands_after_a_newer_snapshot_is_dropped() {
    let (mut app, backend) = held(|e| matches!(e, Effect::Refresh));
    run_for(&mut app, 6_000);
    assert_eq!(backend.refreshes(), 1);

    let old = sim::snapshot(&app.world);
    let mut newer = old.clone();
    newer.instances.retain(|i| i.id != "jk-c41e");
    app.deliver(&Msg::Purged {
        instance: "jk-c41e".into(),
        result: Ok(Box::new(newer)),
    });
    assert!(app.world.instance("jk-c41e").is_none());

    app.deliver(&Msg::Refreshed {
        result: Ok(Box::new(old)),
    });
    assert!(
        app.world.instance("jk-c41e").is_none(),
        "the older refresh did not bring the instance back"
    );
}

fn held_in(
    scenario: Scenario,
    unanswered: fn(&Effect) -> bool,
    answer: Answer,
) -> (App, Arc<TestBackend>) {
    let backend = Arc::new(TestBackend {
        inner: SimBackend::shared(),
        seen: Mutex::new(vec![]),
        answer,
        unanswered,
        files: None,
    });
    let mut world = world_for(scenario);
    world.backend = backend.clone();
    (timed(scenario, world), backend)
}

fn held_roles(unanswered: fn(&Effect) -> bool, answer: Answer) -> (App, Arc<TestBackend>) {
    held_in(Scenario::RoleBinding, unanswered, answer)
}

fn count(backend: &TestBackend, wanted: fn(&Effect) -> bool) -> usize {
    backend.seen().iter().filter(|e| wanted(e)).count()
}

static BINDS: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

fn bind_once_then_refuse(effect: &Effect, world: &World) -> Option<Msg> {
    if !matches!(effect, Effect::BindRole { .. }) {
        return None;
    }
    match BINDS.fetch_add(1, std::sync::atomic::Ordering::SeqCst) {
        0 => bind_role(effect, world),
        _ => refuse_bind(effect, world),
    }
}

#[test]
fn an_undo_that_failed_can_be_tried_again() {
    let (mut app, backend) = backed(Scenario::RoleBinding, bind_once_then_refuse);
    press(&mut app, PICK_EXPERIMENTS);
    settle(&mut app);
    press(&mut app, "u");
    settle(&mut app);
    assert!(
        status(&app).contains("Could not save the binding"),
        "{}",
        status(&app)
    );

    press(&mut app, "u");
    settle(&mut app);
    assert_ne!(status(&app), "Nothing to undo");
    assert_eq!(count(&backend, |e| matches!(e, Effect::BindRole { .. })), 3);
}

#[test]
fn a_second_add_waits_for_the_first_reply() {
    let (mut app, backend) = held_roles(|e| matches!(e, Effect::AddRole { .. }), add_role);
    press(&mut app, ADD_NEW_ROLE);
    press(&mut app, "a");

    assert_eq!(status(&app), "Adding role…");
    let shown = screen(&mut app);
    assert!(!shown.contains("register a role"), "{shown}");
    assert_eq!(count(&backend, |e| matches!(e, Effect::AddRole { .. })), 1);
}

#[test]
fn an_add_that_fails_after_the_operator_left_the_roles_says_so() {
    let (mut app, _) = held_roles(|e| matches!(e, Effect::AddRole { .. }), add_role);
    press(&mut app, ADD_NEW_ROLE);
    press(&mut app, "esc");
    assert_eq!(app.route(), Route::Manager);

    app.deliver(&Msg::RoleAdded {
        location: "~/roles/new-role".into(),
        result: Err("the registry is read-only".into()),
    });
    let shown = status(&app);
    assert!(
        shown.contains("Could not add role") && shown.contains("the registry is read-only"),
        "{shown}"
    );
}

#[test]
fn a_role_an_allowed_list_names_cannot_be_removed() {
    let mut world = world_for(Scenario::RoleBinding);
    for ws in &mut world.workspaces {
        ws.roles.default = None;
    }
    world.workspaces[0].roles.allowed =
        jackin_console::domain::workspace::AllowedRoles::Custom(vec!["sentinel".into()]);
    let name = world.workspaces[0].name.clone();
    let mut app = timed(Scenario::RoleBinding, world);
    press(&mut app, "d");

    let shown = screen(&mut app);
    assert!(shown.contains("Cannot remove role sentinel"), "{shown}");
    assert!(shown.contains(&name), "{shown}");
    assert!(!shown.contains("Remove role sentinel?"), "{shown}");
}

const WRITER: &str = "down down down down";

fn reread_manifest(effect: &Effect, world: &World) -> Option<Msg> {
    let Effect::LoadRole { role } = effect else {
        return None;
    };
    let mut snapshot = sim::snapshot(world);
    let admin = snapshot.surfaces.roles.as_mut()?;
    let doc = admin.role("sentinel")?.manifest.clone();
    admin.role_mut(role)?.manifest = doc;
    Some(Msg::RoleLoaded {
        role: role.clone(),
        result: Ok(Box::new(snapshot)),
    })
}

fn is_check(effect: &Effect) -> bool {
    matches!(effect, Effect::CheckRole { .. })
}

#[test]
fn a_role_whose_manifest_did_not_parse_is_read_again_before_the_check() {
    let (mut app, backend) = backed(Scenario::RoleBinding, reread_manifest);
    press(&mut app, WRITER);
    press(&mut app, "v");
    settle(&mut app);

    let seen = backend.seen();
    assert_eq!(
        seen.first(),
        Some(&Effect::LoadRole {
            role: "writer".into()
        }),
        "{seen:?}"
    );
    assert!(
        matches!(seen.get(1), Some(Effect::CheckRole { role, .. }) if role == "writer"),
        "{seen:?}"
    );
}

#[test]
fn a_manifest_that_still_does_not_parse_is_not_checked() {
    let (mut app, backend) = backed(Scenario::RoleBinding, |_, _| None);
    press(&mut app, WRITER);
    press(&mut app, "v");
    settle(&mut app);

    assert_eq!(count(&backend, |e| matches!(e, Effect::LoadRole { .. })), 1);
    assert_eq!(count(&backend, is_check), 0);
    assert_eq!(
        status(&app),
        "writer: jackin.role.toml still does not parse"
    );
}

#[test]
fn a_check_reply_of_another_ticket_does_not_finish_the_roles_check() {
    let (mut app, backend) = held_roles(is_check, check_role);
    press(&mut app, "v");
    let seen = backend.seen();
    let [Effect::CheckRole { ticket, role, .. }] = seen.as_slice() else {
        panic!("one CheckRole expected, got {seen:?}");
    };
    let mut snapshot = sim::snapshot(&app.world);
    snapshot
        .surfaces
        .roles
        .as_mut()
        .expect("roles")
        .role_mut(role)
        .expect("role")
        .check = RoleCheckState::Done(vec![finding()]);
    app.deliver(&Msg::RoleChecked {
        ticket: ticket + 1_000,
        role: role.clone(),
        result: Ok(Box::new(snapshot.clone())),
    });
    assert!(
        screen(&mut app).contains("checking…"),
        "the screen took the reply of another check"
    );

    app.deliver(&Msg::RoleChecked {
        ticket: *ticket,
        role: role.clone(),
        result: Ok(Box::new(snapshot)),
    });
    assert!(
        status(&app).contains("check") || status(&app).contains("error"),
        "{}",
        status(&app)
    );
    assert!(!screen(&mut app).contains("checking…"));
}

fn without_instances(app: &App) -> Box<jackin_console::world::Snapshot> {
    let mut snapshot = sim::snapshot(&app.world);
    snapshot.instances.clear();
    Box::new(snapshot)
}

#[test]
fn a_stop_confirmed_after_the_instance_left_is_refused() {
    let (mut app, backend) = backed(Scenario::Returning, |_, _| None);
    press(&mut app, "down right down t");
    let asked = screen(&mut app);
    assert!(asked.contains("Stop instance"), "{asked}");

    let gone = without_instances(&app);
    app.deliver(&Msg::Refreshed { result: Ok(gone) });
    press(&mut app, "right enter");

    assert_eq!(count(&backend, |e| matches!(e, Effect::Stop { .. })), 0);
    assert!(status(&app).contains("nothing to stop"), "{}", status(&app));
}

#[test]
fn a_purge_confirmed_after_the_instance_left_is_refused() {
    let (mut app, backend) = backed(Scenario::Returning, |_, _| None);
    press(&mut app, "down right down down p");
    let asked = screen(&mut app);
    assert!(asked.contains("Purge instance"), "{asked}");

    let gone = without_instances(&app);
    app.deliver(&Msg::Refreshed { result: Ok(gone) });
    press(&mut app, "c 4 1 e enter");

    assert_eq!(count(&backend, |e| matches!(e, Effect::Purge { .. })), 0);
    assert!(
        status(&app).contains("nothing to purge"),
        "{}",
        status(&app)
    );
}

#[test]
fn a_second_delete_waits_for_the_first_reply() {
    let (mut app, backend) = held_roles(
        |e| matches!(e, Effect::DeleteWorkspace { .. }),
        delete_workspace,
    );
    app.world.backend = backend.clone();
    let mut app = timed(
        Scenario::Returning,
        std::mem::replace(&mut app.world, world_for(Scenario::Returning)),
    );
    press(&mut app, "down d");
    press(&mut app, "right enter");
    press(&mut app, "d");
    press(&mut app, "right enter");

    assert_eq!(
        count(&backend, |e| matches!(e, Effect::DeleteWorkspace { .. })),
        1,
        "one delete for two confirmations"
    );
    assert_eq!(status(&app), "Workspace is deleting…");
}

#[test]
fn a_delete_error_for_a_workspace_that_is_gone_is_dropped() {
    let (mut app, _) = backed(Scenario::Returning, delete_workspace);
    let doomed = app.world.workspaces[0].id;
    press(&mut app, "down d");
    press(&mut app, "right enter");
    settle(&mut app);
    assert!(app.world.workspace(doomed).is_none());

    app.deliver(&Msg::WorkspaceDeleted {
        id: doomed,
        result: Err("no workspace with that id".into()),
    });
    assert!(
        !status(&app).contains("no workspace with that id"),
        "{}",
        status(&app)
    );
}

#[test]
fn inspect_names_the_account_the_launch_used() {
    let mut world = world_for(Scenario::Returning);
    let (provider, ws, role) = {
        let i = world.instance("jk-7f3a").expect("instance");
        (i.agent.provider(), i.workspace, i.role.clone())
    };
    let resolved = world
        .account_for(
            provider,
            ws.and_then(|id| world.workspace(id)),
            Some(&role),
            None,
        )
        .account
        .expect("an account");
    let other = world
        .accounts
        .by_provider(provider)
        .find(|a| a.id != resolved)
        .expect("a second account")
        .clone();
    world.instance_mut("jk-7f3a").expect("instance").account = Some(other.id.clone());
    let mut app = timed(Scenario::Returning, world);
    press(&mut app, "down right down i");

    let shown = screen(&mut app);
    let wanted = format!("account {}", other.title());
    assert!(
        shown
            .lines()
            .any(|l| l.contains("Agent") && l.contains(&wanted)),
        "{wanted}\n{shown}"
    );
}

#[test]
fn a_refresh_failure_that_a_newer_reply_overtook_is_not_reported() {
    let (mut app, _) = backed(Scenario::Returning, |_, _| None);
    app.world.refresh_from = Some(app.world.snapshots_applied);
    app.world.apply(sim::snapshot(&app.world));
    app.deliver(&Msg::Refreshed {
        result: Err("instance index unreadable".into()),
    });
    assert!(!status(&app).contains("Refresh failed"), "{}", status(&app));

    app.world.refresh_from = Some(app.world.snapshots_applied);
    app.deliver(&Msg::Refreshed {
        result: Err("instance index unreadable".into()),
    });
    assert!(status(&app).contains("Refresh failed"), "{}", status(&app));
}

#[test]
fn a_second_restore_reply_keeps_the_daemon_the_first_built() {
    let (mut app, _) = backed(Scenario::Returning, restore);
    let id = "jk-c41e";
    let reply = |app: &App| {
        let mut snapshot = sim::snapshot(&app.world);
        for i in snapshot.instances.iter_mut().filter(|i| i.id == id) {
            i.status = InstanceStatus::Running;
        }
        Msg::Restored {
            instance: id.into(),
            result: Ok(Box::new(snapshot)),
        }
    };
    let first = reply(&app);
    app.deliver(&first);
    let daemon = app.world.daemons.get_mut(id).expect("the restored daemon");
    daemon.workspace = "kept".into();

    let second = reply(&app);
    app.deliver(&second);
    assert_eq!(app.world.daemons[id].workspace, "kept");
}
