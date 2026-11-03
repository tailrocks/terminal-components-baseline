#![cfg(feature = "demo")]

mod common;

use common::*;
use jackin_console::domain::role::{
    LaunchRole, LaunchRoleRule, NO_DEFAULT_ROLE, NO_SAVED_WORKSPACE,
};
use jackin_console::domain::workspace::{AllowedRoles, Workspace};
use jackin_console::fixtures::{Scenario, world_for};
use jackin_console::world::{LaunchSource, World};

fn world(rule: LaunchRoleRule) -> World {
    let mut world = world_for(Scenario::Returning);
    world.surfaces.launch_role = rule;
    world
}

fn workspace(world: &World) -> Workspace {
    let mut ws = world.workspaces[0].clone();
    ws.roles.default = None;
    ws.roles.last = None;
    ws.roles.allowed = AllowedRoles::All;
    ws
}

fn role(name: &str) -> LaunchRole {
    LaunchRole::Role(name.into())
}

fn names(world: &World, n: usize) -> Vec<String> {
    world.roles.iter().take(n).map(|r| r.name.clone()).collect()
}

#[test]
fn the_demo_rule_keeps_the_fixture_roles() {
    let world = world(LaunchRoleRule::Demo);
    let mut ws = workspace(&world);
    assert_eq!(
        world.launch_role(LaunchSource::Row(None)),
        role("the-architect")
    );
    assert_eq!(
        world.launch_role(LaunchSource::Row(Some(&ws))),
        role("the-architect")
    );
    ws.roles.last = Some("reviewer".into());
    assert_eq!(
        world.launch_role(LaunchSource::Row(Some(&ws))),
        role("reviewer")
    );
    ws.roles.default = Some("data-eng".into());
    assert_eq!(
        world.launch_role(LaunchSource::Row(Some(&ws))),
        role("data-eng")
    );
    assert_eq!(
        world.launch_role(LaunchSource::CurrentDir(Some(&ws))),
        role("the-architect")
    );
    assert_eq!(
        world.launch_role(LaunchSource::CurrentDir(None)),
        role("the-architect")
    );
}

#[test]
fn the_configured_rule_takes_the_default_role() {
    let world = world(LaunchRoleRule::Configured);
    let mut ws = workspace(&world);
    ws.roles.default = Some("data-eng".into());
    assert_eq!(
        world.launch_role(LaunchSource::Row(Some(&ws))),
        role("data-eng")
    );
    assert_eq!(
        world.launch_role(LaunchSource::CurrentDir(Some(&ws))),
        role("data-eng")
    );
}

#[test]
fn the_configured_rule_takes_the_only_allowed_role() {
    let world = world(LaunchRoleRule::Configured);
    let mut ws = workspace(&world);
    let only = names(&world, 3)[1].clone();
    ws.roles.allowed = AllowedRoles::Custom(vec![only.clone()]);
    assert_eq!(world.launch_role(LaunchSource::Row(Some(&ws))), role(&only));
}

#[test]
fn the_configured_rule_asks_for_a_default_when_several_roles_are_allowed() {
    let world = world(LaunchRoleRule::Configured);
    let mut ws = workspace(&world);
    let unavailable = LaunchRole::Unavailable(NO_DEFAULT_ROLE.into());
    assert_eq!(world.launch_role(LaunchSource::Row(Some(&ws))), unavailable);
    ws.roles.allowed = AllowedRoles::Custom(names(&world, 2));
    assert_eq!(world.launch_role(LaunchSource::Row(Some(&ws))), unavailable);
}

#[test]
fn the_configured_rule_asks_when_no_allowed_role_is_configured() {
    let world = world(LaunchRoleRule::Configured);
    let mut ws = workspace(&world);
    ws.roles.allowed = AllowedRoles::Custom(vec!["not-configured".into()]);
    assert_eq!(
        world.launch_role(LaunchSource::Row(Some(&ws))),
        LaunchRole::Unavailable(NO_DEFAULT_ROLE.into())
    );
}

#[test]
fn the_configured_rule_ignores_the_last_role() {
    let world = world(LaunchRoleRule::Configured);
    let mut ws = workspace(&world);
    ws.roles.last = Some("reviewer".into());
    assert_eq!(
        world.launch_role(LaunchSource::Row(Some(&ws))),
        LaunchRole::Unavailable(NO_DEFAULT_ROLE.into())
    );
}

#[test]
fn the_configured_rule_for_a_directory_without_a_workspace() {
    let mut world = world(LaunchRoleRule::Configured);
    assert_eq!(
        world.launch_role(LaunchSource::CurrentDir(None)),
        LaunchRole::Unavailable(NO_SAVED_WORKSPACE.into())
    );
    world.roles.truncate(1);
    let only = world.roles[0].name.clone();
    assert_eq!(
        world.launch_role(LaunchSource::CurrentDir(None)),
        role(&only)
    );
}

#[test]
fn the_picker_disables_every_row_while_no_role_resolves() {
    let mut world = world(LaunchRoleRule::Configured);
    world.workspaces[0].roles.default = None;
    world.workspaces[0].roles.last = None;
    let mut app = app_with(Scenario::Returning, world);
    press(&mut app, "down enter");
    let screen = screen(&mut app);
    assert!(
        screen.contains("payments-platform › no default role"),
        "{screen}"
    );
    let narrow = screen_at(&mut app, 80, 24);
    assert!(
        narrow.contains("no default role · set it in Roles"),
        "{narrow}"
    );
    let rows = screen.matches(NO_DEFAULT_ROLE).count();
    assert!(rows >= 4, "{rows} rows\n{screen}");
    assert_eq!(screen.matches("unavailable").count(), rows);
    press(&mut app, "enter");
    assert_eq!(app.route(), jackin_console::app::Route::Manager);
}

#[test]
fn the_picker_keeps_its_rows_while_a_role_resolves() {
    let mut world = world(LaunchRoleRule::Configured);
    world.workspaces[0].roles.default = Some("the-architect".into());
    let mut app = app_with(Scenario::Returning, world);
    press(&mut app, "down enter");
    let screen = screen(&mut app);
    assert!(
        screen.contains("payments-platform › the-architect"),
        "{screen}"
    );
    assert!(!screen.contains(NO_DEFAULT_ROLE) && !screen.contains("unavailable"));
}
