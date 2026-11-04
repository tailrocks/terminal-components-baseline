#![cfg(feature = "demo")]

mod common;

use std::sync::{Arc, Mutex};

use common::*;
use jackin_console::app::{App, Route};
use jackin_console::backend::{
    Backend, ChangeSet, Completion, Effect, FileSource, FolderProbe, OnePassword, ValidationOutcome,
};
use jackin_console::domain::account::{AccountRegistry, CredentialSource};
use jackin_console::domain::agent::Provider;
use jackin_console::domain::instance::InstanceStatus;
use jackin_console::domain::launch::LaunchContent;
use jackin_console::domain::workspace::RoleSource;
use jackin_console::fixtures::{Scenario, app_with_world, world_for};
use jackin_console::launch::{LaunchPlan, LaunchTarget};
use jackin_console::panes::SessionScript;
use jackin_console::sim::{self, SimBackend};
use jackin_console::world::{GlobalConfig, Msg, World};
use junie_tui::core::event::Input;
use junie_tui::motion::Motion;
use junie_tui::runtime::Application;
use junie_tui::theme::{ColorLevel, Theme};

type Answer = fn(&Effect, &World) -> Option<Msg>;

/// A backend that records every effect it receives and answers with its own
/// function, the way a real backend would. Effects it does not answer go to
/// the simulator.
#[derive(Debug)]
struct TestBackend {
    inner: Arc<dyn Backend>,
    seen: Mutex<Vec<Effect>>,
    answer: Answer,
}

impl TestBackend {
    fn new(answer: Answer) -> Arc<Self> {
        Arc::new(Self {
            inner: SimBackend::shared(),
            seen: Mutex::new(vec![]),
            answer,
        })
    }

    fn seen(&self) -> Vec<Effect> {
        self.seen.lock().expect("seen").clone()
    }
}

impl Backend for TestBackend {
    fn validate(
        &self,
        provider: Provider,
        source: &CredentialSource,
        plain_value: Option<&str>,
        op: &dyn OnePassword,
        now: i64,
    ) -> ValidationOutcome {
        self.inner.validate(provider, source, plain_value, op, now)
    }

    fn probe_folder(&self, path: &str) -> FolderProbe {
        self.inner.probe_folder(path)
    }

    fn perform(&self, effect: &Effect, world: &World) -> Option<Completion> {
        self.seen.lock().expect("seen").push(effect.clone());
        match (self.answer)(effect, world) {
            Some(msg) => Some(Completion::ready(0, msg)),
            None => self.inner.perform(effect, world),
        }
    }

    fn changes(
        &self,
        id: &str,
        touched: &[String],
        uncommitted: usize,
        unpushed: usize,
    ) -> ChangeSet {
        self.inner.changes(id, touched, uncommitted, unpushed)
    }

    fn launch_content(&self) -> Arc<LaunchContent> {
        self.inner.launch_content()
    }

    fn launch_plan(&self, world: &World, target: &LaunchTarget) -> LaunchPlan {
        self.inner.launch_plan(world, target)
    }

    fn session_script(&self) -> Arc<dyn SessionScript> {
        self.inner.session_script()
    }

    fn file_source(&self, world: &World) -> Box<dyn FileSource> {
        self.inner.file_source(world)
    }
}

fn backed(scenario: Scenario, answer: Answer) -> (App, Arc<TestBackend>) {
    let backend = TestBackend::new(answer);
    let mut world = world_for(scenario);
    world.backend = backend.clone();
    let app = app_with_world(
        scenario,
        world,
        Motion::Reduced,
        10_000,
        Theme::for_level(ColorLevel::TrueColor),
    );
    (app, backend)
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
        result: Err("disk full".into()),
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
        result: Err("disk full".into()),
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
        branch: "next".into(),
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
        result: Ok(Box::new(snapshot)),
    })
}

fn fail_accounts(effect: &Effect, _: &World) -> Option<Msg> {
    matches!(effect, Effect::SaveAccounts { .. }).then(|| Msg::AccountsSaved {
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
    let [Effect::SaveAccounts { base, registry }] = seen.as_slice() else {
        panic!("one SaveAccounts expected, got {seen:?}");
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
