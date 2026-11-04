#![cfg(feature = "demo")]

use jackin_console::app::{App, Remote, Route};
use jackin_console::domain::instance::InstanceStatus;
use jackin_console::domain::workspace::Workspace;
use jackin_console::fixtures::{Scenario, app_for};
use jackin_console::screens::Go;
use jackin_console::sim;
use jackin_console::world::{Msg, Snapshot};
use junie_tui::core::event::{External, Input, Outcome};
use junie_tui::motion::Motion;
use junie_tui::runtime::{Application, ExternalSender, external_channel};
use junie_tui::theme::{ColorLevel, Theme};
use ratatui::Terminal;
use ratatui::backend::TestBackend;

const STOPPED: &str = "jk-7f3a";

fn app() -> App {
    app_for(
        Scenario::Returning,
        Motion::Paused,
        0,
        Theme::for_level(ColorLevel::TrueColor),
    )
}

fn linked() -> (App, ExternalSender) {
    let (tx, rx) = external_channel();
    (app().with_external(rx), tx)
}

fn pump(app: &mut App) -> Vec<Outcome> {
    let drained = Application::external(app)
        .map(|rx| rx.drain())
        .unwrap_or_default();
    drained
        .into_iter()
        .map(|m| Application::handle(app, Input::External(m)))
        .collect()
}

/// The reply to a stop: the roster with the instance no longer running.
fn stopped(app: &App, id: &str) -> Msg {
    let mut snapshot = sim::snapshot(&app.world);
    for i in snapshot.instances.iter_mut().filter(|i| i.id == id) {
        i.status = InstanceStatus::RestoreAvailable;
    }
    Msg::Stopped {
        instance: id.into(),
        result: Ok(Box::new(snapshot)),
    }
}

fn status_of(app: &App, id: &str) -> InstanceStatus {
    app.world.instance(id).expect("fixture instance").status
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

#[test]
fn the_application_exposes_the_receiver_it_was_given() {
    let (with, _tx) = linked();
    assert!(Application::external(&with).is_some());
    assert!(Application::external(&app()).is_none());
}

#[test]
fn a_remote_message_reaches_the_screens_like_a_scheduled_one() {
    let (mut app, tx) = linked();
    assert_eq!(status_of(&app, STOPPED), InstanceStatus::Running);
    let running = app.world.running_count();
    assert!(tx.send(Remote::Msg(stopped(&app, STOPPED))));
    assert_eq!(pump(&mut app), vec![Outcome::Changed]);
    assert_ne!(status_of(&app, STOPPED), InstanceStatus::Running);
    assert_eq!(app.world.running_count(), running - 1);
    assert_eq!(app.world.arbiter.running(), Ok(running - 1));
}

#[test]
fn deliver_runs_a_message_without_the_channel() {
    let mut app = app();
    let msg = stopped(&app, STOPPED);
    assert_eq!(app.deliver(&msg), Outcome::Changed);
    assert_ne!(status_of(&app, STOPPED), InstanceStatus::Running);
}

#[test]
fn a_payload_of_another_type_is_ignored() {
    let (mut app, tx) = linked();
    let before = app.world.running_count();
    assert!(tx.send(42_u32));
    assert_eq!(pump(&mut app), vec![Outcome::Ignored]);
    assert_eq!(
        Application::handle(&mut app, Input::External(External::new("text"))),
        Outcome::Ignored
    );
    assert_eq!(app.world.running_count(), before);
}

#[test]
fn a_reloaded_snapshot_shows_on_the_next_frame() {
    let (mut app, tx) = linked();
    let name = "reloaded-by-snapshot";
    assert!(!screen(&mut app).contains(name));
    let mut workspace: Workspace = app.world.workspaces[0].clone();
    workspace.id = 900;
    workspace.name = name.into();
    let w = &app.world;
    let mut workspaces = w.workspaces.clone();
    workspaces.push(workspace);
    let snapshot = Snapshot {
        workspaces,
        roles: w.roles.clone(),
        instances: w.instances.clone(),
        global: w.global.clone(),
        accounts: w.accounts.clone(),
        surfaces: w.surfaces.clone(),
        github: w.github.clone(),
        index_error: None,
        daemon_health: w.daemon_health,
    };
    assert!(tx.send(Remote::Reloaded(Box::new(snapshot))));
    assert_eq!(pump(&mut app), vec![Outcome::Changed]);
    assert!(app.world.workspaces.iter().any(|w| w.name == name));
    assert!(screen(&mut app).contains(name));
}

fn without(app: &App, id: &str) -> Snapshot {
    let mut snapshot = sim::snapshot(&app.world);
    snapshot.instances.retain(|i| i.id != id);
    snapshot
}

#[test]
fn a_reload_that_removes_the_open_instance_leaves_its_capsule() {
    let (mut app, tx) = linked();
    app.go(Go::Attach {
        instance: STOPPED.into(),
        pane: None,
    });
    assert_eq!(app.route(), Route::Capsule);
    assert!(app.world.daemons.contains_key(STOPPED));

    assert!(tx.send(Remote::Reloaded(Box::new(without(&app, STOPPED)))));
    assert_eq!(pump(&mut app), vec![Outcome::Changed]);

    assert_ne!(app.route(), Route::Capsule);
    assert!(app.world.instance(STOPPED).is_none());
    assert!(!app.world.daemons.contains_key(STOPPED));
}

#[test]
fn a_reload_that_removes_an_unopened_instance_drops_its_daemon() {
    let (mut app, tx) = linked();
    let other = "jk-9b02";
    assert!(app.world.daemons.contains_key(other));

    assert!(tx.send(Remote::Reloaded(Box::new(without(&app, other)))));
    pump(&mut app);

    assert!(!app.world.daemons.contains_key(other));
    assert!(app.world.daemons.contains_key(STOPPED));
    assert_eq!(app.world.arbiter.running(), Ok(app.world.running_count()));
}
