use std::collections::BTreeSet;

use super::workspace::RoleName;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DockerSetting {
    pub gateway: String,
    pub engine: EngineState,
    pub risks: Vec<DockerRiskText>,
    pub role_enabled: BTreeSet<RoleName>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EngineState {
    Configured { endpoint: String },
    Missing,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DockerRiskText {
    pub label: String,
    pub text: String,
}
