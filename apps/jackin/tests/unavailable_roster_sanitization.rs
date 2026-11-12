#![cfg(feature = "demo")]

mod common;

use common::{app_with, screen_at};
use jackin_console::domain::instance::{
    InstanceEntry, InstanceId, SandboxPhase, UnavailableInstance,
};
use jackin_console::fixtures::{Scenario, world_for};

#[test]
fn the_wide_running_roster_sanitizes_unavailable_metadata() {
    let mut world = world_for(Scenario::Returning);
    let id = InstanceId::new("gw-roster", "boxmeta");
    world.admit_entry(InstanceEntry::Unavailable(UnavailableInstance::new(
        id.clone(),
        None,
        Some("ws\u{202e}name".into()),
        Some("role\u{202e}x".into()),
        SandboxPhase::Ready,
        None,
        "\u{202e}sk-SECRETVALUE failed",
    )));
    let mut app = app_with(Scenario::Returning, world);
    let screen = screen_at(&mut app, 200, 40);
    assert!(
        !screen.contains("sk-SECRETVALUE"),
        "raw sensitive unavailable reason reached the wide roster:\n{screen}"
    );
    assert!(
        !screen.contains('\u{1b}') && !screen.contains('\u{202e}'),
        "raw terminal/bidirectional controls reached the wide roster:\n{screen}"
    );
    assert!(
        !screen.contains("role\u{202e}x")
            && !screen.contains("ws\u{202e}name")
            && !screen.contains("SECRETVALUE failed"),
        "raw unavailable reason, role or workspace metadata reached the wide roster:\n{screen}"
    );
    assert!(
        screen.contains("agent metadata"),
        "the sanitized reason fallback must render instead of the raw reason:\n{screen}"
    );
}
