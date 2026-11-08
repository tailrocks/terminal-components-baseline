use super::account::AccountId;
use super::agent::{Agent, Provider};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TupleState {
    Supported,
    Unavailable,
    BlockedVendor,
}

impl TupleState {
    pub fn label(self) -> &'static str {
        match self {
            TupleState::Supported => "supported",
            TupleState::Unavailable => "unavailable",
            TupleState::BlockedVendor => "blocked by vendor",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TupleOffer {
    pub agent: Agent,
    pub provider: Provider,
    pub state: TupleState,
    pub reason: Option<String>,
    pub allowed_models: Vec<String>,
    pub default_model: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstanceChoice {
    pub agent: Agent,
    pub provider: Provider,
    pub account: Option<AccountId>,
    pub model: String,
}

impl TupleOffer {
    pub fn supported(agent: Agent, provider: Provider, models: &[&str]) -> Self {
        Self {
            agent,
            provider,
            state: TupleState::Supported,
            reason: None,
            allowed_models: models.iter().map(|m| m.to_string()).collect(),
            default_model: models.first().map(|m| m.to_string()).unwrap_or_default(),
        }
    }
}
