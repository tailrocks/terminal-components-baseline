#![cfg(feature = "demo")]

mod common;

use std::sync::{Arc, Mutex};

use common::*;
use jackin_console::app::App;
use jackin_console::backend::{
    Backend, ChangeSet, Completion, Effect, FileSource, FolderProbe, OnePassword, ValidationOutcome,
};
use jackin_console::domain::account::CredentialSource;
use jackin_console::domain::agent::Provider;
use jackin_console::domain::launch::LaunchContent;
use jackin_console::fixtures::{Scenario, app_with_world, world_for};
use jackin_console::launch::{LaunchPlan, LaunchTarget};
use jackin_console::panes::SessionScript;
use jackin_console::sim::{self, SimBackend};
use jackin_console::world::{Msg, World};
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
