#![cfg(feature = "demo")]

use std::collections::BTreeMap;

use jackin_console::arbiter::DiscoveryError;
use jackin_console::domain::account::{
    Account, AccountForm, AccountIdentity, AccountOrigin, AccountRegistry, Confidence,
    CredentialSource, DetectedKind, EditBlocked, Endpoint, IdentitySubject, IssueCode, LaunchBlock,
    Lifecycle, Provenance, Recoverability, RecoverableIssue, ValidationLevel, ValidationState,
};
use jackin_console::domain::agent::{Agent, AgentBlocked, AuthMode, Provider, UsageSurface};
use jackin_console::domain::docker::{DockerSetting, EngineState};
use jackin_console::domain::doctor::{DoctorState, DoctorSurface};
use jackin_console::domain::fixed::Fixed;
use jackin_console::domain::fixed_fields::*;
use jackin_console::domain::instance::{
    DaemonSnapshot, Instance, InstanceStatus, ManifestError, RepoRef,
};
use jackin_console::domain::openshell::{self, env_edit_block};
use jackin_console::domain::reason::{IssueReason, UsageReason};
use jackin_console::domain::role::{AccountOrder, LaunchRoleRule, RoleAdmin};
use jackin_console::domain::surfaces::{LiveFacts, Surfaces};
use jackin_console::domain::usage::{AccountUsage, FreshnessInfo, QuotaWindow, WindowCategory};
use jackin_console::domain::vocabulary::Vocabulary;
use jackin_console::domain::workspace::{
    AccountPolicy, AllowedRoles, DirtyExitPolicy, EnvVar, Isolation, Mount, MountKind, MountScope,
    MountSource, RoleEntry, RoleSource, WizardDefaults, Workspace,
};
use jackin_console::fixtures::{Scenario, world_for};
use jackin_console::world::{GlobalConfig, Snapshot, TrustKind, TrustRow};

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

fn expected_account(a: &Account) -> AccountFixed {
    AccountFixed {
        enabled: true,
        origin: AccountOrigin::Registered,
        edit_blocked: match &a.source {
            CredentialSource::HostEnv { var, .. } => Some(env_edit_block(var)),
            _ => None,
        },
    }
}

#[test]
fn an_env_account_blocks_its_edit_with_this_text() {
    assert_eq!(
        env_edit_block("ZAI_API_KEY"),
        EditBlocked::new(
            "reads ZAI_API_KEY · remove it, then add an account",
            "env account"
        )
    );
    assert_eq!(
        env_edit_block("A_VARIABLE_WITH_A_LONG_NAME"),
        EditBlocked::new(
            "reads A_VARIABLE_WITH_… · remove it, then add an account",
            "env account"
        )
    );
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
        trust: vec![trust_row(true)],
    }
}

fn trust_row(trusted: bool) -> TrustRow {
    TrustRow {
        source: "github.com/acme-labs".into(),
        kind: TrustKind::Git,
        trusted,
        roles: 2,
    }
}

fn good_surfaces() -> Surfaces {
    Surfaces::openshell(LiveFacts {
        version: "1.4.0".into(),
        config_path: "/home/dev/.config/jackin/config.toml".into(),
        gateway: None,
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
        daemon_health: w.daemon_health,
    };
    let view = world_fixed(&w);
    assert_eq!(view.workspaces.len(), w.workspaces.len());
    assert_eq!(view.accounts.len(), w.accounts.accounts.len());
    assert_eq!(view.roles.len(), w.roles.len());
    assert_eq!(view.instances.len(), w.instances.len());
    assert_eq!(snapshot_fixed(&snapshot), view);
}

#[test]
fn fixed_world_accounts() {
    let w = world_for(Scenario::OpenshellFixed);
    assert_eq!(w.accounts.accounts.len(), 12);
    for a in &w.accounts.accounts {
        assert_eq!(account_fixed(a), expected_account(a), "{}", a.id);
        assert_eq!(
            a.provenance,
            vec![Provenance::ConfiguredSource],
            "{}: a registered account does not come from the live host",
            a.id
        );
        assert_ne!(a.display_name, "discovered", "{}", a.id);
    }
}

#[test]
fn fixed_world_roles() {
    let w = world_for(Scenario::OpenshellFixed);
    assert!(!w.roles.is_empty());
    for r in &w.roles {
        assert_eq!(
            role_fixed(r),
            RoleFixed { trusted: true },
            "{}/{}",
            r.namespace,
            r.name
        );
    }
}

#[test]
fn fixed_world_instances() {
    let w = world_for(Scenario::OpenshellFixed);
    assert!(!w.instances.is_empty());
    for i in &w.instances {
        assert_eq!(
            instance_fixed(i),
            InstanceFixed {
                workdir: PROJECT.into()
            },
            "{}",
            i.id
        );
    }
}

#[test]
fn fixed_world_surfaces() {
    let w = world_for(Scenario::OpenshellFixed);
    let got = surface_fixed(&w.surfaces);
    assert_eq!(got.vocabulary, Vocabulary::openshell(), "vocabulary");
    assert_eq!(got.fixed, Some(Fixed::openshell()), "fixed");
    assert_eq!(got.account_form, AccountForm::openshell(), "account_form");
    assert_eq!(
        got.agent_blocked,
        Some(AgentBlocked::openshell()),
        "agent_blocked"
    );
    assert_eq!(got.wizard, Some(WizardDefaults::openshell()), "wizard");
    assert_eq!(got.launch_role, LaunchRoleRule::Configured, "launch_role");
}

#[test]
fn fixed_world_view_is_the_openshell_constants() {
    let w = world_for(Scenario::OpenshellFixed);
    let view = world_fixed(&w);
    assert_eq!(view.workspaces, vec![expected_workspace(); 4]);
    assert_eq!(w.accounts.accounts.len(), 12);
    assert_eq!(
        view.accounts,
        w.accounts
            .accounts
            .iter()
            .map(expected_account)
            .collect::<Vec<_>>()
    );
    assert_eq!(view.global, expected_global());
    assert_eq!(view.roles, vec![RoleFixed { trusted: true }; w.roles.len()]);
    assert_eq!(
        view.instances,
        vec![
            InstanceFixed {
                workdir: PROJECT.into()
            };
            w.instances.len()
        ]
    );
    assert_eq!(view.surfaces, SurfaceFixed::openshell());
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
        "trust: the first of two rows is trusted, the second is not",
        |g| g.trust.push(trust_row(false)),
        GlobalFixed {
            all_trusted: false,
            ..base()
        },
    );
    global_case(
        "trust: the first of two rows is not trusted, the second is",
        |g| {
            g.trust[0].trusted = false;
            g.trust.push(trust_row(true));
        },
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
            g.trust[0].kind = TrustKind::Path;
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
    assert_eq!(account_fixed(&good), expected_account(&good));
    let mut a = good.clone();
    a.enabled = false;
    assert_eq!(
        account_fixed(&a),
        AccountFixed {
            enabled: false,
            ..expected_account(&good)
        },
        "enabled"
    );
    let mut a = good.clone();
    a.origin = AccountOrigin::Discovered;
    assert_eq!(
        account_fixed(&a),
        AccountFixed {
            origin: AccountOrigin::Discovered,
            ..expected_account(&good)
        },
        "origin"
    );
    let mut a = good.clone();
    a.display_name = "another".into();
    a.default_for_provider = !a.default_for_provider;
    assert_eq!(account_fixed(&a), expected_account(&good), "Data fields");
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

fn wrong_workspace(registry: &AccountRegistry) -> Workspace {
    let defaults: Vec<String> = registry
        .accounts
        .iter()
        .filter(|a| a.default_for_provider)
        .map(|a| a.id.clone())
        .collect();
    let plain = plain_ids(registry);
    assert!(
        defaults.len() > 1 && plain.len() > 1,
        "the registry needs two defaults and two other accounts to start every element wrong"
    );
    let mut ws = Workspace::new(1, "x", "/elsewhere");
    ws.keep_awake = true;
    ws.dirty_policy = DirtyExitPolicy::Discard;
    ws.mounts = vec![Mount::host("/a", "/b"), Mount::host("/c", "/d")];
    ws.accounts.disabled_defaults = defaults.into_iter().collect();
    ws.accounts.role_preferred.insert(
        ("r".into(), jackin_console::domain::agent::Provider::OpenAi),
        plain[0].clone(),
    );
    ws.accounts.role_preferred.insert(
        ("s".into(), jackin_console::domain::agent::Provider::Zai),
        plain[1].clone(),
    );
    ws
}

#[test]
fn fixed_world_normalizer_sets_every_fixed_account_field() {
    let w = world_for(Scenario::OpenshellFixed);
    let mut a: Account = w.accounts.accounts[0].clone();
    a.enabled = false;
    a.origin = AccountOrigin::Discovered;
    assert_ne!(account_fixed(&a), expected_account(&a));
    openshell::account(&mut a);
    assert_eq!(account_fixed(&a), expected_account(&a));
}

#[test]
fn fixed_world_normalizer_sets_every_fixed_workspace_field() {
    let registry = world_for(Scenario::OpenshellFixed).accounts;
    let mut ws = wrong_workspace(&registry);
    assert!(ws.git_pull, "Workspace::new starts with git_pull on");
    assert_ne!(workspace_fixed(&ws, &registry), expected_workspace());
    openshell::workspace(&mut ws, "/host/x", &registry);
    assert_eq!(workspace_fixed(&ws, &registry), expected_workspace());
    assert_eq!(
        ws.mounts[0].source,
        MountSource::Host("/host/x".into()),
        "the mount source is the host path that went in"
    );
}

#[test]
fn fixed_world_normalizer_sets_every_fixed_global_field() {
    let launchable = Fixed::openshell().launchable;
    let mut g = good_global();
    g.coauthor_trailer = true;
    g.dco_signoff = true;
    g.mounts = vec![Mount::host("/a", "/b"), Mount::host("/c", "/d")];
    g.role_env = BTreeMap::from([
        ("r".into(), vec![EnvVar::plain("A", "b")]),
        ("s".into(), vec![EnvVar::plain("C", "d")]),
    ]);
    for a in Agent::ALL.into_iter().filter(|a| *a != launchable) {
        g.agent_modes.insert(a, AuthMode::Sync);
    }
    g.trust = vec![trust_row(false), trust_row(false), trust_row(false)];
    assert!(g.mounts.len() > 1 && g.role_env.len() > 1);
    assert!(
        g.agent_modes
            .iter()
            .all(|(a, mode)| *a == launchable || *mode != AuthMode::Ignore),
        "every agent but the launchable one starts wrong"
    );
    assert!(
        g.trust.iter().all(|row| !row.trusted),
        "every trust row starts wrong"
    );
    assert_ne!(global_fixed(&g), expected_global());
    openshell::global(&mut g);
    assert_eq!(global_fixed(&g), expected_global());
    assert_eq!(g.agent_modes[&launchable], AuthMode::Sync);
    assert_eq!(g.trust.len(), 3, "the rows stay, only `trusted` changes");

    let mut g = good_global();
    g.agent_modes.clear();
    openshell::global(&mut g);
    assert_eq!(global_fixed(&g), expected_global());
    assert_eq!(
        g.agent_modes[&launchable],
        AuthMode::Sync,
        "a missing launchable row gets Sync"
    );
    let mut g = good_global();
    g.agent_modes.insert(launchable, AuthMode::Ignore);
    openshell::global(&mut g);
    assert_eq!(
        g.agent_modes[&launchable],
        AuthMode::Ignore,
        "the launchable row keeps the mode the account decided"
    );
}

#[test]
fn fixed_world_normalizer_sets_the_role_and_instance_fields() {
    let w = world_for(Scenario::OpenshellFixed);
    let mut role: RoleEntry = w.roles[0].clone();
    role.trusted = false;
    openshell::role(&mut role);
    assert_eq!(role_fixed(&role), RoleFixed { trusted: true });
    let mut instance: Instance = w.instances[0].clone();
    instance.workdir = "/workspace/x".into();
    openshell::instance(&mut instance);
    assert_eq!(
        instance_fixed(&instance),
        InstanceFixed {
            workdir: PROJECT.into()
        }
    );
}

#[test]
fn fixed_world_normalizer_sets_every_fixed_surface_field() {
    let mut s = Surfaces::demo(Scenario::Returning);
    assert_ne!(surface_fixed(&s), SurfaceFixed::openshell());
    openshell::surfaces(&mut s);
    let got = surface_fixed(&s);
    assert_eq!(got.vocabulary, Vocabulary::openshell(), "vocabulary");
    assert_eq!(got.fixed, Some(Fixed::openshell()), "fixed");
    assert_eq!(got.account_form, AccountForm::openshell(), "account_form");
    assert_eq!(
        got.agent_blocked,
        Some(AgentBlocked::openshell()),
        "agent_blocked"
    );
    assert_eq!(got.wizard, Some(WizardDefaults::openshell()), "wizard");
    assert_eq!(got.launch_role, LaunchRoleRule::Configured, "launch_role");
}

fn probe_account() -> Account {
    Account {
        id: "probe-id".into(),
        origin: AccountOrigin::Discovered,
        display_name: "probe name".into(),
        agent: Some(Agent::Amp),
        provider: Provider::Zai,
        surface: UsageSurface::Grok,
        source: CredentialSource::HostEnv {
            var: "PROBE_KEY".into(),
            detected: DetectedKind::ZaiApiKeyEnv,
        },
        identity: AccountIdentity {
            subject: Some(IdentitySubject::Handle("probe".into())),
            plan: Some("probe plan".into()),
        },
        provenance: vec![Provenance::LiveHost],
        confidence: Confidence::Estimated,
        lifecycle: Lifecycle::NeedsLogin,
        purpose: Some("probe purpose".into()),
        enabled: false,
        default_for_provider: true,
        validation: ValidationState::Valid(ValidationLevel::QuotaReadable),
        last_refresh_secs: Some(1234),
        issue: Some(RecoverableIssue::new(
            IssueCode::RateLimited,
            IssueReason::Usage(UsageReason::RateLimited {
                retry_in_secs: None,
            }),
            Recoverability::Retryable,
        )),
        endpoint: Some(Endpoint {
            label: "probe endpoint".into(),
            host: "probe.example".into(),
        }),
        usage: AccountUsage {
            freshness: FreshnessInfo::current(4321),
            windows: vec![QuotaWindow::pct(
                "probe-window",
                "probe window",
                WindowCategory::Model,
                37,
            )],
        },
        launch_blocked: Some(LaunchBlock::UsageOnlySource),
        edit_blocked: Some(EditBlocked::new("probe text", "probe")),
    }
}

fn probe_instance() -> Instance {
    Instance {
        id: "jk-probe".into(),
        container: "probe-container".into(),
        workspace: Some(42),
        workdir: "/workspace/probe".into(),
        role: "probe-role".into(),
        agent: Agent::Amp,
        status: InstanceStatus::Crashed,
        created_secs: 11,
        last_seen_secs: 22,
        run_id: "run-probe".into(),
        sessions: Err(ManifestError::ReadError),
        daemon: DaemonSnapshot::NoTabs,
        branch: Some("probe-branch".into()),
        pr: Some((7, "probe pr".into())),
        default_branch: "probe-main".into(),
        github: Some(RepoRef {
            full_name: "probe/repo".into(),
            url: "https://probe.test/probe/repo".into(),
        }),
        uncommitted: 3,
        unpushed: 5,
        account: Some("probe-account".into()),
        accounts: vec!["probe-id".into()],
        agents: Vec::new(),
        task: None,
    }
}

fn probe_surfaces() -> Surfaces {
    Surfaces {
        tuples: Vec::new(),
        roles: Some(RoleAdmin {
            agents: vec![Agent::Amp],
            roles: vec![],
            order: AccountOrder::Jackin,
        }),
        docker: Some(DockerSetting {
            gateway: "probe-gateway".into(),
            engine: EngineState::Missing,
            risks: vec![],
            role_enabled: Default::default(),
        }),
        doctor: Some(DoctorSurface {
            noun: "probe".into(),
            state: DoctorState::NeverRun,
            rerun_secs: None,
        }),
        vocabulary: Vocabulary::demo(),
        account_form: AccountForm::default(),
        agent_blocked: None,
        launch_role: LaunchRoleRule::Demo,
        wizard: None,
        fixed: None,
        version: "7.7.7".into(),
        cockpit_version: Some("7.7.8".into()),
        config_path: Some("/probe/config.toml".into()),
        about: vec![("probe".into(), "about".into())],
    }
}

#[test]
fn fixed_world_normalizers_leave_data_fields_alone() {
    let registry = world_for(Scenario::OpenshellFixed).accounts;

    let before = probe_account();
    let mut after = before.clone();
    openshell::account(&mut after);
    assert_eq!(
        after,
        Account {
            enabled: true,
            origin: AccountOrigin::Registered,
            edit_blocked: Some(env_edit_block("PROBE_KEY")),
            ..before
        }
    );

    let mut role = RoleEntry {
        namespace: "probe-ns".into(),
        name: "probe-role".into(),
        source: RoleSource::Local {
            path: "/probe".into(),
        },
        trusted: false,
        in_registry: true,
        description: "probe description".into(),
        load_error: Some("probe load".into()),
        fetch_error: Some("probe fetch".into()),
    };
    let before = role.clone();
    openshell::role(&mut role);
    assert_eq!(
        role,
        RoleEntry {
            trusted: true,
            ..before
        }
    );

    let before = probe_instance();
    let mut after = before.clone();
    openshell::instance(&mut after);
    assert_eq!(
        after,
        Instance {
            workdir: PROJECT.into(),
            ..before
        }
    );

    let launchable = Fixed::openshell().launchable;
    let mut g = good_global();
    g.env = vec![EnvVar::plain("PROBE", "value")];
    g.agent_modes.insert(launchable, AuthMode::Ignore);
    g.trust[0].source = "probe-source".into();
    g.trust[0].kind = TrustKind::Path;
    g.trust[0].roles = 9;
    let before = g.clone();
    openshell::global(&mut g);
    assert_eq!(g, before, "the global normalizer only writes Fixed fields");
    let GlobalConfig {
        coauthor_trailer: _,
        dco_signoff: _,
        mounts: _,
        env,
        role_env: _,
        agent_modes,
        trust,
    } = &g;
    assert_eq!(env, &before.env);
    assert_eq!(agent_modes[&launchable], AuthMode::Ignore);
    let TrustRow {
        source,
        kind,
        trusted: _,
        roles,
    } = &trust[0];
    assert_eq!(
        (source.as_str(), *kind, *roles),
        ("probe-source", TrustKind::Path, 9)
    );

    let before = probe_surfaces();
    let mut after = before.clone();
    openshell::surfaces(&mut after);
    let Surfaces {
        roles,
        docker,
        doctor,
        vocabulary: _,
        account_form: _,
        agent_blocked: _,
        launch_role: _,
        wizard: _,
        fixed: _,
        tuples: _,
        version,
        cockpit_version,
        config_path,
        about,
    } = &after;
    assert_eq!(roles, &before.roles);
    assert_eq!(docker, &before.docker);
    assert_eq!(doctor, &before.doctor);
    assert_eq!(version, &before.version);
    assert_eq!(cockpit_version, &before.cockpit_version);
    assert_eq!(config_path, &before.config_path);
    assert_eq!(about, &before.about);

    let mut ws = wrong_workspace(&registry);
    ws.id = 77;
    ws.name = "probe".into();
    ws.roles.allowed = AllowedRoles::Custom(vec!["probe-role".into(), "probe-other".into()]);
    ws.roles.default = Some("probe-role".into());
    ws.roles.last = Some("probe-other".into());
    ws.env = vec![EnvVar::plain("PROBE", "value")];
    ws.role_env
        .insert("probe-role".into(), vec![EnvVar::plain("R", "v")]);
    ws.docker = true;
    ws.github = Some(RepoRef {
        full_name: "acme/probe".into(),
        url: "https://github.com/acme/probe".into(),
    });
    ws.accounts
        .preferred
        .insert(Provider::OpenAi, plain_ids(&registry).remove(0));
    let before = ws.clone();
    openshell::workspace(&mut ws, "/host/probe", &registry);
    let Workspace {
        id,
        name,
        workdir: _,
        mounts: _,
        roles,
        env,
        role_env,
        keep_awake: _,
        git_pull: _,
        docker,
        accounts,
        dirty_policy: _,
        github,
    } = &ws;
    assert_eq!(*id, before.id);
    assert_eq!(github, &before.github);
    assert_eq!(name, &before.name);
    assert_eq!(roles, &before.roles);
    assert_eq!(env, &before.env);
    assert_eq!(role_env, &before.role_env);
    assert_eq!(*docker, before.docker);
    let AccountPolicy {
        disabled_defaults: _,
        enabled,
        preferred,
        role_preferred: _,
    } = accounts;
    assert_eq!(preferred, &before.accounts.preferred);
    assert_eq!(
        enabled.iter().cloned().collect::<Vec<_>>(),
        {
            let mut ids = plain_ids(&registry);
            ids.sort();
            ids
        },
        "accounts.enabled is Fixed through the predicate; the normalizer writes the set"
    );
}

#[test]
fn fixed_world_normalizers_are_idempotent() {
    let registry = world_for(Scenario::OpenshellFixed).accounts;
    let mut ws = wrong_workspace(&registry);
    openshell::workspace(&mut ws, "/host/x", &registry);
    let once = ws.clone();
    openshell::workspace(&mut ws, "/host/x", &registry);
    assert_eq!(ws, once, "workspace");
    let mut g = good_global();
    g.coauthor_trailer = true;
    openshell::global(&mut g);
    let once = g.clone();
    openshell::global(&mut g);
    assert_eq!(g, once, "global");
    let mut s = Surfaces::demo(Scenario::Returning);
    openshell::surfaces(&mut s);
    let once = s.clone();
    openshell::surfaces(&mut s);
    assert_eq!(s, once, "surfaces");
    let mut a = probe_account();
    openshell::account(&mut a);
    let once = a.clone();
    openshell::account(&mut a);
    assert_eq!(a, once, "account");
}
