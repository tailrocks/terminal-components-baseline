use junie_tui::widgets::steps::StepState;

use crate::backend::OnePassword;
use crate::domain::account::AccountId;
use crate::domain::agent::Agent;
use crate::domain::instance::Instance;
use crate::domain::launch::{LaunchContent, LaunchFailure};
use crate::domain::workspace::WorkspaceId;
use crate::world::World;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LaunchTarget {
    pub workspace: Option<WorkspaceId>,
    pub role: String,
    pub agent: Agent,
    pub account: Option<AccountId>,
}

/// The instance a launch made, as its backend recorded it, and the run that
/// made it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReadyInstance {
    pub instance: Instance,
    pub run_id: String,
}

/// Why a launch failed, and the `FailedSetup` record the backend kept of it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FailedLaunch {
    pub failure: LaunchFailure,
    pub instance: Instance,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LaunchEvent {
    StageChanged(usize, StepState),
    /// How long a finished stage took. A backend that does not time its
    /// stages sends none, and the rail shows none.
    StageTimed {
        stage: usize,
        elapsed_ms: u64,
    },
    Activity(String),
    BuildLine(String),
    ContainerReady(String),
    CredentialsResolved {
        origin: String,
        validation: String,
    },

    CredentialError {
        message: String,
    },
    Failed(Box<FailedLaunch>),
    Ready(Box<ReadyInstance>),
}

/// What the credentials line says once the operator chose to type the key.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypedKey {
    pub origin: String,
    pub validation: String,
}

/// What a backend gives the cockpit for one launch. The cockpit polls it on
/// every step and draws what the events say; it records nothing in the world
/// except the instances the events and `cancel` hand it.
pub trait LaunchFeed: std::fmt::Debug + Send {
    /// The id of the run, shown from the first frame.
    fn run_id(&self) -> &str;

    /// The collector the run reports to. A backend that sends no telemetry
    /// names none, and the debug info shows no telemetry line.
    fn telemetry(&self) -> Option<&str> {
        None
    }

    /// The events that happened since the last poll, in order.
    fn poll(&mut self, world: &World) -> Vec<LaunchEvent>;

    /// The operator unlocked the credential source and asked to try again.
    /// `op` is the session the console holds; a backend that unlocks
    /// elsewhere leaves it alone.
    fn retry_credentials(&mut self, op: &mut dyn OnePassword);

    /// The operator chose to type the key instead of resolving it. The
    /// credentials line the backend wants shown for it, if it wants one.
    fn use_plain_text(&mut self) -> Option<TypedKey>;

    /// The operator ended the launch. The record the backend kept of the
    /// launch, if it kept one.
    fn cancel(&mut self, world: &World) -> Option<Instance>;
}

/// The accounts a launch offers: the usable ones the workspace enables, with
/// the requested one first when it is not among them, and for a content that
/// takes only the agent's provider, only those.
pub fn launch_accounts(
    w: &World,
    target: &LaunchTarget,
    content: &LaunchContent,
) -> Vec<AccountId> {
    let workspace = target.workspace.and_then(|id| w.workspace(id));
    let mut accounts: Vec<AccountId> = workspace
        .map(|x| {
            x.effective_accounts(&w.accounts)
                .into_iter()
                .filter(|e| e.usable.is_ready())
                .map(|e| e.id)
                .collect()
        })
        .unwrap_or_default();
    if let Some(a) = &target.account
        && !accounts.contains(a)
    {
        accounts.insert(0, a.clone());
    }
    if content.agent_provider_only {
        accounts.retain(|id| {
            w.accounts
                .get(id)
                .is_some_and(|a| a.provider == target.agent.provider())
        });
    }
    accounts
}
