#![cfg(feature = "demo")]

use jackin_console::arbiter::{Arbiter, DiscoveryError};
use jackin_console::domain::surfaces::Surfaces;
use jackin_console::fixtures::{CLOCK, HOME, Scenario, world_for};
use jackin_console::sim::SimBackend;
use jackin_console::sim::onepassword::SimOnePassword;
use jackin_console::world::{DaemonHealth, World, WorldParts};

fn parts() -> WorldParts {
    WorldParts::new(
        CLOCK,
        HOME.into(),
        format!("{HOME}/src/demo"),
        Box::new(SimOnePassword::fixture(CLOCK.now_secs())),
        SimBackend::shared(),
        Surfaces::demo(Scenario::Returning),
    )
}

#[test]
fn every_scenario_builds_a_world_that_names_it() {
    for scenario in Scenario::ALL {
        let world = world_for(scenario);
        let named = world
            .surfaces
            .about
            .iter()
            .find(|(label, _)| label == "Scenario")
            .map(|(_, value)| value.as_str());
        assert_eq!(named, Some(scenario.name()), "{}", scenario.name());
    }
    assert!(world_for(Scenario::FirstUse).workspaces.is_empty());
    assert!(world_for(Scenario::FirstUse).discovery_pending);
    assert!(!world_for(Scenario::Returning).workspaces.is_empty());
}

#[test]
fn world_new_fills_what_the_parts_leave_out() {
    let world = World::new(parts());
    assert!(world.workspaces.is_empty() && world.roles.is_empty() && world.instances.is_empty());
    assert!(world.daemons.is_empty() && world.jobs.is_empty() && world.fs.is_empty());
    assert_eq!(world.clipboard, None);
    assert_eq!(world.daemon_health, DaemonHealth::Healthy);
    assert!(!world.refresh_fails && !world.discovery_pending);
    assert_eq!(world.takeover_at_ms, None);
    assert!(world.session_accounts.is_empty());
    assert_eq!(world.last_refresh_secs, CLOCK.now_secs());
    assert_eq!(world.cwd, format!("{HOME}/src/demo"));
}

#[test]
fn world_new_keeps_the_parts_it_is_given() {
    let source = world_for(Scenario::Returning);
    let world = World::new(WorldParts {
        workspaces: source.workspaces.clone(),
        last_refresh_secs: 5,
        ..parts()
    });
    assert_eq!(world.workspaces, source.workspaces);
    assert_eq!(world.last_refresh_secs, 5);
}

#[test]
fn world_new_counts_the_running_instances_in_the_arbiter() {
    let source = world_for(Scenario::Returning);
    let running = source.running_count();
    assert!(running > 0);
    let world = World::new(WorldParts {
        instances: source.instances.clone(),
        ..parts()
    });
    assert_eq!(world.arbiter.running(), Ok(running));
}

#[test]
fn world_new_keeps_an_unreadable_instance_index() {
    let source = world_for(Scenario::Returning);
    let mut arbiter = Arbiter::new(0);
    arbiter.discovery = Err(DiscoveryError::IndexUnreadable);
    let world = World::new(WorldParts {
        arbiter,
        instances: source.instances.clone(),
        ..parts()
    });
    assert_eq!(
        world.arbiter.running(),
        Err(DiscoveryError::IndexUnreadable)
    );
}
