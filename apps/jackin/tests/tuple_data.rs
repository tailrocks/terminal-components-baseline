#![cfg(feature = "demo")]

use jackin_console::backend::Backend;
use jackin_console::domain::agent::{Agent, Provider};
use jackin_console::domain::instance::{AgentInstanceView, AgentState};
use jackin_console::domain::launch::RefusalReason;
use jackin_console::domain::tuple::{InstanceChoice, TupleOffer, TupleState};
use jackin_console::fixtures::{Scenario, world_for};
use jackin_console::launch::LaunchTarget;
use jackin_console::sim::SimBackend;
use jackin_console::sim::launch::validate_tuples;

fn choice(agent: Agent, provider: Provider, model: &str) -> InstanceChoice {
    InstanceChoice {
        agent,
        provider,
        account: None,
        model: model.into(),
    }
}

fn target(instances: Vec<InstanceChoice>) -> LaunchTarget {
    LaunchTarget {
        workspace: Some(1),
        role: "the-architect".into(),
        instances,
        task: Some("ship it".into()),
    }
}

fn world_with(tuples: Vec<TupleOffer>) -> jackin_console::world::World {
    let mut w = world_for(Scenario::Returning);
    w.surfaces.tuples = tuples;
    w
}

#[test]
fn valid_supported_choice_is_accepted_and_task_passes_through() {
    let w = world_with(vec![TupleOffer::supported(
        Agent::Codex,
        Provider::OpenAi,
        &["gpt-5-codex"],
    )]);
    let t = target(vec![choice(Agent::Codex, Provider::OpenAi, "gpt-5-codex")]);
    assert_eq!(validate_tuples(&w, &t), None);
    assert_eq!(t.task.as_deref(), Some("ship it"));
    assert_eq!(t.role, "the-architect");
}

#[test]
fn empty_duplicate_unknown_unsupported_and_unknown_model_are_refused() {
    let supported = TupleOffer::supported(Agent::Codex, Provider::OpenAi, &["gpt-5-codex"]);
    let unavailable = TupleOffer {
        state: TupleState::Unavailable,
        reason: Some("no login".into()),
        ..TupleOffer::supported(Agent::Omp, Provider::OpenCode, &["omp-default"])
    };
    let blocked = TupleOffer {
        state: TupleState::BlockedVendor,
        reason: Some("vendor blocked".into()),
        ..TupleOffer::supported(Agent::MimoCode, Provider::Mimo, &["mimo-1"])
    };
    let w = world_with(vec![supported, unavailable, blocked]);

    let empty = validate_tuples(&w, &target(vec![])).expect("empty refused");
    assert!(matches!(
        empty,
        RefusalReason::TupleNotLaunchable {
            state: TupleState::Unavailable,
            ..
        }
    ));

    let dup = validate_tuples(
        &w,
        &target(vec![
            choice(Agent::Codex, Provider::OpenAi, "gpt-5-codex"),
            choice(Agent::Codex, Provider::OpenAi, "gpt-5-codex"),
        ]),
    )
    .expect("duplicate refused");
    assert!(matches!(
        dup,
        RefusalReason::TupleNotLaunchable {
            agent: Agent::Codex,
            ..
        }
    ));

    let unknown = validate_tuples(&w, &target(vec![choice(Agent::Omp, Provider::Mimo, "x")]))
        .expect("unknown tuple refused");
    assert!(matches!(
        unknown,
        RefusalReason::TupleNotLaunchable {
            agent: Agent::Omp,
            provider: Provider::Mimo,
            ..
        }
    ));

    let unsup = validate_tuples(
        &w,
        &target(vec![choice(Agent::Omp, Provider::OpenCode, "omp-default")]),
    )
    .expect("unavailable refused");
    assert!(matches!(
        unsup,
        RefusalReason::TupleNotLaunchable {
            state: TupleState::Unavailable,
            reason: Some(_),
            ..
        }
    ));

    let vendor = validate_tuples(
        &w,
        &target(vec![choice(Agent::MimoCode, Provider::Mimo, "mimo-1")]),
    )
    .expect("blocked vendor refused");
    assert!(matches!(
        vendor,
        RefusalReason::TupleNotLaunchable {
            state: TupleState::BlockedVendor,
            ..
        }
    ));

    let model = validate_tuples(
        &w,
        &target(vec![choice(Agent::Codex, Provider::OpenAi, "nope")]),
    )
    .expect("unknown model refused");
    assert!(matches!(
        model,
        RefusalReason::TupleNotLaunchable {
            agent: Agent::Codex,
            ..
        }
    ));
}

#[test]
fn tuple_classification_covers_supported_unavailable_blocked_and_zero_offers() {
    assert_eq!(TupleState::Supported.label(), "supported");
    assert_eq!(TupleState::Unavailable.label(), "unavailable");
    assert_eq!(TupleState::BlockedVendor.label(), "blocked by vendor");
    let w = world_with(vec![]);
    let refused = validate_tuples(
        &w,
        &target(vec![choice(Agent::Codex, Provider::OpenAi, "gpt-5-codex")]),
    );
    assert!(refused.is_some(), "zero offers refuse every choice");
}

#[test]
fn new_agents_and_mimo_provider_have_labels() {
    assert_eq!(Agent::MuseCode.label(), "Muse");
    assert_eq!(Agent::Antigravity.label(), "Antigravity");
    assert_eq!(Agent::MinimaxCode.label(), "MiniMax Code");
    assert_eq!(Agent::MimoCode.label(), "Mimo Code");
    assert_eq!(Agent::Omp.label(), "OMP");
    assert_eq!(Provider::Mimo.label(), "Mimo");
    assert!(Provider::ALL.contains(&Provider::Mimo));
    assert!(Agent::ALL.contains(&Agent::Omp));
}

#[test]
fn initial_panes_follow_instance_agents_in_order() {
    let w = world_for(Scenario::CapsuleTerminal);
    let instance = w
        .instances
        .iter()
        .find(|i| i.id == "jk-7f3a")
        .expect("capsule-terminal instance");
    assert_eq!(instance.agents.len(), 2);
    assert_eq!(
        instance.task.as_ref().map(|t| t.text.as_str()),
        Some("Ship the multi-agent launch")
    );
    let backend = SimBackend::shared();
    let panes = backend.initial_panes(instance);
    assert_eq!(panes.len(), 2);
    let kinds: Vec<String> = panes.iter().map(|p| format!("{:?}", p.kind)).collect();
    assert!(kinds[0].contains("Codex"), "{kinds:?}");
    assert!(kinds[1].contains("Omp"), "{kinds:?}");
    let _ = AgentInstanceView {
        agent: Agent::Codex,
        provider: Provider::OpenAi,
        account: None,
        model: String::new(),
        branch: None,
        state: AgentState::Idle,
    };
}
