use super::account::AccountForm;
use super::agent::AgentBlocked;
use super::docker::DockerSetting;
use super::doctor::DoctorSurface;
use super::fixed::Fixed;
use super::fixed_fields::SurfaceFixed;
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
    /// The gateway the console talks to, when the config names one.
    pub gateway: Option<String>,
}

impl Surfaces {
    pub fn openshell(facts: LiveFacts) -> Self {
        let LiveFacts {
            version,
            config_path,
            gateway,
        } = facts;
        let SurfaceFixed {
            vocabulary,
            fixed,
            account_form,
            agent_blocked,
            wizard,
            launch_role,
        } = SurfaceFixed::openshell();
        let about = about_rows(&vocabulary, &version, gateway, &config_path);
        Self {
            roles: None,
            docker: None,
            doctor: None,
            vocabulary,
            account_form,
            agent_blocked,
            launch_role,
            wizard,
            fixed,
            version,
            cockpit_version: None,
            config_path: Some(config_path),
            about,
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
}

fn about_rows(
    vocabulary: &Vocabulary,
    version: &str,
    gateway: Option<String>,
    config_path: &str,
) -> Vec<(String, String)> {
    let mut rows = vec![(vocabulary.about_product.clone(), version.to_owned())];
    if let Some(gateway) = gateway {
        rows.push((vocabulary.about_gateway.clone(), gateway));
    }
    rows.push((vocabulary.about_config.clone(), config_path.to_owned()));
    rows
}
