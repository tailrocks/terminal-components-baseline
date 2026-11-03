use std::collections::BTreeMap;

use super::account::AccountId;
use super::agent::Agent;
use super::workspace::{RoleName, RoleSource};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AccountOrder {
    #[default]
    Demo,
    Jackin,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LaunchRoleRule {
    #[default]
    Demo,
    Configured,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LaunchRole {
    Role(RoleName),
    Unavailable(String),
}

pub const NO_DEFAULT_ROLE: &str = "no default role · set it in Roles";
pub const NO_SAVED_WORKSPACE: &str = "no saved workspace · press n, then set a default role";

#[derive(Debug, Clone)]
pub struct RoleAdmin {
    pub agents: Vec<Agent>,
    pub roles: Vec<RoleRecord>,
    pub order: AccountOrder,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RoleRecord {
    pub name: RoleName,
    pub source: RoleSource,
    pub manifest: Result<RoleManifestDoc, String>,
    pub check: RoleCheckState,
    pub bindings: BTreeMap<Agent, AccountId>,
    pub files: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RoleCheckState {
    Unchecked,
    Running,
    Done(Vec<RoleFinding>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FindingArea {
    Manifest,
    Dockerfile,
    Policy,
    Hook,
    Provider,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RoleFinding {
    pub area: FindingArea,
    pub key: Option<String>,
    pub message: String,
    pub fix: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RoleManifestDoc {
    pub name: String,
    pub dockerfile: String,
    pub policy: String,
    pub codex_model: Option<String>,
    pub docker: bool,
    pub hook_setup: Option<String>,
    pub hook_preflight: Option<String>,
    pub env: Vec<RoleEnvEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RoleEnvEntry {
    pub name: String,
    pub default: Option<String>,
    pub prompt: Option<String>,
}

impl RoleRecord {
    pub fn writable(&self) -> bool {
        matches!(self.source, RoleSource::Local { .. })
    }

    pub fn findings(&self) -> &[RoleFinding] {
        match &self.check {
            RoleCheckState::Done(f) => f,
            _ => &[],
        }
    }
}

impl RoleAdmin {
    pub fn role(&self, name: &str) -> Option<&RoleRecord> {
        self.roles.iter().find(|r| r.name == name)
    }

    pub fn role_mut(&mut self, name: &str) -> Option<&mut RoleRecord> {
        self.roles.iter_mut().find(|r| r.name == name)
    }

    pub fn binding(&self, role: &str, agent: Agent) -> Option<&AccountId> {
        self.role(role)?.bindings.get(&agent)
    }

    pub fn binding_for_provider(
        &self,
        role: &str,
        provider: super::agent::Provider,
    ) -> Option<&AccountId> {
        self.role(role)?
            .bindings
            .iter()
            .find(|(a, _)| a.provider() == provider)
            .map(|(_, id)| id)
    }

    pub fn docker_roles(&self) -> impl Iterator<Item = &RoleName> {
        self.roles
            .iter()
            .filter(|r| r.manifest.as_ref().is_ok_and(|m| m.docker))
            .map(|r| &r.name)
    }
}
