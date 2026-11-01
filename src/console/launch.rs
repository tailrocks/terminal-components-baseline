use std::sync::Arc;

use crate::widgets::steps::StepState;

use crate::console::domain::agent::Agent;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Stage {
    Identity,
    Role,
    Credentials,
    Construct,
    AgentBinaries,
    DerivedImage,
    Workspace,
    Network,
    Sidecar,
    Capsule,
    Hardline,
}

impl Stage {
    pub const ALL: [Stage; 11] = [
        Stage::Identity,
        Stage::Role,
        Stage::Credentials,
        Stage::Construct,
        Stage::AgentBinaries,
        Stage::DerivedImage,
        Stage::Workspace,
        Stage::Network,
        Stage::Sidecar,
        Stage::Capsule,
        Stage::Hardline,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Stage::Identity => "Identity",
            Stage::Role => "Role",
            Stage::Credentials => "Credentials",
            Stage::Construct => "Construct",
            Stage::AgentBinaries => "Agent Binaries",
            Stage::DerivedImage => "Derived Image",
            Stage::Workspace => "Workspace",
            Stage::Network => "Network",
            Stage::Sidecar => "Sidecar",
            Stage::Capsule => "Capsule",
            Stage::Hardline => "Hardline",
        }
    }

    pub fn index(self) -> usize {
        Stage::ALL.iter().position(|s| *s == self).unwrap_or(0)
    }

    pub fn activity(self, agent: Agent) -> String {
        match self {
            Stage::Identity => "Resolving launch identity…".into(),
            Stage::Role => "Loading role manifest…".into(),
            Stage::Credentials => "Resolving credentials…".into(),
            Stage::Construct => "Preparing the Construct image…".into(),
            Stage::AgentBinaries => format!("Installing {} binaries…", agent.label()),
            Stage::DerivedImage => "Building derived image…".into(),
            Stage::Workspace => "Mounting the Workspace…".into(),
            Stage::Network => "Attaching the network…".into(),
            Stage::Sidecar => "Starting the sidecar…".into(),
            Stage::Capsule => "Waiting for the Capsule daemon…".into(),
            Stage::Hardline => "Opening the hardline…".into(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LaunchPlan {
    Clean,

    FailNetwork,

    CredentialsLocked,

    BlockedSidecar,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LaunchFailure {
    pub stage: Stage,
    pub summary: String,
    pub next_step: String,
    pub detail: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LaunchEvent {
    StageChanged(Stage, StepState),
    Activity(String),
    BuildLine(String),
    ContainerReady(String),
    CredentialsResolved { origin: String, validation: String },

    CredentialError { message: String },
    Failed(LaunchFailure),
    Ready,
}

#[derive(Debug, Clone)]
pub struct LaunchRun {
    pub plan: LaunchPlan,
    pub run_id: String,
    pub container: String,
    pub agent: Agent,
    pub states: [StepState; 11],
    pub durations: [u64; 11],
    pub tick: u64,
    pub stage_start: u64,
    pub current: Option<usize>,
    pub done: bool,
    pub failure: Option<LaunchFailure>,
    pub blocked_at: Option<Stage>,

    pub credential_hold: bool,
    pub credential_retried: bool,
    pub build_lines_emitted: usize,
    pub cancelled: bool,
    pub content: Arc<LaunchContent>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LaunchContent {
    pub build_log: Vec<String>,
    pub network_failure: LaunchFailure,
}

impl LaunchRun {
    pub fn new(
        plan: LaunchPlan,
        agent: Agent,
        container: &str,
        run_id: &str,
        content: Arc<LaunchContent>,
    ) -> Self {
        let durations = [14, 18, 26, 30, 8, 92, 22, 20, 18, 34, 16];
        Self {
            plan,
            run_id: run_id.to_owned(),
            container: container.to_owned(),
            agent,
            states: [StepState::Queued; 11],
            durations,
            tick: 0,
            stage_start: 0,
            current: None,
            done: false,
            failure: None,
            blocked_at: None,
            credential_hold: false,
            credential_retried: false,
            build_lines_emitted: 0,
            cancelled: false,
            content,
        }
    }

    pub fn counts(&self) -> (usize, usize) {
        (
            self.states
                .iter()
                .filter(|s| **s == StepState::Done)
                .count(),
            self.states
                .iter()
                .filter(|s| **s == StepState::Skipped)
                .count(),
        )
    }

    pub fn is_terminal(&self) -> bool {
        self.done || self.failure.is_some() || self.blocked_at.is_some() || self.cancelled
    }

    pub fn retry_credentials(&mut self) {
        self.credential_hold = false;
        self.credential_retried = true;
        self.stage_start = self.tick;
    }

    pub fn cancel(&mut self) {
        self.cancelled = true;
        if let Some(i) = self.current {
            self.states[i] = StepState::Failed;
        }
    }

    pub fn advance(&mut self) -> Vec<LaunchEvent> {
        let mut ev = vec![];
        if self.is_terminal() || self.credential_hold {
            return ev;
        }
        self.tick += 1;
        let Some(i) = self.current else {
            self.current = Some(0);
            self.stage_start = self.tick;
            self.states[0] = StepState::Running;
            ev.push(LaunchEvent::StageChanged(
                Stage::Identity,
                StepState::Running,
            ));
            ev.push(LaunchEvent::Activity(Stage::Identity.activity(self.agent)));
            return ev;
        };
        let stage = Stage::ALL[i];
        let elapsed = self.tick - self.stage_start;

        match stage {
            Stage::DerivedImage => {
                let log = &self.content.build_log;
                let want =
                    ((elapsed as usize) * log.len() / self.durations[i] as usize).min(log.len());
                while self.build_lines_emitted < want {
                    ev.push(LaunchEvent::BuildLine(
                        self.content.build_log[self.build_lines_emitted].clone(),
                    ));
                    self.build_lines_emitted += 1;
                }
            }
            Stage::Construct if elapsed == 6 => {
                ev.push(LaunchEvent::ContainerReady(self.container.clone()));
            }
            Stage::Credentials
                if elapsed == 10
                    && self.plan == LaunchPlan::CredentialsLocked
                    && !self.credential_retried =>
            {
                self.credential_hold = true;
                ev.push(LaunchEvent::CredentialError {
                    message: "1Password locked: unlock the app and retry".into(),
                });
                return ev;
            }
            _ => {}
        }
        if elapsed < self.durations[i] {
            return ev;
        }

        let final_state = match (self.plan, stage) {
            (_, Stage::AgentBinaries) => StepState::Skipped,
            (LaunchPlan::FailNetwork, Stage::Network) => StepState::Failed,
            (LaunchPlan::BlockedSidecar, Stage::Sidecar) => StepState::Blocked,
            _ => StepState::Done,
        };
        self.states[i] = final_state;
        ev.push(LaunchEvent::StageChanged(stage, final_state));
        if stage == Stage::Credentials {
            ev.push(LaunchEvent::CredentialsResolved {
                origin: String::new(),
                validation: String::new(),
            });
        }
        match final_state {
            StepState::Failed => {
                let f = self.content.network_failure.clone();
                self.failure = Some(f.clone());
                ev.push(LaunchEvent::Failed(f));
                return ev;
            }
            StepState::Blocked => {
                self.blocked_at = Some(stage);
                ev.push(LaunchEvent::Activity(
                    "Blocked · this stage cannot continue".into(),
                ));
                return ev;
            }
            _ => {}
        }
        if i + 1 < Stage::ALL.len() {
            let next = Stage::ALL[i + 1];
            self.current = Some(i + 1);
            self.stage_start = self.tick;
            self.states[i + 1] = StepState::Running;
            ev.push(LaunchEvent::StageChanged(next, StepState::Running));
            ev.push(LaunchEvent::Activity(next.activity(self.agent)));
        } else {
            self.done = true;
            ev.push(LaunchEvent::Ready);
        }
        ev
    }
}
