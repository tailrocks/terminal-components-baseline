use super::account::AccountForm;
use super::agent::AgentBlocked;
use super::docker::DockerSetting;
use super::doctor::DoctorSurface;
use super::fixed::Fixed;
use super::role::{LaunchRoleRule, RoleAdmin};
use super::vocabulary::Vocabulary;
use super::workspace::WizardDefaults;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Surfaces {
    pub roles: Option<RoleAdmin>,
    pub docker: Option<DockerSetting>,
    pub doctor: Option<DoctorSurface>,
    pub vocabulary: Vocabulary,
    pub account_form: AccountForm,
    pub agent_blocked: Option<AgentBlocked>,
    pub launch_role: LaunchRoleRule,
    pub wizard: Option<WizardDefaults>,
    pub fixed: Option<Fixed>,
    pub version: String,
    pub cockpit_version: Option<String>,
    pub config_path: Option<String>,
    pub about: Vec<(String, String)>,
}

/// What a live host knows about itself. Every other static field of
/// `Surfaces::openshell` is baseline data.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LiveFacts {
    pub version: String,
    pub config_path: String,
}

impl Surfaces {
    pub fn openshell(facts: LiveFacts) -> Self {
        let LiveFacts {
            version,
            config_path,
        } = facts;
        Self {
            roles: None,
            docker: None,
            doctor: None,
            vocabulary: Vocabulary::openshell(),
            account_form: AccountForm::openshell(),
            agent_blocked: Some(AgentBlocked::openshell()),
            launch_role: LaunchRoleRule::Configured,
            wizard: Some(WizardDefaults::openshell()),
            fixed: Some(Fixed::openshell()),
            version,
            cockpit_version: None,
            config_path: Some(config_path),
            about: Vec::new(),
        }
    }

    pub fn roles(mut self, roles: RoleAdmin) -> Self {
        self.roles = Some(roles);
        self
    }

    pub fn docker(mut self, docker: DockerSetting) -> Self {
        self.docker = Some(docker);
        self
    }

    pub fn doctor(mut self, doctor: DoctorSurface) -> Self {
        self.doctor = Some(doctor);
        self
    }

    pub fn about(mut self, rows: Vec<(String, String)>) -> Self {
        self.about = rows;
        self
    }
}
