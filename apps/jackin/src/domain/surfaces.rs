use super::account::AccountForm;
use super::agent::AgentBlocked;
use super::docker::DockerSetting;
use super::doctor::DoctorSurface;
use super::role::{LaunchRoleRule, RoleAdmin};
use super::vocabulary::Vocabulary;
use super::workspace::WizardDefaults;

#[derive(Debug, Clone, Default)]
pub struct Surfaces {
    pub roles: Option<RoleAdmin>,
    pub docker: Option<DockerSetting>,
    pub doctor: Option<DoctorSurface>,
    pub vocabulary: Vocabulary,
    pub account_form: AccountForm,
    pub agent_blocked: Option<AgentBlocked>,
    pub launch_role: LaunchRoleRule,
    pub wizard: Option<WizardDefaults>,
}
