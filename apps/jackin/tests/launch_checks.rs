#![cfg(feature = "demo")]

mod common;

use common::*;
use jackin_console::app::{App, Route};
use jackin_console::domain::account::{
    Account, CredentialSource, Lifecycle, ValidationLevel, ValidationState,
};
use jackin_console::domain::agent::{Agent, AgentBlocked, AuthMode, Provider};
use jackin_console::domain::launch::RefusalReason;
use jackin_console::domain::tuple::{
    AccountRequirement, InstanceChoice, TupleIdentity, TupleOffer,
};
use jackin_console::fixtures::{Scenario, app_with_world, world_for};
use jackin_console::launch::{EnvAnswer, EnvQuestion, LaunchEvent, LaunchTarget};
use jackin_console::sim::launch::validate_tuples;
use jackin_console::sim::{self, SimBackend};
use jackin_console::world::{Msg, Snapshot, World};
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

fn supported_tuple(agent: Agent, provider: Provider, model: &str) -> TupleOffer {
    TupleOffer::supported(agent, provider, &[model])
}

fn ready_account(id: &str, name: &str, provider: Provider) -> Account {
    let mut account = Account::registered(
        id,
        name,
        provider,
        CredentialSource::PlainApiKey {
            fingerprint: format!("fixture-{id}"),
            tail: "test".into(),
        },
    );
    account.lifecycle = Lifecycle::Available;
    account.default_for_provider = true;
    account.validation = ValidationState::Valid(ValidationLevel::IdentityAuthenticated);
    account
}

fn tuple_world(tuples: Vec<TupleOffer>, accounts: Vec<Account>) -> World {
    let mut world = world_for(Scenario::Returning);
    world.surfaces.tuples = tuples;
    world.surfaces.agent_blocked = None;
    world.global.agent_modes.insert(Agent::Omp, AuthMode::Sync);
    let providers: Vec<Provider> = accounts.iter().map(|account| account.provider).collect();
    world
        .accounts
        .accounts
        .retain(|existing| !providers.contains(&existing.provider));
    for account in accounts {
        world.accounts.insert(account);
    }
    world.backend = SimBackend::shared();
    world
}

fn accountless_tuple_world() -> World {
    let tuple = TupleIdentity::new(Agent::Omp, Provider::OpenCode);
    let mut world = tuple_world(
        vec![supported_tuple(
            Agent::Omp,
            Provider::OpenCode,
            "opencode-default",
        )],
        vec![],
    );
    world.backend = SimBackend::guarded_accountless(tuple);
    world
}

fn launch_tuple(world: World, label: &str, expected: InstanceChoice) {
    let mut app = started(world);
    press(&mut app, "enter");
    let picker = screen(&mut app);
    assert!(picker.contains(label), "{label}\n{picker}");
    click_text(&mut app, label);
    press(&mut app, "enter");
    assert_eq!(app.route(), Route::Cockpit, "{}", screen(&mut app));
    tick_until(&mut app, "the selected tuple is recorded", |app| {
        app.world.instances.iter().any(|instance| {
            instance.agents.iter().any(|choice| {
                choice.agent == expected.agent && choice.provider == expected.provider
            })
        })
    });
    let launched = app
        .world
        .instances
        .iter()
        .find(|instance| {
            instance.agents.iter().any(|choice| {
                choice.agent == expected.agent && choice.provider == expected.provider
            })
        })
        .expect("the selected tuple launched");
    let [actual] = launched.agents.as_slice() else {
        panic!("one chosen tuple expected");
    };
    assert_eq!(actual.agent, expected.agent);
    assert_eq!(actual.provider, expected.provider);
    assert_eq!(actual.account, expected.account);
    assert_eq!(actual.model, expected.model);
}

fn blocked_tuple_stays_unlaunchable(world: World, label: &str, reason: &str) {
    let before = world.instances.clone();
    let mut app = started(world);
    press(&mut app, "enter");
    let picker = screen(&mut app);
    assert!(picker.contains(label), "{label}\n{picker}");
    assert!(picker.contains(reason), "{reason}\n{picker}");
    click_text(&mut app, label);
    press(&mut app, "enter");
    assert_eq!(app.route(), Route::Manager);
    assert!(screen(&mut app).contains("Launch · choose Agent"));
    assert_eq!(app.world.instances, before);
}

fn click_picker_option(app: &mut App, title: &str, label: &str) {
    let shown = screen(app);
    let lines: Vec<&str> = shown.lines().collect();
    let start = lines
        .iter()
        .position(|line| line.contains(title))
        .unwrap_or_else(|| panic!("{title} not shown:\n{shown}"));
    let (index, line) = lines
        .iter()
        .enumerate()
        .skip(start + 1)
        .find(|(_, line)| line.contains(label))
        .unwrap_or_else(|| panic!("{label} not shown below {title}:\n{shown}"));
    let x = line
        .split_once(label)
        .expect("option label")
        .0
        .chars()
        .count() as u16;
    click(app, x + 1, index as u16);
}

fn two_choice_world() -> World {
    let mut world = world_for(Scenario::Returning);
    world.surfaces.tuples = vec![
        supported_tuple(Agent::Codex, Provider::OpenAi, "codex-model"),
        supported_tuple(Agent::ClaudeCode, Provider::Anthropic, "claude-model"),
    ];
    world.backend = SimBackend::shared();
    world
        .global
        .agent_modes
        .insert(Agent::Codex, AuthMode::Sync);
    world
        .global
        .agent_modes
        .insert(Agent::ClaudeCode, AuthMode::Sync);
    for id in ["acct-codex-primary", "acct-codex-experiments"] {
        let account = world.accounts.get_mut(id).expect("Codex account");
        account.lifecycle = Lifecycle::Available;
        account.default_for_provider = false;
    }
    for id in ["acct-claude-personal", "acct-claude-work"] {
        let account = world.accounts.get_mut(id).expect("Claude account");
        account.lifecycle = Lifecycle::Available;
        account.default_for_provider = false;
    }
    let workspace = world.workspace_mut(1).expect("payments-platform");
    workspace.accounts.disabled_defaults.clear();
    workspace.accounts.enabled = [
        "acct-codex-primary".into(),
        "acct-codex-experiments".into(),
        "acct-claude-personal".into(),
        "acct-claude-work".into(),
    ]
    .into();
    workspace.accounts.preferred.clear();
    workspace.accounts.role_preferred.clear();
    world
}

fn automatic_first_account_world() -> World {
    let mut world = two_choice_world();
    world
        .workspace_mut(1)
        .expect("payments-platform")
        .accounts
        .preferred
        .insert(Provider::OpenAi, "acct-codex-primary".into());
    world
}

fn open_two_choice_later_account_picker(world: World, explicit_first: Option<&str>) -> App {
    let mut app = started(world);
    press(&mut app, "enter");
    click_picker_option(&mut app, "Launch · choose Agent", "Codex");
    click_picker_option(&mut app, "Launch · choose Agent", "Claude Code");
    press(&mut app, "enter");
    if screen(&mut app).contains("Account for Codex") {
        let account = explicit_first.expect("an unresolved first choice needs an explicit account");
        click_picker_option(&mut app, "Account for Codex", account);
    } else {
        assert!(
            explicit_first.is_none(),
            "the first choice was selected automatically"
        );
    }
    assert!(screen(&mut app).contains("Account for Claude Code"));
    app
}

fn snapshot_after(app: &App, change: impl FnOnce(&mut Snapshot)) -> Msg {
    let mut snapshot = sim::snapshot(&app.world);
    change(&mut snapshot);
    Msg::Refreshed {
        ticket: app.world.refresh_ticket,
        result: Ok(Box::new(snapshot)),
    }
}

fn select_later_claude_account(app: &mut App, label: &str) {
    click_picker_option(app, "Account for Claude Code", label);
}

fn assert_launch_invalidated(app: &mut App, before: &[jackin_console::domain::instance::Instance]) {
    let shown = screen(app);
    assert!(shown.contains("The launch options changed"), "{shown}");
    assert_eq!(app.route(), Route::Manager);
    assert_eq!(app.world.instances.as_slice(), before);
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
    assert_eq!(app.route(), Route::Cockpit, "{}", screen(app));
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
fn the_launch_picker_accepts_a_supported_accountless_tuple() {
    let mut world = world_for(Scenario::LaunchChecks);
    world.accounts.accounts.clear();
    world.surfaces.tuples = vec![TupleOffer::supported(
        Agent::Omp,
        Provider::OpenCode,
        &["opencode-default"],
    )];
    world.surfaces.agent_blocked = None;
    world.backend =
        SimBackend::guarded_accountless(TupleIdentity::new(Agent::Omp, Provider::OpenCode));
    let mut app = started(world);
    for _ in 0..2 {
        press(&mut app, "down");
    }
    press(&mut app, "enter");
    let picker = screen(&mut app);
    assert!(
        picker.contains("infra-control-plane › sentinel"),
        "{picker}"
    );
    press(&mut app, "space enter");
    assert_eq!(app.route(), Route::Cockpit, "{}", screen(&mut app));

    tick_until(
        &mut app,
        "the OMP launch questions",
        shows("Fail the preflight hook on purpose"),
    );
    press(&mut app, "tab enter h i enter");
    click_text(&mut app, "Start launch");
    tick_until(&mut app, "the OMP launch record", |app| {
        app.world.instances.iter().any(|instance| {
            instance
                .agents
                .iter()
                .any(|agent| agent.agent == Agent::Omp)
        })
    });
    let launched = app
        .world
        .instances
        .iter()
        .find(|instance| {
            instance
                .agents
                .iter()
                .any(|agent| agent.agent == Agent::Omp)
        })
        .expect("the tuple launched");
    let [agent] = launched.agents.as_slice() else {
        panic!("one chosen agent expected");
    };
    assert_eq!(agent.provider, Provider::OpenCode);
    assert_eq!(agent.account, None);
    assert_eq!(agent.model, "opencode-default");
}

#[test]
fn a_supported_tuple_and_zero_accounts_do_not_enable_accountless_by_default() {
    let mut world = world_for(Scenario::Returning);
    world.accounts.accounts.clear();
    world.surfaces.tuples = vec![supported_tuple(
        Agent::Omp,
        Provider::OpenCode,
        "opencode-default",
    )];
    world.backend = SimBackend::shared();
    let identity = TupleIdentity::new(Agent::Omp, Provider::OpenCode);
    assert_eq!(
        world.backend.account_requirement(identity),
        AccountRequirement::Required
    );
    let target = LaunchTarget {
        workspace: Some(1),
        role: "the-architect".into(),
        instances: vec![InstanceChoice {
            agent: Agent::Omp,
            provider: Provider::OpenCode,
            account: None,
            model: "opencode-default".into(),
        }],
        task: None,
    };
    let refusal = validate_tuples(&world, &target).expect("credentials are required");
    assert!(format!("{refusal:?}").contains("eligible provider account is required"));
}

#[test]
fn an_accountless_tuple_does_not_bypass_an_explicit_agent_block() {
    let mut world = accountless_tuple_world();
    world.surfaces.agent_blocked = Some(AgentBlocked {
        launchable: Agent::Codex,
        text: "only Codex can launch".into(),
    });
    blocked_tuple_stays_unlaunchable(world, "OMP", "only Codex can launch");
}

#[test]
fn an_accountless_tuple_does_not_bypass_an_ignored_runtime_mode() {
    let mut world = accountless_tuple_world();
    world
        .global
        .agent_modes
        .insert(Agent::Omp, AuthMode::Ignore);
    blocked_tuple_stays_unlaunchable(world, "OMP", "runtime mode");
}

#[test]
fn the_manager_records_credentialed_and_accountless_choices_in_order() {
    let mut world = world_for(Scenario::Returning);
    world.accounts.accounts.clear();
    world.surfaces.tuples = vec![
        supported_tuple(Agent::Codex, Provider::OpenAi, "codex-default"),
        supported_tuple(Agent::Omp, Provider::OpenCode, "opencode-default"),
    ];
    world.surfaces.agent_blocked = None;
    world
        .global
        .agent_modes
        .insert(Agent::Codex, AuthMode::Sync);
    world.global.agent_modes.insert(Agent::Omp, AuthMode::Sync);
    world.accounts.insert(ready_account(
        "fixture-codex",
        "Codex account",
        Provider::OpenAi,
    ));
    world.backend =
        SimBackend::guarded_accountless(TupleIdentity::new(Agent::Omp, Provider::OpenCode));

    let mut app = started(world);
    let before = app.world.instances.len();
    press(&mut app, "enter");
    click_picker_option(&mut app, "Launch · choose Agent", "Codex");
    click_picker_option(&mut app, "Launch · choose Agent", "OMP");
    press(&mut app, "enter");
    assert_eq!(app.route(), Route::Cockpit, "{}", screen(&mut app));
    tick_until(&mut app, "the new launch instance is recorded", |app| {
        app.world.instances.len() > before
    });

    let instance = app
        .world
        .instances
        .get(before)
        .expect("the two-choice launch record");
    let actual: Vec<_> = instance
        .agents
        .iter()
        .map(|choice| {
            (
                choice.agent,
                choice.provider,
                choice.account.clone(),
                choice.model.as_str(),
            )
        })
        .collect();
    assert_eq!(
        actual,
        [
            (
                Agent::Codex,
                Provider::OpenAi,
                Some("fixture-codex".into()),
                "codex-default"
            ),
            (Agent::Omp, Provider::OpenCode, None, "opencode-default"),
        ]
    );
}

#[test]
fn the_picker_routes_omp_kimi_to_its_moonshot_account_with_reversed_offers() {
    let kimi = ready_account("fixture-kimi", "Kimi account", Provider::Moonshot);
    let mimo = ready_account("fixture-mimo", "MiMo account", Provider::Mimo);
    let opencode = ready_account("fixture-opencode", "OpenCode account", Provider::OpenCode);
    let world = tuple_world(
        vec![
            supported_tuple(Agent::Omp, Provider::Mimo, "mimo-default"),
            supported_tuple(Agent::Omp, Provider::Moonshot, "kimi-default"),
            supported_tuple(Agent::Omp, Provider::OpenCode, "opencode-default"),
        ],
        vec![kimi, mimo, opencode],
    );
    launch_tuple(
        world,
        "OMP · Moonshot / Kimi",
        InstanceChoice {
            agent: Agent::Omp,
            provider: Provider::Moonshot,
            account: Some("fixture-kimi".into()),
            model: "kimi-default".into(),
        },
    );
}

#[test]
fn the_picker_routes_omp_mimo_to_its_mimo_account_with_reversed_offers() {
    let kimi = ready_account("fixture-kimi", "Kimi account", Provider::Moonshot);
    let mimo = ready_account("fixture-mimo", "MiMo account", Provider::Mimo);
    let opencode = ready_account("fixture-opencode", "OpenCode account", Provider::OpenCode);
    let world = tuple_world(
        vec![
            supported_tuple(Agent::Omp, Provider::OpenCode, "opencode-default"),
            supported_tuple(Agent::Omp, Provider::Moonshot, "kimi-default"),
            supported_tuple(Agent::Omp, Provider::Mimo, "mimo-default"),
        ],
        vec![kimi, mimo, opencode],
    );
    launch_tuple(
        world,
        "OMP · Mimo",
        InstanceChoice {
            agent: Agent::Omp,
            provider: Provider::Mimo,
            account: Some("fixture-mimo".into()),
            model: "mimo-default".into(),
        },
    );
}

#[test]
fn the_picker_keeps_omp_opencode_on_the_opencode_account() {
    let kimi = ready_account("fixture-kimi", "Kimi account", Provider::Moonshot);
    let mimo = ready_account("fixture-mimo", "MiMo account", Provider::Mimo);
    let opencode = ready_account("fixture-opencode", "OpenCode account", Provider::OpenCode);
    let world = tuple_world(
        vec![
            supported_tuple(Agent::Omp, Provider::Mimo, "mimo-default"),
            supported_tuple(Agent::Omp, Provider::Moonshot, "kimi-default"),
            supported_tuple(Agent::Omp, Provider::OpenCode, "opencode-default"),
        ],
        vec![kimi, mimo, opencode],
    );
    launch_tuple(
        world,
        "OMP · OpenCode",
        InstanceChoice {
            agent: Agent::Omp,
            provider: Provider::OpenCode,
            account: Some("fixture-opencode".into()),
            model: "opencode-default".into(),
        },
    );
}

#[test]
fn the_simulator_accepts_matching_credentials_and_refuses_a_mismatched_provider() {
    let world = tuple_world(
        vec![supported_tuple(
            Agent::Omp,
            Provider::Moonshot,
            "kimi-default",
        )],
        vec![
            ready_account("fixture-kimi", "Kimi account", Provider::Moonshot),
            ready_account("fixture-opencode", "OpenCode account", Provider::OpenCode),
        ],
    );
    let target = |account: &str| LaunchTarget {
        workspace: Some(1),
        role: "the-architect".into(),
        instances: vec![InstanceChoice {
            agent: Agent::Omp,
            provider: Provider::Moonshot,
            account: Some(account.into()),
            model: "kimi-default".into(),
        }],
        task: None,
    };

    assert_eq!(validate_tuples(&world, &target("fixture-kimi")), None);
    let refusal = validate_tuples(&world, &target("fixture-opencode"))
        .expect("an account from another provider is refused");
    let RefusalReason::TupleNotLaunchable { reason, .. } = refusal else {
        panic!("provider mismatch must be a tuple refusal");
    };
    assert_eq!(
        reason.as_deref(),
        Some("account is not eligible for this provider")
    );
}

#[test]
fn the_picker_refuses_two_provider_choices_for_the_same_omp_agent() {
    let world = tuple_world(
        vec![
            supported_tuple(Agent::Omp, Provider::Moonshot, "kimi-default"),
            supported_tuple(Agent::Omp, Provider::Mimo, "mimo-default"),
        ],
        vec![
            ready_account("fixture-kimi", "Kimi account", Provider::Moonshot),
            ready_account("fixture-mimo", "MiMo account", Provider::Mimo),
        ],
    );
    let before = world.instances.clone();
    let mut app = started(world);
    press(&mut app, "enter");
    click_picker_option(&mut app, "Launch · choose Agent", "OMP · Moonshot / Kimi");
    click_picker_option(&mut app, "Launch · choose Agent", "OMP · Mimo");
    let conflict = screen(&mut app);
    assert!(
        conflict.contains("Choose only one provider for each agent"),
        "{conflict}"
    );
    assert!(conflict.contains("Launch · choose Agent"));
    press(&mut app, "esc");
    assert_eq!(app.world.instances, before);
}

#[test]
fn the_picker_keeps_unsupported_omp_kimi_disabled_when_open_code_is_supported() {
    let kimi = ready_account("fixture-kimi", "Kimi account", Provider::Moonshot);
    let opencode = ready_account("fixture-opencode", "OpenCode account", Provider::OpenCode);
    let mut unavailable = supported_tuple(Agent::Omp, Provider::Moonshot, "kimi-default");
    unavailable.state = jackin_console::domain::tuple::TupleState::Unavailable;
    unavailable.reason = Some("not offered".into());
    let world = tuple_world(
        vec![
            supported_tuple(Agent::Omp, Provider::OpenCode, "opencode-default"),
            unavailable,
        ],
        vec![kimi, opencode],
    );
    let mut app = started(world);
    press(&mut app, "enter");
    let picker = screen(&mut app);
    assert!(picker.contains("OMP · Moonshot / Kimi"), "{picker}");
    assert!(picker.contains("not offered"), "{picker}");
    click_text(&mut app, "OMP · Moonshot / Kimi");
    press(&mut app, "enter");
    assert_eq!(app.route(), Route::Manager);
    assert!(screen(&mut app).contains("Launch · choose Agent"));
    assert!(app.world.instances.iter().all(|instance| {
        instance
            .agents
            .iter()
            .all(|choice| choice.agent != Agent::Omp)
    }));
}

#[test]
fn a_supported_omp_tuple_with_a_blocked_account_stays_disabled() {
    let mut blocked = ready_account("fixture-kimi", "Kimi account", Provider::Moonshot);
    blocked.lifecycle = Lifecycle::NeedsSecret;
    let world = tuple_world(
        vec![supported_tuple(
            Agent::Omp,
            Provider::Moonshot,
            "kimi-default",
        )],
        vec![blocked],
    );
    blocked_tuple_stays_unlaunchable(world, "OMP · Moonshot / Kimi", "needs secret");
}

#[test]
fn a_supported_omp_tuple_with_ignored_runtime_stays_disabled() {
    let mut world = tuple_world(
        vec![supported_tuple(
            Agent::Omp,
            Provider::Moonshot,
            "kimi-default",
        )],
        vec![ready_account(
            "fixture-kimi",
            "Kimi account",
            Provider::Moonshot,
        )],
    );
    world
        .global
        .agent_modes
        .insert(Agent::Omp, AuthMode::Ignore);
    blocked_tuple_stays_unlaunchable(world, "OMP · Moonshot / Kimi", "runtime mode");
}

#[test]
fn a_blocked_preferred_account_is_not_replaced_by_another_ready_account() {
    let mut blocked = ready_account("fixture-kimi-blocked", "Blocked Kimi", Provider::Moonshot);
    blocked.lifecycle = Lifecycle::NeedsSecret;
    let ready = ready_account("fixture-kimi-ready", "Ready Kimi", Provider::Moonshot);
    let mut world = tuple_world(
        vec![supported_tuple(
            Agent::Omp,
            Provider::Moonshot,
            "kimi-default",
        )],
        vec![blocked, ready],
    );
    world
        .workspace_mut(1)
        .expect("payments-platform")
        .accounts
        .preferred
        .insert(Provider::Moonshot, "fixture-kimi-blocked".into());
    blocked_tuple_stays_unlaunchable(world, "OMP · Moonshot / Kimi", "needs secret");
}

#[test]
fn an_account_collected_before_a_later_picker_is_revalidated_before_dispatch() {
    let mut app = open_two_choice_later_account_picker(automatic_first_account_world(), None);
    let before = app.world.instances.clone();
    let reload = snapshot_after(&app, |snapshot| {
        snapshot
            .accounts
            .get_mut("acct-codex-primary")
            .expect("first account")
            .lifecycle = Lifecycle::NeedsSecret;
    });
    app.deliver(&reload);
    select_later_claude_account(&mut app, "Work");
    assert_launch_invalidated(&mut app, &before);
}

#[test]
fn a_collected_tuple_becoming_unsupported_cancels_the_whole_launch() {
    let mut app = open_two_choice_later_account_picker(automatic_first_account_world(), None);
    let before = app.world.instances.clone();
    let reload = snapshot_after(&app, |snapshot| {
        let offer = snapshot
            .surfaces
            .tuples
            .iter_mut()
            .find(|offer| offer.agent == Agent::Codex && offer.provider == Provider::OpenAi)
            .expect("first tuple");
        offer.state = jackin_console::domain::tuple::TupleState::Unavailable;
        offer.reason = Some("Codex tuple became unavailable".into());
    });
    app.deliver(&reload);
    select_later_claude_account(&mut app, "Work");
    assert_launch_invalidated(&mut app, &before);
}

#[test]
fn a_collected_provider_change_cancels_the_whole_launch() {
    let mut app = open_two_choice_later_account_picker(automatic_first_account_world(), None);
    let before = app.world.instances.clone();
    let reload = snapshot_after(&app, |snapshot| {
        snapshot
            .surfaces
            .tuples
            .retain(|offer| offer.agent != Agent::Codex || offer.provider != Provider::OpenAi);
        snapshot.surfaces.tuples.push(supported_tuple(
            Agent::Codex,
            Provider::OpenCode,
            "codex-opencode-model",
        ));
    });
    app.deliver(&reload);
    select_later_claude_account(&mut app, "Work");
    assert_launch_invalidated(&mut app, &before);
}

#[test]
fn a_collected_model_change_cancels_the_whole_launch() {
    let mut app = open_two_choice_later_account_picker(automatic_first_account_world(), None);
    let before = app.world.instances.clone();
    let reload = snapshot_after(&app, |snapshot| {
        let offer = snapshot
            .surfaces
            .tuples
            .iter_mut()
            .find(|offer| offer.agent == Agent::Codex && offer.provider == Provider::OpenAi)
            .expect("first tuple");
        offer.default_model = "codex-model-updated".into();
        offer.allowed_models = vec![offer.default_model.clone()];
    });
    app.deliver(&reload);
    select_later_claude_account(&mut app, "Work");
    assert_launch_invalidated(&mut app, &before);
}

#[test]
fn a_later_account_becoming_unusable_cancels_the_whole_launch() {
    let mut app = open_two_choice_later_account_picker(automatic_first_account_world(), None);
    let before = app.world.instances.clone();
    let reload = snapshot_after(&app, |snapshot| {
        snapshot
            .accounts
            .get_mut("acct-claude-work")
            .expect("later account")
            .lifecycle = Lifecycle::NeedsSecret;
    });
    app.deliver(&reload);
    select_later_claude_account(&mut app, "Work");
    assert_launch_invalidated(&mut app, &before);
}

#[test]
fn an_automatic_account_binding_change_is_refused_even_when_old_account_stays_ready() {
    let mut app = open_two_choice_later_account_picker(automatic_first_account_world(), None);
    let before = app.world.instances.clone();
    let reload = snapshot_after(&app, |snapshot| {
        snapshot
            .workspaces
            .iter_mut()
            .find(|workspace| workspace.id == 1)
            .expect("workspace")
            .accounts
            .preferred
            .insert(Provider::OpenAi, "acct-codex-experiments".into());
    });
    app.deliver(&reload);
    select_later_claude_account(&mut app, "Work");
    assert_launch_invalidated(&mut app, &before);
    assert_eq!(
        app.world
            .accounts
            .get("acct-codex-primary")
            .expect("old automatic account")
            .lifecycle,
        Lifecycle::Available
    );
}

#[test]
fn an_automatic_binding_change_before_confirmation_refuses_the_selected_account() {
    let mut app = started(automatic_first_account_world());
    press(&mut app, "enter");
    click_picker_option(&mut app, "Launch · choose Agent", "Codex");
    let before = app.world.instances.clone();
    let reload = snapshot_after(&app, |snapshot| {
        snapshot
            .workspaces
            .iter_mut()
            .find(|workspace| workspace.id == 1)
            .expect("workspace")
            .accounts
            .preferred
            .insert(Provider::OpenAi, "acct-codex-experiments".into());
    });
    app.deliver(&reload);
    press(&mut app, "enter");
    assert_launch_invalidated(&mut app, &before);
}

#[test]
fn an_unchanged_automatic_binding_launches_the_selected_account() {
    let mut app = started(automatic_first_account_world());
    press(&mut app, "enter");
    click_picker_option(&mut app, "Launch · choose Agent", "Codex");
    press(&mut app, "enter");
    assert_eq!(app.route(), Route::Cockpit, "{}", screen(&mut app));
    tick_until(&mut app, "the automatic account is recorded", |app| {
        app.world.instances.iter().any(|instance| {
            instance.agents.iter().any(|choice| {
                choice.agent == Agent::Codex
                    && choice.account.as_deref() == Some("acct-codex-primary")
            })
        })
    });
    let instance = app
        .world
        .instances
        .iter()
        .find(|instance| {
            instance.agents.iter().any(|choice| {
                choice.agent == Agent::Codex
                    && choice.account.as_deref() == Some("acct-codex-primary")
            })
        })
        .expect("the automatically selected account");
    let [choice] = instance.agents.as_slice() else {
        panic!("one selected tuple expected");
    };
    assert_eq!(choice.provider, Provider::OpenAi);
    assert_eq!(choice.model, "codex-model");
}

#[test]
fn an_explicit_account_remains_selected_after_the_automatic_preference_changes() {
    let mut app = open_two_choice_later_account_picker(two_choice_world(), Some("Primary"));
    let reload = snapshot_after(&app, |snapshot| {
        snapshot
            .workspaces
            .iter_mut()
            .find(|workspace| workspace.id == 1)
            .expect("workspace")
            .accounts
            .preferred
            .insert(Provider::OpenAi, "acct-codex-experiments".into());
    });
    app.deliver(&reload);
    select_later_claude_account(&mut app, "Work");
    assert_eq!(app.route(), Route::Cockpit, "{}", screen(&mut app));
    tick_until(
        &mut app,
        "both explicit and selected tuples launch",
        |app| {
            app.world.instances.iter().any(|instance| {
                instance.agents.len() == 2
                    && instance.agents.iter().any(|choice| {
                        choice.agent == Agent::Codex
                            && choice.provider == Provider::OpenAi
                            && choice.account.as_deref() == Some("acct-codex-primary")
                    })
                    && instance.agents.iter().any(|choice| {
                        choice.agent == Agent::ClaudeCode
                            && choice.provider == Provider::Anthropic
                            && choice.account.as_deref() == Some("acct-claude-work")
                    })
            })
        },
    );
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

fn target(world: &World, workspace: u32, role: &str) -> LaunchTarget {
    let account = world
        .offer_for_provider(
            Agent::Codex,
            Provider::OpenAi,
            world.workspace(workspace),
            Some(role),
        )
        .accounts
        .into_iter()
        .next()
        .expect("the fixture offers an eligible Codex account");
    LaunchTarget {
        workspace: Some(workspace),
        role: role.into(),
        instances: vec![jackin_console::domain::tuple::InstanceChoice {
            agent: Agent::Codex,
            provider: Provider::OpenAi,
            account: Some(account),
            model: "gpt-5-codex".into(),
        }],
        task: None,
    }
}

#[test]
fn the_guarded_sim_sends_the_refusal_before_any_stage() {
    let world = world_for(Scenario::LaunchChecks);
    let mut feed = SimBackend::guarded().launch(&world, &target(&world, PORTAL, "writer"));
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
    let mut feed = SimBackend::guarded().launch(&world, &target(&world, INFRA, "sentinel"));
    let events = feed.poll(&world);
    let asked: Vec<_> = events
        .iter()
        .filter_map(|e| match e {
            LaunchEvent::NeedsInput(q) => Some(q.clone()),
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
        ]],
        "{events:?}"
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
    let mut feed = SimBackend::guarded().launch(&world, &target(&world, INFRA, "sentinel"));
    feed.poll(&world);
    assert_eq!(feed.cancel(&world), None);
}

#[test]
fn a_role_without_questions_starts_at_once() {
    let world = world_for(Scenario::LaunchChecks);
    let mut feed = SimBackend::guarded().launch(&world, &target(&world, 1, "the-architect"));
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
