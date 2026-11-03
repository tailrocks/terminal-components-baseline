#![cfg(feature = "demo")]

use std::collections::BTreeMap;

use jackin_console::arbiter::DiscoveryError;
use jackin_console::domain::account::{Account, AccountForm, AccountOrigin, AccountRegistry};
use jackin_console::domain::agent::{Agent, AuthMode};
use jackin_console::domain::fixed::Fixed;
use jackin_console::domain::fixed_fields::*;
use jackin_console::domain::instance::Instance;
use jackin_console::domain::role::LaunchRoleRule;
use jackin_console::domain::surfaces::{LiveFacts, Surfaces};
use jackin_console::domain::vocabulary::Vocabulary;
use jackin_console::domain::workspace::{
    DirtyExitPolicy, EnvVar, Isolation, Mount, MountKind, MountScope, MountSource, RoleEntry,
    Workspace,
};
use jackin_console::fixtures::{Scenario, world_for};
use jackin_console::world::{GlobalConfig, Snapshot, TrustRow};

const PROJECT: &str = "/sandbox/project";

fn expected_mount() -> MountShape {
    MountShape {
        destination: PROJECT.into(),
        readonly: false,
        isolation: Isolation::Clone,
        kind: MountKind::Directory,
        scope: MountScope::Workspace,
        host_source: true,
        drift: None,
        running_isolated: false,
    }
}

fn expected_workspace() -> WorkspaceFixed {
    WorkspaceFixed {
        workdir: PROJECT.into(),
        mounts: vec![expected_mount()],
        keep_awake: false,
        git_pull: false,
        dirty_policy: DirtyExitPolicy::Ask,
        disabled_defaults_empty: true,
        role_preferred_empty: true,
        enabled_is_every_account: true,
    }
}

fn expected_global() -> GlobalFixed {
    GlobalFixed {
        coauthor_trailer: false,
        dco_signoff: false,
        mounts_empty: true,
        role_env_empty: true,
        non_launchable_modes_ignore: true,
        all_trusted: true,
    }
}

fn expected_account() -> AccountFixed {
    AccountFixed {
        enabled: true,
        origin: AccountOrigin::Registered,
    }
}

fn plain_ids(registry: &AccountRegistry) -> Vec<String> {
    registry
        .accounts
        .iter()
        .filter(|a| !a.default_for_provider)
        .map(|a| a.id.clone())
        .collect()
}

fn good_workspace(registry: &AccountRegistry) -> Workspace {
    let mut ws = Workspace::new(1, "good", PROJECT);
    ws.mounts = vec![Mount::host("~/src/good", PROJECT).isolation(Isolation::Clone)];
    ws.git_pull = false;
    ws.accounts.enabled = plain_ids(registry).into_iter().collect();
    ws
}

fn good_global() -> GlobalConfig {
    let launchable = Fixed::openshell().launchable;
    GlobalConfig {
        coauthor_trailer: false,
        dco_signoff: false,
        mounts: vec![],
        env: vec![],
        role_env: BTreeMap::new(),
        agent_modes: Agent::ALL
            .into_iter()
            .map(|a| {
                let mode = if a == launchable {
                    AuthMode::Sync
                } else {
                    AuthMode::Ignore
                };
                (a, mode)
            })
            .collect(),
        trust: vec![TrustRow {
            source: "github.com/acme-labs".into(),
            kind: "git",
            trusted: true,
            roles: 2,
        }],
    }
}

fn good_surfaces() -> Surfaces {
    Surfaces::openshell(LiveFacts {
        version: "1.4.0".into(),
        config_path: "/home/dev/.config/jackin/config.toml".into(),
    })
}

#[test]
fn fixed_world_workspaces() {
    let w = world_for(Scenario::OpenshellFixed);
    assert_eq!(w.workspaces.len(), 4);
    for ws in &w.workspaces {
        assert_eq!(
            workspace_fixed(ws, &w.accounts),
            expected_workspace(),
            "{}",
            ws.name
        );
    }
}

#[test]
fn fixed_world_global() {
    let w = world_for(Scenario::OpenshellFixed);
    assert_eq!(w.global.trust.len(), 4);
    assert_eq!(w.global.agent_modes.len(), Agent::ALL.len());
    assert_eq!(global_fixed(&w.global), expected_global());
}

#[test]
fn fixed_world_snapshot_and_world_views_agree() {
    let w = world_for(Scenario::OpenshellFixed);
    let snapshot = Snapshot {
        workspaces: w.workspaces.clone(),
        roles: w.roles.clone(),
        instances: w.instances.clone(),
        global: w.global.clone(),
        accounts: w.accounts.clone(),
        surfaces: w.surfaces.clone(),
        github: vec![],
        index_error: Some(DiscoveryError::IndexUnreadable),
    };
    let view = world_fixed(&w);
    assert_eq!(view.workspaces.len(), w.workspaces.len());
    assert_eq!(view.accounts.len(), w.accounts.accounts.len());
    assert_eq!(view.roles.len(), w.roles.len());
    assert_eq!(view.instances.len(), w.instances.len());
    assert_eq!(snapshot_fixed(&snapshot), view);
}

fn workspace_case(name: &str, mutate: impl Fn(&mut Workspace), want: WorkspaceFixed) {
    let registry = world_for(Scenario::OpenshellFixed).accounts;
    let mut ws = good_workspace(&registry);
    assert_eq!(
        workspace_fixed(&ws, &registry),
        expected_workspace(),
        "the starting record is not the Fixed value"
    );
    mutate(&mut ws);
    assert_eq!(workspace_fixed(&ws, &registry), want, "{name}");
}

#[test]
fn fixed_world_extractor_sees_every_workspace_field() {
    let registry = world_for(Scenario::OpenshellFixed).accounts;
    let default_id = registry
        .accounts
        .iter()
        .find(|a| a.default_for_provider)
        .expect("a default account")
        .id
        .clone();
    let plain_id = plain_ids(&registry).remove(0);
    let base = expected_workspace;
    let mount = |f: &dyn Fn(&mut MountShape)| {
        let mut m = expected_mount();
        f(&mut m);
        vec![m]
    };
    workspace_case(
        "workdir",
        |s| s.workdir = "/x".into(),
        WorkspaceFixed {
            workdir: "/x".into(),
            ..base()
        },
    );
    workspace_case(
        "mounts: none",
        |s| s.mounts.clear(),
        WorkspaceFixed {
            mounts: vec![],
            ..base()
        },
    );
    workspace_case(
        "mounts: two",
        |s| s.mounts.push(Mount::host("/a", "/b")),
        WorkspaceFixed {
            mounts: vec![
                expected_mount(),
                MountShape {
                    destination: "/b".into(),
                    isolation: Isolation::Shared,
                    ..expected_mount()
                },
            ],
            ..base()
        },
    );
    workspace_case(
        "mount destination",
        |s| s.mounts[0].destination = "/x".into(),
        WorkspaceFixed {
            mounts: mount(&|m| m.destination = "/x".into()),
            ..base()
        },
    );
    workspace_case(
        "mount readonly",
        |s| s.mounts[0].readonly = true,
        WorkspaceFixed {
            mounts: mount(&|m| m.readonly = true),
            ..base()
        },
    );
    workspace_case(
        "mount isolation",
        |s| s.mounts[0].isolation = Isolation::Worktree,
        WorkspaceFixed {
            mounts: mount(&|m| m.isolation = Isolation::Worktree),
            ..base()
        },
    );
    workspace_case(
        "mount kind",
        |s| s.mounts[0].kind = MountKind::Repository,
        WorkspaceFixed {
            mounts: mount(&|m| m.kind = MountKind::Repository),
            ..base()
        },
    );
    workspace_case(
        "mount scope",
        |s| s.mounts[0].scope = MountScope::Global,
        WorkspaceFixed {
            mounts: mount(&|m| m.scope = MountScope::Global),
            ..base()
        },
    );
    workspace_case(
        "mount source",
        |s| s.mounts[0].source = MountSource::Git("https://x/y".into()),
        WorkspaceFixed {
            mounts: mount(&|m| m.host_source = false),
            ..base()
        },
    );
    workspace_case(
        "mount drift",
        |s| s.mounts[0].drift = Some("moved".into()),
        WorkspaceFixed {
            mounts: mount(&|m| m.drift = Some("moved".into())),
            ..base()
        },
    );
    workspace_case(
        "mount running_isolated",
        |s| s.mounts[0].running_isolated = true,
        WorkspaceFixed {
            mounts: mount(&|m| m.running_isolated = true),
            ..base()
        },
    );
    workspace_case(
        "keep_awake",
        |s| s.keep_awake = true,
        WorkspaceFixed {
            keep_awake: true,
            ..base()
        },
    );
    workspace_case(
        "git_pull",
        |s| s.git_pull = true,
        WorkspaceFixed {
            git_pull: true,
            ..base()
        },
    );
    workspace_case(
        "dirty_policy",
        |s| s.dirty_policy = DirtyExitPolicy::Keep,
        WorkspaceFixed {
            dirty_policy: DirtyExitPolicy::Keep,
            ..base()
        },
    );
    workspace_case(
        "disabled_defaults (it also drops a default from the effective set)",
        |s| {
            s.accounts.disabled_defaults.insert(default_id.clone());
        },
        WorkspaceFixed {
            disabled_defaults_empty: false,
            enabled_is_every_account: false,
            ..base()
        },
    );
    workspace_case(
        "role_preferred",
        |s| {
            s.accounts.role_preferred.insert(
                ("r".into(), jackin_console::domain::agent::Provider::OpenAi),
                plain_id.clone(),
            );
        },
        WorkspaceFixed {
            role_preferred_empty: false,
            ..base()
        },
    );
    workspace_case(
        "enabled_is_every_account",
        |s| {
            s.accounts.enabled.remove(&plain_id);
        },
        WorkspaceFixed {
            enabled_is_every_account: false,
            ..base()
        },
    );
    workspace_case(
        "Data fields do not move the extractor",
        |s| {
            s.id = 99;
            s.name = "other".into();
            s.docker = true;
            s.env = vec![EnvVar::plain("K", "v")];
            s.accounts.preferred.insert(
                jackin_console::domain::agent::Provider::OpenAi,
                plain_id.clone(),
            );
        },
        base(),
    );
}

fn global_case(name: &str, mutate: impl Fn(&mut GlobalConfig), want: GlobalFixed) {
    let mut g = good_global();
    assert_eq!(
        global_fixed(&g),
        expected_global(),
        "the starting record is not the Fixed value"
    );
    mutate(&mut g);
    assert_eq!(global_fixed(&g), want, "{name}");
}

#[test]
fn fixed_world_extractor_sees_every_global_field() {
    let launchable = Fixed::openshell().launchable;
    let other = Agent::ALL
        .into_iter()
        .find(|a| *a != launchable)
        .expect("a second agent");
    let base = expected_global;
    global_case(
        "coauthor_trailer",
        |g| g.coauthor_trailer = true,
        GlobalFixed {
            coauthor_trailer: true,
            ..base()
        },
    );
    global_case(
        "dco_signoff",
        |g| g.dco_signoff = true,
        GlobalFixed {
            dco_signoff: true,
            ..base()
        },
    );
    global_case(
        "mounts",
        |g| g.mounts.push(Mount::host("/a", "/b")),
        GlobalFixed {
            mounts_empty: false,
            ..base()
        },
    );
    global_case(
        "role_env",
        |g| {
            g.role_env.insert("r".into(), vec![EnvVar::plain("A", "b")]);
        },
        GlobalFixed {
            role_env_empty: false,
            ..base()
        },
    );
    global_case(
        "an agent other than the launchable one is not Ignore",
        |g| {
            g.agent_modes.insert(other, AuthMode::Sync);
        },
        GlobalFixed {
            non_launchable_modes_ignore: false,
            ..base()
        },
    );
    global_case(
        "an agent other than the launchable one is missing",
        |g| {
            g.agent_modes.remove(&other);
        },
        GlobalFixed {
            non_launchable_modes_ignore: false,
            ..base()
        },
    );
    global_case(
        "no agent modes at all",
        |g| g.agent_modes.clear(),
        GlobalFixed {
            non_launchable_modes_ignore: false,
            ..base()
        },
    );
    global_case(
        "trust",
        |g| g.trust[0].trusted = false,
        GlobalFixed {
            all_trusted: false,
            ..base()
        },
    );
    global_case(
        "Data fields do not move the extractor",
        |g| {
            g.env = vec![EnvVar::plain("K", "v")];
            g.agent_modes.insert(launchable, AuthMode::Ignore);
            g.trust[0].source = "elsewhere".into();
            g.trust[0].kind = "local";
            g.trust[0].roles = 9;
        },
        base(),
    );
}

#[test]
fn fixed_world_extractor_sees_every_account_field() {
    let w = world_for(Scenario::OpenshellFixed);
    let mut good: Account = w.accounts.accounts[0].clone();
    good.enabled = true;
    good.origin = AccountOrigin::Registered;
    assert_eq!(account_fixed(&good), expected_account());
    let mut a = good.clone();
    a.enabled = false;
    assert_eq!(
        account_fixed(&a),
        AccountFixed {
            enabled: false,
            ..expected_account()
        },
        "enabled"
    );
    let mut a = good.clone();
    a.origin = AccountOrigin::Discovered;
    assert_eq!(
        account_fixed(&a),
        AccountFixed {
            origin: AccountOrigin::Discovered,
            ..expected_account()
        },
        "origin"
    );
    let mut a = good;
    a.display_name = "another".into();
    a.default_for_provider = !a.default_for_provider;
    assert_eq!(account_fixed(&a), expected_account(), "Data fields");
}

#[test]
fn fixed_world_extractor_sees_the_role_and_instance_fields() {
    let w = world_for(Scenario::OpenshellFixed);
    let mut role: RoleEntry = w.roles[0].clone();
    role.trusted = true;
    assert_eq!(role_fixed(&role), RoleFixed { trusted: true });
    role.trusted = false;
    assert_eq!(role_fixed(&role), RoleFixed { trusted: false }, "trusted");
    role.description = "another".into();
    assert_eq!(role_fixed(&role), RoleFixed { trusted: false }, "Data");
    let mut instance: Instance = w.instances[0].clone();
    instance.workdir = PROJECT.into();
    let want = InstanceFixed {
        workdir: PROJECT.into(),
    };
    assert_eq!(instance_fixed(&instance), want);
    instance.workdir = "/workspace/x".into();
    assert_eq!(
        instance_fixed(&instance),
        InstanceFixed {
            workdir: "/workspace/x".into()
        },
        "workdir"
    );
}

#[test]
fn fixed_world_extractor_sees_every_surface_field() {
    let good = good_surfaces();
    let want = SurfaceFixed::openshell();
    assert_eq!(surface_fixed(&good), want);
    let seen = |name: &str, mutate: &dyn Fn(&mut Surfaces), want: SurfaceFixed| {
        let mut s = good.clone();
        mutate(&mut s);
        assert_eq!(surface_fixed(&s), want, "{name}");
    };
    seen(
        "vocabulary",
        &|s| s.vocabulary = Vocabulary::demo(),
        SurfaceFixed {
            vocabulary: Vocabulary::demo(),
            ..want.clone()
        },
    );
    seen(
        "fixed",
        &|s| s.fixed = None,
        SurfaceFixed {
            fixed: None,
            ..want.clone()
        },
    );
    seen(
        "account_form",
        &|s| s.account_form = AccountForm::default(),
        SurfaceFixed {
            account_form: AccountForm::default(),
            ..want.clone()
        },
    );
    seen(
        "agent_blocked",
        &|s| s.agent_blocked = None,
        SurfaceFixed {
            agent_blocked: None,
            ..want.clone()
        },
    );
    seen(
        "wizard",
        &|s| s.wizard = None,
        SurfaceFixed {
            wizard: None,
            ..want.clone()
        },
    );
    seen(
        "launch_role",
        &|s| s.launch_role = LaunchRoleRule::Demo,
        SurfaceFixed {
            launch_role: LaunchRoleRule::Demo,
            ..want.clone()
        },
    );
    seen(
        "Data fields",
        &|s| {
            s.version = "9.9.9".into();
            s.cockpit_version = Some("x".into());
            s.config_path = None;
            s.about = vec![("a".into(), "b".into())];
        },
        want,
    );
}
