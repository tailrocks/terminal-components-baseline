use crate::domain::agent::Agent;

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
    pub credentials_resolved: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LaunchFailure {
    pub stage: usize,
    pub summary: String,
    pub next_step: String,
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
            credentials_resolved: "resolved in memory · discarded".into(),
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
                next_step: "Run jackin doctor, fix the failing check, then launch again.".into(),
                detail: Vec::new(),
            },
            agent_provider_only: true,
            name_slug: Some(7),
        }
    }
}
