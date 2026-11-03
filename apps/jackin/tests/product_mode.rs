#![cfg(not(feature = "demo"))]

use std::collections::BTreeMap;
use std::sync::Arc;

use jackin_console::app::{App, AppOptions, ClipboardMode, Entry, EntryPage, Remote, assemble};
use jackin_console::backend::{
    Backend, ChangeSet, Completion, Effect, FieldDescriptor, FolderProbe, OnePassword, OpAccount,
    OpError, OpField, OpItem, OpSession, OpVault, SecretClass, ValidationOutcome,
};
use jackin_console::domain::account::{AccountRegistry, CredentialSource};
use jackin_console::domain::agent::Provider;
use jackin_console::domain::launch::LaunchContent;
use jackin_console::domain::onepassword::OpReference;
use jackin_console::domain::surfaces::{LiveFacts, Surfaces};
use jackin_console::domain::workspace::{
    AccountPolicy, AllowedRoles, DirtyExitPolicy, RolePolicy, Workspace,
};
use jackin_console::panes::SessionScript;
use jackin_console::rain::INTRO_END;
use jackin_console::world::{Snapshot, World, WorldParts};
use junie_tui::clock::SimClock;
use junie_tui::core::event::{Input, Key};
use junie_tui::motion::Motion;
use junie_tui::runtime::{Application, ExternalSender, external_channel};
use junie_tui::theme::{ColorLevel, Theme};
use ratatui::Terminal;
use ratatui::backend::TestBackend;
use ratatui::crossterm::event::{KeyCode, KeyModifiers};

const NOT_USED: &str = "the product-shape test only renders the manager";

#[derive(Debug)]
struct TestBackendData;

impl Backend for TestBackendData {
    fn validate(
        &self,
        _: Provider,
        _: &CredentialSource,
        _: Option<&str>,
        _: &dyn OnePassword,
        _: i64,
    ) -> ValidationOutcome {
        unreachable!("{NOT_USED}")
    }

    fn probe_folder(&self, _: &str) -> FolderProbe {
        FolderProbe::Missing
    }

    fn perform(&self, _: &Effect, _: &World) -> Option<Completion> {
        None
    }

    fn changes(&self, _: &str, _: &[String], _: usize, _: usize) -> ChangeSet {
        unreachable!("{NOT_USED}")
    }

    fn launch_content(&self) -> Arc<LaunchContent> {
        unreachable!("{NOT_USED}")
    }

    fn session_script(&self) -> Arc<dyn SessionScript> {
        unreachable!("{NOT_USED}")
    }
}

#[derive(Debug, Clone)]
struct SignedInOnly;

impl OnePassword for SignedInOnly {
    fn session(&self) -> OpSession {
        OpSession::SignedIn
    }

    fn set_session(&mut self, _: OpSession) {}

    fn accounts(&self) -> &[OpAccount] {
        &[]
    }

    fn latency_ms(&self) -> i64 {
        0
    }

    fn list_accounts(&self) -> Result<Vec<&OpAccount>, OpError> {
        Ok(Vec::new())
    }

    fn list_vaults(&self, _: &str) -> Result<Vec<&OpVault>, OpError> {
        unreachable!("{NOT_USED}")
    }

    fn list_items(&self, _: &str, _: &str) -> Result<Vec<&OpItem>, OpError> {
        unreachable!("{NOT_USED}")
    }

    fn list_fields(&self, _: &str, _: &str, _: &str) -> Result<Vec<&OpField>, OpError> {
        unreachable!("{NOT_USED}")
    }

    fn reference(&self, _: &str, _: &str, _: &str, _: &str) -> Result<OpReference, OpError> {
        unreachable!("{NOT_USED}")
    }

    fn describe(&self, _: &OpReference) -> Result<FieldDescriptor, OpError> {
        unreachable!("{NOT_USED}")
    }

    fn endpoint_of(&self, _: &OpReference) -> Option<String> {
        None
    }

    fn classify(&self, _: &OpReference) -> Result<SecretClass, OpError> {
        unreachable!("{NOT_USED}")
    }

    fn box_clone(&self) -> Box<dyn OnePassword> {
        Box::new(self.clone())
    }
}

fn empty_world() -> World {
    World::new(WorldParts::new(
        SimClock::new(1_788_401_640, 0),
        "/home/dev".into(),
        "/home/dev".into(),
        Box::new(SignedInOnly),
        Arc::new(TestBackendData),
        Surfaces::openshell(LiveFacts {
            version: "1.4.0".into(),
        }),
    ))
}

fn workspace(id: u32, name: &str) -> Workspace {
    Workspace {
        id,
        name: name.into(),
        workdir: format!("/home/dev/{name}"),
        mounts: Vec::new(),
        roles: RolePolicy {
            allowed: AllowedRoles::default(),
            default: None,
            last: None,
        },
        env: Vec::new(),
        role_env: BTreeMap::new(),
        keep_awake: false,
        git_pull: false,
        docker: false,
        accounts: AccountPolicy::default(),
        dirty_policy: DirtyExitPolicy::Ask,
    }
}

fn app_over(world: World) -> App {
    assemble(
        world,
        Entry {
            page: EntryPage::Manager,
            outro_when_framed: false,
        },
        AppOptions {
            motion: Motion::Paused,
            frame: INTRO_END,
            theme: Theme::for_level(ColorLevel::TrueColor),
            clipboard: ClipboardMode::Preview,
        },
    )
}

fn screen(app: &mut App) -> String {
    let mut terminal = Terminal::new(TestBackend::new(140, 40)).expect("test terminal");
    terminal
        .draw(|f| Application::render(app, f))
        .expect("draw");
    let buffer = terminal.backend().buffer();
    buffer
        .content()
        .chunks(buffer.area.width as usize)
        .map(|row| row.iter().map(|c| c.symbol()).collect::<String>())
        .collect::<Vec<_>>()
        .join("\n")
}

fn press(app: &mut App, keys: &[KeyCode]) {
    for code in keys {
        Application::handle(
            app,
            Input::Key(Key {
                code: *code,
                mods: KeyModifiers::NONE,
            }),
        );
        screen(app);
    }
}

fn drain(app: &mut App) {
    let pending = Application::external(app)
        .map(|rx| rx.drain())
        .unwrap_or_default();
    for message in pending {
        Application::handle(app, Input::External(message));
    }
}

fn snapshot_of(world: &World, workspaces: Vec<Workspace>) -> Snapshot {
    Snapshot {
        workspaces,
        roles: Vec::new(),
        instances: Vec::new(),
        global: world.global.clone(),
        accounts: AccountRegistry::default(),
        surfaces: world.surfaces.clone(),
        github: Vec::new(),
        index_error: None,
    }
}

fn linked(world: World) -> (App, ExternalSender) {
    let (tx, rx) = external_channel();
    (app_over(world).with_external(rx), tx)
}

#[test]
fn product_shape_renders_and_receives_a_snapshot() {
    let world = empty_world();
    let pushed = snapshot_of(&world, vec![workspace(1, "live-project")]);
    let (mut app, tx) = linked(world);
    let first = screen(&mut app);
    assert!(
        first.contains("Workspaces"),
        "the first frame is the manager:\n{first}"
    );
    assert!(!first.contains("live-project"));
    assert!(tx.send(Remote::Reloaded(Box::new(pushed))));
    drain(&mut app);
    let after = screen(&mut app);
    assert!(after.contains("live-project"), "{after}");
}

#[test]
fn product_shape_about_rows_come_from_the_surfaces() {
    let mut world = empty_world();
    world.surfaces.about = vec![
        ("Product".into(), "jackin 1.4.0".into()),
        ("Mode".into(), "live".into()),
    ];
    let mut app = app_over(world);
    press(
        &mut app,
        &[
            KeyCode::F(10),
            KeyCode::Right,
            KeyCode::Right,
            KeyCode::Down,
            KeyCode::Enter,
        ],
    );
    let about = screen(&mut app)
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    assert!(about.contains("Product jackin 1.4.0"), "{about}");
    assert!(about.contains("Mode live"), "{about}");
    assert!(!about.contains("simulated"), "{about}");
}
