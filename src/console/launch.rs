use std::sync::Arc;

use crate::widgets::steps::StepState;

use crate::console::domain::agent::Agent;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StageKind {
    Plain,
    BuildLog,
    Credentials { hold_at: u64 },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StageSpec {
    pub id: String,
    pub label: String,
    pub activity: String,
    pub ticks: u64,
    pub kind: StageKind,
    pub skipped: bool,
    pub container_at: Option<u64>,
}

impl StageSpec {
    pub fn new(id: &str, label: &str, activity: &str, ticks: u64) -> Self {
        Self {
            id: id.to_owned(),
            label: label.to_owned(),
            activity: activity.to_owned(),
            ticks,
            kind: StageKind::Plain,
            skipped: false,
            container_at: None,
        }
    }

    pub fn kind(mut self, kind: StageKind) -> Self {
        self.kind = kind;
        self
    }

    pub fn skipped(mut self) -> Self {
        self.skipped = true;
        self
    }

    pub fn container_at(mut self, tick: u64) -> Self {
        self.container_at = Some(tick);
        self
    }

    pub fn activity_for(&self, agent: Agent) -> String {
        self.activity.replace("{agent}", agent.label())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StageList(Vec<StageSpec>);

impl StageList {
    pub fn new(first: StageSpec, rest: impl IntoIterator<Item = StageSpec>) -> Self {
        let mut stages = vec![first];
        stages.extend(rest);
        Self(stages)
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn is_empty(&self) -> bool {
        false
    }

    pub fn get(&self, index: usize) -> &StageSpec {
        &self.0[index.min(self.0.len() - 1)]
    }

    pub fn iter(&self) -> std::slice::Iter<'_, StageSpec> {
        self.0.iter()
    }

    pub fn position(&self, id: &str) -> Option<usize> {
        self.0.iter().position(|s| s.id == id)
    }

    pub fn first_of(&self, pick: impl Fn(StageKind) -> bool) -> Option<usize> {
        self.0.iter().position(|s| pick(s.kind))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LaunchWords {
    pub unit: String,
    pub unit_missing: String,
    pub log_title: String,
    pub log_waiting: String,
    pub build_failed: String,
    pub ready_activity: String,
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
    pub stage: usize,
    pub summary: String,
    pub next_step: String,
    pub detail: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LaunchEvent {
    StageChanged(usize, StepState),
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
    pub states: Vec<StepState>,
    pub tick: u64,
    pub stage_start: u64,
    pub current: Option<usize>,
    pub done: bool,
    pub failure: Option<LaunchFailure>,
    pub blocked_at: Option<usize>,

    pub credential_hold: bool,
    pub credential_retried: bool,
    pub build_lines_emitted: usize,
    pub cancelled: bool,
    pub content: Arc<LaunchContent>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LaunchContent {
    pub stages: StageList,
    pub words: LaunchWords,
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
        let states = vec![StepState::Queued; content.stages.len()];
        Self {
            plan,
            run_id: run_id.to_owned(),
            container: container.to_owned(),
            agent,
            states,
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

    fn plan_stage(&self) -> Option<usize> {
        let id = match self.plan {
            LaunchPlan::FailNetwork => "network",
            LaunchPlan::BlockedSidecar => "sidecar",
            LaunchPlan::Clean | LaunchPlan::CredentialsLocked => return None,
        };
        self.content.stages.position(id)
    }

    pub fn advance(&mut self) -> Vec<LaunchEvent> {
        let mut ev = vec![];
        if self.is_terminal() || self.credential_hold {
            return ev;
        }
        self.tick += 1;
        let stages = &self.content.stages;
        let Some(i) = self.current else {
            self.current = Some(0);
            self.stage_start = self.tick;
            self.states[0] = StepState::Running;
            ev.push(LaunchEvent::StageChanged(0, StepState::Running));
            ev.push(LaunchEvent::Activity(
                stages.get(0).activity_for(self.agent),
            ));
            return ev;
        };
        let spec = stages.get(i);
        let elapsed = self.tick - self.stage_start;

        if spec.kind == StageKind::BuildLog {
            let log = &self.content.build_log;
            let want = ((elapsed as usize) * log.len() / spec.ticks.max(1) as usize).min(log.len());
            while self.build_lines_emitted < want {
                ev.push(LaunchEvent::BuildLine(
                    self.content.build_log[self.build_lines_emitted].clone(),
                ));
                self.build_lines_emitted += 1;
            }
        }
        if spec.container_at == Some(elapsed) {
            ev.push(LaunchEvent::ContainerReady(self.container.clone()));
        }
        if let StageKind::Credentials { hold_at } = spec.kind
            && elapsed == hold_at
            && self.plan == LaunchPlan::CredentialsLocked
            && !self.credential_retried
        {
            self.credential_hold = true;
            ev.push(LaunchEvent::CredentialError {
                message: "1Password locked: unlock the app and retry".into(),
            });
            return ev;
        }
        if elapsed < spec.ticks {
            return ev;
        }

        let planned = self.plan_stage() == Some(i);
        let final_state = match (self.plan, planned) {
            _ if spec.skipped => StepState::Skipped,
            (LaunchPlan::FailNetwork, true) => StepState::Failed,
            (LaunchPlan::BlockedSidecar, true) => StepState::Blocked,
            _ => StepState::Done,
        };
        self.states[i] = final_state;
        ev.push(LaunchEvent::StageChanged(i, final_state));
        if matches!(spec.kind, StageKind::Credentials { .. }) {
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
                self.blocked_at = Some(i);
                ev.push(LaunchEvent::Activity(
                    "Blocked · this stage cannot continue".into(),
                ));
                return ev;
            }
            _ => {}
        }
        if i + 1 < stages.len() {
            self.current = Some(i + 1);
            self.stage_start = self.tick;
            self.states[i + 1] = StepState::Running;
            ev.push(LaunchEvent::StageChanged(i + 1, StepState::Running));
            ev.push(LaunchEvent::Activity(
                stages.get(i + 1).activity_for(self.agent),
            ));
        } else {
            self.done = true;
            ev.push(LaunchEvent::Ready);
        }
        ev
    }
}
