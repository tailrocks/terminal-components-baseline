#![cfg(feature = "demo")]

mod common;

use common::*;
use jackin_console::app::{App, Route};
use jackin_console::domain::agent::Agent;
use jackin_console::domain::launch::RefusalReason;
use jackin_console::fixtures::{Scenario, app_with_world, world_for};
use jackin_console::launch::{EnvAnswer, EnvQuestion, LaunchEvent, LaunchTarget};
use jackin_console::sim::SimBackend;
use jackin_console::world::World;
use junie_tui::core::event::Input;
use junie_tui::motion::Motion;
use junie_tui::runtime::Application;
use junie_tui::theme::{ColorLevel, Theme};

const INFRA: u32 = 2;
const PORTAL: u32 = 4;

fn started(world: World) -> App {
    app_with_world(
        Scenario::LaunchChecks,
        world,
        Motion::Reduced,
        0,
        Theme::for_level(ColorLevel::TrueColor),
    )
}

fn tick_until(app: &mut App, what: &str, done: impl Fn(&mut App) -> bool) {
    for _ in 0..600 {
        if done(app) {
            return;
        }
        Application::handle(app, Input::Tick);
    }
    panic!("never reached: {what}\n{}", screen(app));
}

fn shows(text: &'static str) -> impl Fn(&mut App) -> bool {
    move |app| screen(app).contains(text)
}

/// Opens the launch picker of the workspace `downs` rows below the current
/// directory and launches its first agent.
fn launch(app: &mut App, downs: usize, scope: &str) {
    for _ in 0..downs {
        press(app, "down");
    }
    press(app, "enter");
    let picker = screen(app);
    assert!(
        picker.contains(scope),
        "{scope:?} not in the picker\n{picker}"
    );
    press(app, "space enter");
    assert_eq!(app.route(), Route::Cockpit);
}

#[test]
fn the_sim_refuses_a_role_the_workspace_does_not_allow() {
    let mut app = started(world_for(Scenario::LaunchChecks));
    let others = app.world.instances.clone();
    let workspace = app.world.workspace(PORTAL).expect("customer-portal");
    assert!(!workspace.roles.allows("writer"), "the fixture's premise");

    launch(&mut app, 4, "customer-portal › writer");
    tick_until(&mut app, "the refusal", shows("Role not allowed"));
    let shown = screen(&mut app);
    for text in [
        "Workspace customer-portal does not allow role writer.",
        "nothing was created",
        "Allow it in the workspace's Roles tab, or load another role.",
    ] {
        assert!(shown.contains(text), "{text:?} missing\n{shown}");
    }
    assert_eq!(app.world.instances, others);

    press(&mut app, "esc");
    assert_eq!(app.route(), Route::Manager);
    assert_eq!(app.world.instances, others);
}

/// The refusal and the questions sit over the cockpit of the launch jackin
/// can make: Codex, the seven stages of the OpenShell plan.
#[test]
fn the_checks_sit_over_the_codex_cockpit_of_seven_stages() {
    for (downs, scope, shown_when) in [
        (
            PORTAL as usize,
            "customer-portal › writer",
            "Role not allowed",
        ),
        (
            INFRA as usize,
            "infra-control-plane › sentinel",
            "Fail the preflight hook on purpose",
        ),
    ] {
        let mut app = started(world_for(Scenario::LaunchChecks));
        launch(&mut app, downs, scope);
        tick_until(&mut app, "the check", shows(shown_when));
        let shown = screen(&mut app);
        assert!(shown.contains("Codex · OpenAI"), "{scope}:\n{shown}");
        assert!(shown.contains("0/7 stages"), "{scope}:\n{shown}");
        assert!(
            !shown.contains("Claude Code") && !shown.contains("/11 stages"),
            "{scope}:\n{shown}"
        );
    }
}

#[test]
fn the_sim_launches_a_role_the_workspace_allows_and_asks_nothing() {
    let mut app = started(world_for(Scenario::LaunchChecks));
    launch(&mut app, 1, "payments-platform › the-architect");
    tick_until(&mut app, "the capsule", |a| a.route() == Route::Capsule);
    assert!(!screen(&mut app).contains("Role not allowed"));
}

#[test]
fn the_sim_asks_what_the_role_manifest_asks() {
    let mut app = started(world_for(Scenario::LaunchChecks));
    launch(&mut app, INFRA as usize, "infra-control-plane › sentinel");
    tick_until(
        &mut app,
        "the questions",
        shows("Fail the preflight hook on purpose"),
    );
    let shown = screen(&mut app);
    for text in [
        "Note for the run",
        "SENTINEL_FAIL_PREFLIGHT",
        "SENTINEL_NOTE",
    ] {
        assert!(shown.contains(text), "{text:?} missing\n{shown}");
    }
    assert_eq!(app.route(), Route::Cockpit);

    press(&mut app, "tab enter h i enter");
    click_text(&mut app, "Start launch");
    tick_until(&mut app, "the capsule", |a| a.route() == Route::Capsule);
}

#[test]
fn escape_on_the_sim_questions_records_nothing() {
    let mut app = started(world_for(Scenario::LaunchChecks));
    let others = app.world.instances.clone();
    launch(&mut app, INFRA as usize, "infra-control-plane › sentinel");
    tick_until(
        &mut app,
        "the questions",
        shows("Fail the preflight hook on purpose"),
    );
    press(&mut app, "esc");
    assert_eq!(app.route(), Route::Manager);
    assert_eq!(
        app.world.instances, others,
        "nothing exists to record before the questions are answered"
    );
}

#[test]
fn a_backend_that_does_not_guard_neither_asks_nor_refuses() {
    for (downs, scope) in [
        (INFRA as usize, "infra-control-plane › sentinel"),
        (PORTAL as usize, "customer-portal › writer"),
    ] {
        let mut world = world_for(Scenario::LaunchChecks);
        world.backend = SimBackend::shared();
        let mut app = started(world);
        launch(&mut app, downs, scope);
        tick_until(&mut app, "the capsule", |a| a.route() == Route::Capsule);
    }
}

fn target(workspace: u32, role: &str) -> LaunchTarget {
    LaunchTarget {
        workspace: Some(workspace),
        role: role.into(),
        instances: vec![jackin_console::domain::tuple::InstanceChoice {
            agent: Agent::ClaudeCode,
            provider: jackin_console::domain::agent::Provider::Anthropic,
            account: None,
            model: String::new(),
        }],
        task: None,
    }
}

#[test]
fn the_guarded_sim_sends_the_refusal_before_any_stage() {
    let world = world_for(Scenario::LaunchChecks);
    let mut feed = SimBackend::guarded().launch(&world, &target(PORTAL, "writer"));
    let first = feed.poll(&world);
    assert_eq!(
        first,
        [LaunchEvent::Refused(RefusalReason::RoleNotAllowed {
            workspace: "customer-portal".into(),
            role: "writer".into(),
        })]
    );
    assert_eq!(feed.poll(&world), [], "a refused launch says nothing more");
}

#[test]
fn the_guarded_sim_holds_the_launch_until_the_questions_are_answered() {
    let world = world_for(Scenario::LaunchChecks);
    let mut feed = SimBackend::guarded().launch(&world, &target(INFRA, "sentinel"));
    let asked: Vec<_> = feed
        .poll(&world)
        .into_iter()
        .filter_map(|e| match e {
            LaunchEvent::NeedsInput(q) => Some(q),
            _ => None,
        })
        .collect();
    assert_eq!(
        asked,
        [vec![
            EnvQuestion {
                name: "SENTINEL_FAIL_PREFLIGHT".into(),
                prompt: "Fail the preflight hook on purpose".into(),
                default: Some("0".into()),
            },
            EnvQuestion {
                name: "SENTINEL_NOTE".into(),
                prompt: "Note for the run".into(),
                default: None,
            },
        ]]
    );
    for _ in 0..20 {
        assert_eq!(feed.poll(&world), [], "no stage starts before the answers");
    }

    feed.answer_env(vec![EnvAnswer {
        name: "SENTINEL_NOTE".into(),
        value: "hello".into(),
    }]);
    let resumed = feed.poll(&world);
    assert!(
        resumed
            .iter()
            .any(|e| matches!(e, LaunchEvent::StageChanged(0, _))),
        "{resumed:?}"
    );
}

#[test]
fn cancelling_the_guarded_sim_at_the_questions_keeps_no_record() {
    let world = world_for(Scenario::LaunchChecks);
    let mut feed = SimBackend::guarded().launch(&world, &target(INFRA, "sentinel"));
    feed.poll(&world);
    assert_eq!(feed.cancel(&world), None);
}

#[test]
fn a_role_without_questions_starts_at_once() {
    let world = world_for(Scenario::LaunchChecks);
    let mut feed = SimBackend::guarded().launch(&world, &target(1, "the-architect"));
    let first = feed.poll(&world);
    assert!(
        first
            .iter()
            .any(|e| matches!(e, LaunchEvent::StageChanged(0, _))),
        "{first:?}"
    );
    assert!(
        !first
            .iter()
            .any(|e| matches!(e, LaunchEvent::NeedsInput(_) | LaunchEvent::Refused(_)))
    );
}
