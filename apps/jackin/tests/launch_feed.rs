#![cfg(feature = "demo")]

mod common;

use common::*;
use jackin_console::app::{App, Route};
use jackin_console::backend::OpSession;
use jackin_console::domain::agent::Agent;
use jackin_console::domain::instance::{DaemonSnapshot, Instance};
use jackin_console::fixtures::{Scenario, app_with_world, world_for};
use jackin_console::sim::SimBackend;
use jackin_console::world::World;
use junie_tui::core::event::Input;
use junie_tui::motion::Motion;
use junie_tui::runtime::Application;
use junie_tui::theme::{ColorLevel, Theme};

const WORK: &str = "acct-claude-work";

fn started(world: World) -> App {
    app_with_world(
        Scenario::LaunchRunning,
        world,
        Motion::Reduced,
        0,
        Theme::for_level(ColorLevel::TrueColor),
    )
}

fn tick(app: &mut App, times: usize) {
    for _ in 0..times {
        Application::handle(app, Input::Tick);
    }
}

fn tick_until(app: &mut App, what: &str, done: impl Fn(&mut App) -> bool) {
    for _ in 0..600 {
        if done(app) {
            return;
        }
        tick(app, 1);
    }
    let shown = screen(app);
    panic!("never reached: {what}\n{shown}");
}

fn shows(text: &'static str) -> impl Fn(&mut App) -> bool {
    move |app| screen(app).contains(text)
}

fn facts(i: &Instance) -> String {
    format!(
        "{} | {} | {} | {:?} | {} | {} | {} | {}",
        i.id,
        i.container,
        i.workdir,
        i.status,
        i.created_secs,
        i.run_id,
        i.github.as_ref().map_or("-", |r| r.full_name.as_str()),
        i.branch.as_deref().unwrap_or("-"),
    )
}

fn the_sim_launch(world: World) -> App {
    let mut app = started(world);
    tick_until(&mut app, "the capsule", |a| a.route() == Route::Capsule);
    app
}

#[test]
fn the_sim_launch_ends_as_it_did() {
    let app = the_sim_launch(world_for(Scenario::LaunchRunning));
    let record = app
        .world
        .instance("jk-b5df")
        .expect("the launched instance");
    assert_eq!(
        facts(record),
        "jk-b5df | jackin-payments-platform | /workspace/payments-platform | Running | 1788401643 \
         | run-202609030914-b5df | chainargos/payments-platform | feature/settlement-backoff"
    );
    assert_eq!(record.workspace, Some(1));
    assert_eq!(record.role, "the-architect");
    assert_eq!(record.agent, Agent::ClaudeCode);
    assert_eq!(record.default_branch, "main");
    assert_eq!(
        record.accounts,
        [
            "acct-claude-personal",
            "acct-claude-work",
            "acct-codex-primary",
            "acct-grok-team",
            "acct-opencode-go",
        ]
    );

    let daemon = app.world.daemons.get("jk-b5df").expect("its daemon");
    assert_eq!(daemon.workspace, "payments-platform");
    assert_eq!(daemon.tabs.len(), 1);
    let [pane] = daemon.panes.as_slice() else {
        panic!("one pane expected");
    };
    assert_eq!(pane.proc.agent, Some(Agent::ClaudeCode));
    assert_eq!(pane.proc.account.as_deref(), Some(WORK));
}

#[test]
fn the_sim_retry_unlocks_the_session_and_resumes() {
    let mut world = world_for(Scenario::LaunchRunning);
    world.backend = SimBackend::hard_cases();
    world.op.set_session(OpSession::Locked);
    let mut app = started(world);
    tick_until(
        &mut app,
        "the credential dialog",
        shows("Credential source unavailable"),
    );
    assert_eq!(app.world.op.session(), OpSession::Locked);
    assert!(screen(&mut app).contains("1Password locked: unlock the app and retry"));

    press(&mut app, "enter");
    assert_eq!(app.world.op.session(), OpSession::SignedIn);
    assert_eq!(
        status(&app),
        "1Password unlocked · resolving the reference again"
    );
    tick_until(&mut app, "the capsule", |a| a.route() == Route::Capsule);
    assert_eq!(
        app.world.instances.last().map(|i| i.run_id.as_str()),
        Some("run-202609030914-b5df")
    );
}

#[test]
fn the_sim_cancel_records_a_failed_setup() {
    let mut app = started(world_for(Scenario::LaunchRunning));
    tick(&mut app, 10);
    press(&mut app, "x");
    press(&mut app, "right enter");
    let record = app.world.instances.last().expect("a record");
    assert_eq!(
        facts(record),
        "jk-b5df | jackin-payments-platform | /workspace/payments-platform | FailedSetup | 1788401640 \
         | run-202609030914-b5df | chainargos/payments-platform | feature/settlement-backoff"
    );
    assert_eq!(record.daemon, DaemonSnapshot::Unavailable);
    assert_eq!(app.route(), Route::Manager);
    assert!(!app.world.daemons.contains_key("jk-b5df"));
}

#[test]
fn the_sim_abort_records_a_failed_setup() {
    let mut app = started(world_for(Scenario::LaunchRunning));
    tick(&mut app, 10);
    press(&mut app, "ctrl+c");
    let record = app.world.instances.last().expect("a record");
    assert_eq!(
        facts(record),
        "jk-b5df | jackin-payments-platform | /workspace/payments-platform | FailedSetup | 1788401640 \
         | run-202609030914-b5df | chainargos/payments-platform | feature/settlement-backoff"
    );
    assert_eq!(app.route(), Route::Manager);
}

#[test]
fn the_sim_failure_records_a_failed_setup() {
    let mut app = started(world_for(Scenario::LaunchFailure));
    tick_until(
        &mut app,
        "the failure",
        shows("The Construct network could not be attached"),
    );
    let record = app.world.instances.last().expect("a record");
    assert_eq!(
        facts(record),
        "jk-b5df | jackin-payments-platform | /workspace/payments-platform | FailedSetup | 1788401642 \
         | run-202609030914-b5df | chainargos/payments-platform | feature/settlement-backoff"
    );
    press(&mut app, "esc");
    assert_eq!(app.route(), Route::Manager);
}

fn status(app: &App) -> String {
    app.shell.status.text().unwrap_or_default().to_owned()
}
