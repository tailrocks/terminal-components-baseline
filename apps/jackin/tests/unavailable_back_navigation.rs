#![cfg(feature = "demo")]

mod common;

use common::{app_with, press};
use jackin_console::domain::instance::{
    InstanceEntry, InstanceId, SandboxPhase, UnavailableInstance,
};
use jackin_console::fixtures::{Scenario, world_for};
use jackin_console::pages::Pages;
use jackin_console::screens::manager::RowKey;

#[test]
fn back_from_an_unavailable_sandbox_row_returns_to_its_parent() {
    let mut world = world_for(Scenario::Returning);
    let ws = world
        .workspaces
        .first()
        .expect("returning scenario has a workspace")
        .id;
    let id = InstanceId::new("gw-back", "boxdead");
    world.admit_entry(InstanceEntry::Unavailable(UnavailableInstance::new(
        id.clone(),
        Some(ws),
        None,
        Some("api".into()),
        SandboxPhase::Ready,
        None,
        "metadata degraded",
    )));
    let mut app = app_with(Scenario::Returning, world);
    app.router.manager_mut().selected = RowKey::Instance(id);
    press(&mut app, "h");
    assert!(
        app.router.manager_mut().selected == RowKey::Workspace(ws),
        "Back from an unavailable sandbox row must route to its workspace parent, got {:?}",
        app.router.manager_mut().selected
    );

    let loose = InstanceId::new("gw-back", "boxloose");
    app.world
        .admit_entry(InstanceEntry::Unavailable(UnavailableInstance::new(
            loose.clone(),
            None,
            None,
            Some("api".into()),
            SandboxPhase::Ready,
            None,
            "metadata degraded",
        )));
    app.router.manager_mut().selected = RowKey::Instance(loose);
    press(&mut app, "h");
    assert!(
        app.router.manager_mut().selected == RowKey::UnattributedSandboxes,
        "Back from an unattributed unavailable sandbox row must route to the unattributed parent, got {:?}",
        app.router.manager_mut().selected
    );
}
