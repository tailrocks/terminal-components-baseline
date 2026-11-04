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
    Configured { endpoint: EngineEndpoint },
    Missing,
}

/// Where the engine answers: an address a host knows by name, or the port of
/// a TLS socket that the vocabulary words.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EngineEndpoint {
    Address(String),
    TlsPort(std::num::NonZeroU16),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DockerRiskText {
    pub label: String,
    pub text: String,
}

/// What a launch tells the cockpit about the Docker engine its agent can
/// reach: the risks the banner counts and the debug info lists.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DockerCapability {
    pub risks: Vec<DockerRiskText>,
}
