use std::collections::BTreeSet;

use crate::domain::account::{AccountForm, CredentialSource, Provenance};
use crate::domain::agent::{Agent, AgentBlocked, AuthMode};
use crate::domain::docker::{DockerRiskText, DockerSetting, EngineState};
use crate::domain::doctor::{DoctorState, DoctorSurface};
use crate::domain::fixed::Fixed;
use crate::domain::openshell;
use crate::domain::role::LaunchRoleRule;
use crate::domain::role::{
    AccountOrder, RoleAdmin, RoleCheckState, RoleEnvEntry, RoleManifestDoc, RoleRecord,
};
use crate::domain::surfaces::Surfaces;
use crate::domain::vocabulary::Vocabulary;
use crate::domain::workspace::{AllowedRoles, EnvValue, RoleSource};
use crate::fixtures::{Scenario, about_rows};
use crate::sim;
use crate::world::World;

impl Surfaces {
    pub fn demo(scenario: Scenario) -> Self {
        Self {
            roles: None,
            docker: None,
            doctor: None,
            vocabulary: Vocabulary::demo(),
            account_form: AccountForm::default(),
            agent_blocked: None,
            launch_role: LaunchRoleRule::Demo,
            wizard: None,
            fixed: None,
            version: "0.9.2".into(),
            cockpit_version: Some("0.6.4 · preview".into()),
            config_path: None,
            about: about_rows(scenario),
        }
    }
}

pub const GATEWAY: &str = "jackin-local";
const ENDPOINT: &str = "host.openshell.internal:18476";

fn risk(label: &str, text: &str) -> DockerRiskText {
    DockerRiskText {
        label: label.into(),
        text: text.into(),
    }
}

pub fn docker_setting(engine: EngineState) -> DockerSetting {
    DockerSetting {
        gateway: GATEWAY.into(),
        engine,
        risks: vec![
            risk("Client key", "The agent can read the engine client key."),
            risk("Network", "Containers bypass the sandbox network policy."),
            risk(
                "Bind mounts",
                "A bind mount names a path on the engine host, not in the sandbox.",
            ),
            risk(
                "Lifetime",
                "The engine outlives the sandbox. Sandboxes that share one engine see each other's containers, and you remove them.",
            ),
        ],
        role_enabled: BTreeSet::from(["reviewer".to_owned()]),
    }
}

pub fn docker(w: &mut World, engine_missing: bool) {
    let engine = if engine_missing {
        EngineState::Missing
    } else {
        EngineState::Configured {
            endpoint: ENDPOINT.into(),
        }
    };
    w.surfaces.docker = Some(docker_setting(engine));
    if let Some(ws) = w.workspace_mut(2) {
        ws.docker = true;
    }
}

fn doc(name: &str, docker: bool) -> RoleManifestDoc {
    RoleManifestDoc {
        name: name.into(),
        dockerfile: "Dockerfile".into(),
        policy: "policy.yaml".into(),
        codex_model: None,
        docker,
        hook_setup: Some("hooks/setup.sh".into()),
        hook_preflight: Some("hooks/preflight.sh".into()),
        env: vec![],
    }
}

fn entry(name: &str, default: Option<&str>, prompt: &str) -> RoleEnvEntry {
    RoleEnvEntry {
        name: name.into(),
        default: default.map(Into::into),
        prompt: Some(prompt.into()),
    }
}

fn git() -> RoleSource {
    RoleSource::Git {
        url: "github.com/chainargos/roles".into(),
        branch: "main".into(),
    }
}

fn local(name: &str) -> RoleSource {
    RoleSource::Local {
        path: format!("~/roles/{name}"),
    }
}

fn files() -> Vec<String> {
    [
        "Dockerfile",
        "Dockerfile.old",
        "policy.yaml",
        "hooks/setup.sh",
        "hooks/preflight.sh",
    ]
    .iter()
    .map(|f| (*f).to_owned())
    .collect()
}

fn record(
    name: &str,
    source: RoleSource,
    manifest: Result<RoleManifestDoc, String>,
    bindings: &[(Agent, &str)],
) -> RoleRecord {
    let check = match &manifest {
        Ok(d) => RoleCheckState::Done(sim::surfaces::check_role(d)),
        Err(_) => RoleCheckState::Unchecked,
    };
    RoleRecord {
        name: name.into(),
        source,
        manifest,
        check,
        bindings: bindings
            .iter()
            .map(|(a, id)| (*a, (*id).to_owned()))
            .collect(),
        files: files(),
    }
}

pub fn roles(w: &mut World, agents: Vec<Agent>) {
    let mut sentinel = doc("sentinel", false);
    sentinel.env = vec![
        entry(
            "SENTINEL_FAIL_PREFLIGHT",
            Some("0"),
            "Fail the preflight hook on purpose",
        ),
        entry("SENTINEL_NOTE", None, "Note for the run"),
    ];
    let mut data_eng = doc("data-eng", false);
    data_eng.policy = "missing.yaml".into();
    w.surfaces.roles = Some(RoleAdmin {
        agents,
        order: AccountOrder::Jackin,
        roles: vec![
            record(
                "sentinel",
                local("sentinel"),
                Ok(sentinel),
                &[(Agent::Codex, "acct-codex-primary")],
            ),
            record("the-architect", git(), Ok(doc("the-architect", false)), &[]),
            record(
                "reviewer",
                git(),
                Ok(doc("reviewer", true)),
                &[(Agent::Codex, "acct-codex-experiments")],
            ),
            record("data-eng", local("data-eng"), Ok(data_eng), &[]),
            record(
                "writer",
                local("writer"),
                Err("~/roles/writer/jackin.role.toml: unknown field `toolz` at line 4".into()),
                &[],
            ),
        ],
    });
}

/// `infra-control-plane` launches the role whose manifest asks for values,
/// and `customer-portal` no longer allows the role it last used. Only Codex
/// launches, so the first agent of the picker is the one the checks sit over.
pub fn launch_checks(w: &mut World) {
    w.surfaces.agent_blocked = Some(AgentBlocked::openshell());
    if let Some(ws) = w.workspace_mut(2) {
        ws.roles.default = Some("sentinel".into());
        ws.roles.last = Some("sentinel".into());
    }
    if let Some(ws) = w.workspace_mut(4) {
        ws.roles.allowed = AllowedRoles::Custom(vec!["backend".into()]);
    }
}

pub fn doctor(w: &mut World) {
    let at = w.now_secs() - 4;
    w.surfaces.doctor = Some(DoctorSurface {
        noun: "gateway".into(),
        state: DoctorState::Done(sim::surfaces::doctor_report(1, at)),
    });
}

fn plain(e: &crate::domain::workspace::EnvVar) -> bool {
    matches!(e.value, EnvValue::Plain(_))
}

pub fn fixed(w: &mut World) {
    openshell::surfaces(&mut w.surfaces);
    w.surfaces.config_path = Some(format!("{}/.config/jackin/config.toml", w.home));
    openshell::global(&mut w.global);
    w.global
        .agent_modes
        .insert(Fixed::openshell().launchable, AuthMode::Sync);
    w.global.env.retain(plain);
    for a in &mut w.accounts.accounts {
        openshell::account(a);
        if let CredentialSource::HostEnv { var, .. } = &a.source {
            a.edit_blocked = Some(openshell::env_edit_block(var));
        }
        if a.provenance == [Provenance::LiveHost] {
            a.provenance = vec![Provenance::ConfiguredSource];
        }
        if a.display_name == "discovered" {
            a.display_name = "host login".into();
        }
    }
    w.roles.iter_mut().for_each(openshell::role);
    w.instances.iter_mut().for_each(openshell::instance);
    for ws in &mut w.workspaces {
        openshell::workspace(ws, &format!("~/src/{}", ws.name), &w.accounts);
        if ws.name == "release-automation" {
            ws.roles.allowed = AllowedRoles::Custom(vec!["backend".into(), "ghost-role".into()]);
        }
        ws.env.retain(plain);
        for vars in ws.role_env.values_mut() {
            vars.retain(plain);
        }
    }
}
