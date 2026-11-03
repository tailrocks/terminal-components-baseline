use super::account::AccountForm;
use super::agent::AgentBlocked;
use super::docker::DockerSetting;
use super::doctor::DoctorSurface;
use super::role::{LaunchRoleRule, RoleAdmin};
use super::vocabulary::Vocabulary;
use super::workspace::WizardDefaults;

#[derive(Debug, Clone)]
pub struct Surfaces {
    pub roles: Option<RoleAdmin>,
    pub docker: Option<DockerSetting>,
    pub doctor: Option<DoctorSurface>,
    pub vocabulary: Vocabulary,
    pub account_form: AccountForm,
    pub agent_blocked: Option<AgentBlocked>,
    pub launch_role: LaunchRoleRule,
    pub wizard: Option<WizardDefaults>,
    pub version: String,
    pub about: Option<Vec<(String, String)>>,
}

impl Default for Surfaces {
    fn default() -> Self {
        Self {
            roles: None,
            docker: None,
            doctor: None,
            vocabulary: Vocabulary::default(),
            account_form: AccountForm::default(),
            agent_blocked: None,
            launch_role: LaunchRoleRule::default(),
            wizard: None,
            version: "0.9.2".into(),
            about: None,
        }
    }
}
