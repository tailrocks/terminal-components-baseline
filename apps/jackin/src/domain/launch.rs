use crate::domain::agent::Agent;
use crate::domain::vocabulary::fill_in;

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
    /// Takes `{status}`.
    pub ready_not_running: String,
    pub credentials_resolved: String,
    /// The `State` row of a refused launch.
    pub refused_state: String,
    /// Templates take `{workspace}` and `{role}`.
    pub role_not_allowed: RefusalWords,
    /// Templates take `{agent}` and `{launchable}`.
    pub agent_not_launchable: RefusalWords,
    /// Takes `{sandbox}` and `{launch}`.
    pub next_retry: String,
    pub next_doctor: String,
    pub next_credential: String,
}

/// What the cockpit says about one kind of refused launch.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RefusalWords {
    pub title: String,
    pub found: String,
    pub next_step: String,
}

/// Why a launch never started. The backend names the case and its facts, and
/// `LaunchWords::refusal` words it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RefusalReason {
    RoleNotAllowed {
        workspace: String,
        role: String,
    },
    /// The agent is not one the backend launches. `launchable` is the one it
    /// does.
    AgentNotLaunchable {
        agent: Agent,
        launchable: Agent,
    },
}

/// The texts of a refused launch, as the cockpit draws them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RefusalText {
    pub title: String,
    pub found: String,
    pub state: String,
    pub next_step: String,
}

/// What is left to do after a launch failed, as a fact.
/// `LaunchWords::next_step` words it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NextStep {
    /// Nothing is left to do, and the failure shows no `Next step` row.
    None,
    /// The sandbox of the launch remains; launching again with the same
    /// launch id picks it up.
    Retry {
        sandbox: String,
        launch: String,
    },
    RunDoctor,
    FixCredential,
}

/// Where the console goes when a launch ends without an instance and nothing
/// is running.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EmptyLanding {
    /// The outro: nothing runs, so the operator leaves.
    Outro,
    /// The manager, which lists what the launch left.
    Manager,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LaunchFailure {
    pub stage: usize,
    pub summary: String,
    pub next_step: NextStep,
    pub detail: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LaunchContent {
    pub stages: StageList,
    pub words: LaunchWords,
    pub build_log: Vec<String>,
    pub network_failure: LaunchFailure,
    pub agent_provider_only: bool,
    pub name_slug: Option<usize>,
    pub when_empty: EmptyLanding,
}

impl StageList {
    pub fn openshell() -> Self {
        Self::new(
            StageSpec::new("image", "Image build", "Building the role image…", 30)
                .kind(StageKind::BuildLog),
            [
                StageSpec::new(
                    "provider",
                    "OpenShell provider",
                    "Preparing the OpenShell provider…",
                    10,
                ),
                StageSpec::new("sandbox", "Sandbox", "Creating the sandbox…", 22).container_at(6),
                StageSpec::new("credentials", "Credentials", "Resolving credentials…", 14)
                    .kind(StageKind::Credentials { hold_at: 10 }),
                StageSpec::new("upload", "Upload", "Uploading the project…", 30),
                StageSpec::new(
                    "hooks",
                    "Hooks",
                    "Running the setup and preflight hooks…",
                    18,
                ),
                StageSpec::new("codex", "Codex", "Starting {agent}…", 16),
            ],
        )
    }
}

impl LaunchWords {
    pub fn openshell() -> Self {
        Self {
            unit: "Sandbox".into(),
            unit_missing: "not created".into(),
            log_title: "Image build".into(),
            log_waiting: "(waiting for build output…)".into(),
            build_failed: "Image build failed".into(),
            ready_activity: "Codex is running · opening the session".into(),
            ready_not_running: "The sandbox is {status}, not running".into(),
            credentials_resolved: "resolved in memory · discarded".into(),
            refused_state: "nothing was created".into(),
            role_not_allowed: RefusalWords {
                title: "Role not allowed".into(),
                found: "Workspace {workspace} does not allow role {role}.".into(),
                next_step: "Allow it in the workspace's Roles tab, or load another role.".into(),
            },
            agent_not_launchable: RefusalWords {
                title: "Agent not launchable".into(),
                found:
                    "{agent} cannot be launched: {launchable} is the only agent jackin launches."
                        .into(),
                next_step: "Launch with {launchable}; {agent} stays usage only.".into(),
            },
            next_retry: "sandbox {sandbox} remains; retry with: jackin load --launch-id {launch}"
                .into(),
            next_doctor: "Run jackin doctor, fix the failing check, then launch again.".into(),
            next_credential: "Fix the account's credential in Accounts, then launch again.".into(),
        }
    }

    pub fn refusal(&self, reason: &RefusalReason) -> RefusalText {
        let (words, pairs) = match reason {
            RefusalReason::RoleNotAllowed { workspace, role } => (
                &self.role_not_allowed,
                vec![("workspace", workspace.as_str()), ("role", role.as_str())],
            ),
            RefusalReason::AgentNotLaunchable { agent, launchable } => (
                &self.agent_not_launchable,
                vec![("agent", agent.label()), ("launchable", launchable.label())],
            ),
        };
        RefusalText {
            title: fill_in(&words.title, &pairs),
            found: fill_in(&words.found, &pairs),
            state: self.refused_state.clone(),
            next_step: fill_in(&words.next_step, &pairs),
        }
    }

    pub fn next_step(&self, step: &NextStep) -> Option<String> {
        match step {
            NextStep::None => None,
            NextStep::Retry { sandbox, launch } => Some(fill_in(
                &self.next_retry,
                &[("sandbox", sandbox), ("launch", launch)],
            )),
            NextStep::RunDoctor => Some(self.next_doctor.clone()),
            NextStep::FixCredential => Some(self.next_credential.clone()),
        }
    }
}

impl LaunchContent {
    /// The OpenShell stages and words. The build log and the failure detail
    /// are empty: a backend fills them from what it observes.
    pub fn openshell() -> Self {
        let stages = StageList::openshell();
        let sandbox = stages.position("sandbox").unwrap_or(0);
        Self {
            stages,
            words: LaunchWords::openshell(),
            build_log: Vec::new(),
            network_failure: LaunchFailure {
                stage: sandbox,
                summary: "The sandbox could not be created".into(),
                next_step: NextStep::RunDoctor,
                detail: Vec::new(),
            },
            agent_provider_only: true,
            name_slug: Some(7),
            when_empty: EmptyLanding::Manager,
        }
    }
}
