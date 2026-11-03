#![cfg(feature = "demo")]

use jackin_console::arbiter::DiscoveryError;
use jackin_console::domain::instance::DaemonSnapshot;
use jackin_console::fixtures::{Scenario, world_for};
use jackin_console::world::{Snapshot, World};

fn snapshot_of(w: &World) -> Snapshot {
    Snapshot {
        workspaces: w.workspaces.clone(),
        roles: w.roles.clone(),
        instances: w.instances.clone(),
        global: w.global.clone(),
        accounts: w.accounts.clone(),
        surfaces: w.surfaces.clone(),
        github: w.github.clone(),
        index_error: None,
    }
}

#[test]
fn apply_replaces_every_domain_collection() {
    let mut source = world_for(Scenario::RoleEditor);
    let mut target = world_for(Scenario::FirstUse);
    assert!(target.workspaces.is_empty() && target.surfaces.roles.is_none());
    assert!(target.roles.len() > 1 && target.github.len() > 1);
    source.roles.truncate(1);
    source.github.truncate(1);
    target.apply(snapshot_of(&source));
    assert_eq!(target.workspaces, source.workspaces);
    assert_eq!(target.roles, source.roles);
    assert_eq!(target.instances.len(), source.instances.len());
    assert_eq!(target.global, source.global);
    assert_eq!(target.accounts, source.accounts);
    assert_eq!(target.github, source.github);
    assert_eq!(
        format!("{:?}", target.surfaces),
        format!("{:?}", source.surfaces)
    );
}

#[test]
fn apply_leaves_the_console_state_alone() {
    let source = world_for(Scenario::Returning);
    let mut target = world_for(Scenario::CapsuleMulti);
    let daemons: Vec<_> = target.daemons.keys().cloned().collect();
    assert!(!daemons.is_empty());
    let (home, cwd, fs, now_ms) = (
        target.home.clone(),
        target.cwd.clone(),
        target.fs.clone(),
        target.clock.now_ms,
    );
    target.apply(snapshot_of(&source));
    assert_eq!(target.daemons.keys().cloned().collect::<Vec<_>>(), daemons);
    assert_eq!((target.home, target.cwd), (home, cwd));
    assert_eq!((target.fs, target.clock.now_ms), (fs, now_ms));
}

#[test]
fn apply_shows_the_console_tabs_on_the_new_instances() {
    let mut target = world_for(Scenario::CapsuleMulti);
    let mut snapshot = snapshot_of(&target);
    for instance in &mut snapshot.instances {
        instance.daemon = DaemonSnapshot::Unavailable;
    }
    let id = target.daemons.keys().next().cloned().expect("a daemon");
    target.apply(snapshot);
    let shown = &target.instance(&id).expect("instance").daemon;
    assert!(matches!(shown, DaemonSnapshot::Tabs(_)), "{shown:?}");
}

#[test]
fn apply_counts_the_running_instances_in_the_arbiter() {
    let source = world_for(Scenario::Returning);
    let mut target = world_for(Scenario::FirstUse);
    assert_eq!(target.arbiter.running(), Ok(0));
    target.apply(snapshot_of(&source));
    assert!(source.running_count() > 0);
    assert_eq!(target.arbiter.running(), Ok(source.running_count()));
}

#[test]
fn apply_reports_an_unreadable_index_and_recovers() {
    let source = world_for(Scenario::Returning);
    let mut target = world_for(Scenario::Returning);
    target.apply(Snapshot {
        index_error: Some(DiscoveryError::IndexUnreadable),
        ..snapshot_of(&source)
    });
    assert_eq!(
        target.arbiter.running(),
        Err(DiscoveryError::IndexUnreadable)
    );
    target.apply(snapshot_of(&source));
    assert_eq!(target.arbiter.running(), Ok(source.running_count()));
}

#[test]
fn apply_never_hands_out_a_workspace_id_in_use() {
    let source = world_for(Scenario::Returning);
    let highest = source.workspaces.iter().map(|w| w.id).max().expect("ids");
    let mut low = world_for(Scenario::FirstUse);
    low.next_workspace_id = 1;
    low.apply(snapshot_of(&source));
    assert!(low.next_workspace_id > highest);
    let mut high = world_for(Scenario::FirstUse);
    high.next_workspace_id = highest + 50;
    high.apply(snapshot_of(&source));
    assert_eq!(high.next_workspace_id, highest + 50);
}
