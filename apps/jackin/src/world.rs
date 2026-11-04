use std::collections::BTreeMap;
use std::sync::Arc;

use crate::arbiter::{Arbiter, DiscoveryError};
use crate::backend::{Backend, Effect, OnePassword, Reply};
use crate::domain::account::{AccountId, AccountRegistry};
use crate::domain::agent::{Agent, AuthMode, Provider};
use crate::domain::doctor::{DoctorReport, DoctorState};
use crate::domain::instance::{DaemonSnapshot, Instance, InstanceId, InstanceStatus, RepoRef};
use crate::domain::role::{
    AccountOrder, LaunchRole, LaunchRoleRule, NO_DEFAULT_ROLE, NO_SAVED_WORKSPACE, RoleFinding,
    RoleRecord,
};
use crate::domain::surfaces::Surfaces;
use crate::domain::workspace::{
    EnvVar, Mount, RoleEntry, RoleName, Usability, Workspace, WorkspaceId,
};
use crate::panes::Daemon;
use junie_tui::clock::SimClock;
use junie_tui::shell::JobKey;

pub mod job {
    use junie_tui::shell::JobKey;
    pub const WORKSPACE_SAVE: JobKey = JobKey::new("workspace.save");
    pub const SETTINGS_SAVE: JobKey = JobKey::new("settings.save");
    pub const ROLE_LOAD: JobKey = JobKey::new("role.load");
    pub const PREWARM: JobKey = JobKey::new("prewarm");
    pub const STOP: JobKey = JobKey::new("instance.stop");
    pub const PURGE: JobKey = JobKey::new("instance.purge");
    pub const ACCOUNT_VALIDATE: JobKey = JobKey::new("account.validate");
    pub const ROLE_CHECK: JobKey = JobKey::new("role.check");
    pub const ROLE_SAVE: JobKey = JobKey::new("role.save");
    pub const ROLE_BIND: JobKey = JobKey::new("role.bind");
    pub const ROLE_ADD: JobKey = JobKey::new("role.add");
    pub const DOCTOR: JobKey = JobKey::new("doctor");
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct GlobalConfig {
    pub coauthor_trailer: bool,
    pub dco_signoff: bool,
    pub mounts: Vec<Mount>,
    pub env: Vec<EnvVar>,
    pub role_env: BTreeMap<RoleName, Vec<EnvVar>>,

    pub agent_modes: BTreeMap<Agent, AuthMode>,
    pub trust: Vec<TrustRow>,
}

impl GlobalConfig {
    pub fn change_count(&self, other: &GlobalConfig) -> usize {
        let mut n = 0;
        n += usize::from(self.coauthor_trailer != other.coauthor_trailer);
        n += usize::from(self.dco_signoff != other.dco_signoff);
        n += keyed_diff(&self.mounts, &other.mounts, |m| m.destination.clone());
        n += keyed_diff(&self.env, &other.env, |e| e.key.clone());
        n += usize::from(self.role_env != other.role_env);
        n += self
            .agent_modes
            .iter()
            .filter(|(a, m)| other.agent_modes.get(a) != Some(m))
            .count();
        n += keyed_diff(&self.trust, &other.trust, |t| t.source.clone());
        n
    }
}

pub fn keyed_diff<T: PartialEq>(a: &[T], b: &[T], key: impl Fn(&T) -> String) -> usize {
    let mut n = 0;
    for x in a {
        match b.iter().find(|y| key(y) == key(x)) {
            None => n += 1,
            Some(y) if y != x => n += 1,
            _ => {}
        }
    }
    n += b
        .iter()
        .filter(|y| !a.iter().any(|x| key(x) == key(y)))
        .count();
    n
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrustRow {
    pub source: String,
    pub kind: &'static str,
    pub trusted: bool,
    pub roles: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FsEntry {
    pub path: String,
    pub dir: bool,
    pub git: Option<String>,
    pub meta: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GithubRepo {
    pub full_name: String,
    pub default_branch: String,
    pub branches: Vec<String>,
    pub updated: String,
    pub url: String,
}

impl From<&GithubRepo> for RepoRef {
    fn from(r: &GithubRepo) -> Self {
        Self {
            full_name: r.full_name.clone(),
            url: r.url.clone(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Msg {
    WorkspaceSaved {
        id: Option<WorkspaceId>,
        result: Result<Box<Snapshot>, String>,
    },
    GlobalSaved {
        result: Result<Box<Snapshot>, String>,
    },
    AccountsSaved {
        result: Result<Box<Snapshot>, String>,
    },
    WorkspaceDeleted {
        id: WorkspaceId,
        result: Result<Box<Snapshot>, String>,
    },
    Restored {
        instance: InstanceId,
        result: Result<Box<Snapshot>, String>,
    },

    Refreshed {
        ok: bool,
    },
    Prewarmed {
        workspace: WorkspaceId,
    },
    Stopped {
        instance: InstanceId,
    },
    Purged {
        instance: InstanceId,
    },
    AccountRefreshed {
        account: AccountId,
    },
    AccountValidated {
        account: AccountId,
    },
    RoleLoaded {
        role: String,
        ok: bool,
        error: Option<String>,
    },

    Takeover {
        instance: InstanceId,
        by: String,
    },
    RoleChecked {
        role: RoleName,
        findings: Vec<RoleFinding>,
    },
    RoleSaved {
        role: RoleName,
        ok: bool,
        error: Option<String>,
    },
    RoleBound {
        role: RoleName,
        agent: Agent,
        ok: bool,
        error: Option<String>,
    },
    RoleAdded {
        location: String,
        result: Result<Box<RoleRecord>, String>,
    },
    DoctorFinished(DoctorReport),
}

impl Msg {
    pub fn job(&self) -> Option<JobKey> {
        match self {
            Msg::WorkspaceSaved { .. } => Some(job::WORKSPACE_SAVE),
            Msg::GlobalSaved { .. } => Some(job::SETTINGS_SAVE),
            Msg::RoleLoaded { .. } => Some(job::ROLE_LOAD),
            Msg::Prewarmed { .. } => Some(job::PREWARM),
            Msg::Stopped { .. } => Some(job::STOP),
            Msg::Purged { .. } => Some(job::PURGE),
            Msg::AccountValidated { .. } => Some(job::ACCOUNT_VALIDATE),
            Msg::RoleChecked { .. } => Some(job::ROLE_CHECK),
            Msg::RoleSaved { .. } => Some(job::ROLE_SAVE),
            Msg::RoleBound { .. } => Some(job::ROLE_BIND),
            Msg::RoleAdded { .. } => Some(job::ROLE_ADD),
            Msg::DoctorFinished(_) => Some(job::DOCTOR),
            Msg::AccountsSaved { .. }
            | Msg::WorkspaceDeleted { .. }
            | Msg::Restored { .. }
            | Msg::Refreshed { .. }
            | Msg::AccountRefreshed { .. }
            | Msg::Takeover { .. } => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Job {
    pub due_ms: i64,
    pub work: Work,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Work {
    Msg(Msg),
    Settle(Effect),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DaemonHealth {
    Healthy,
    Stale,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Health {
    Healthy,
    Stale,
    Failing,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HealthView {
    pub noun: String,
    pub health: Health,
    pub at_secs: i64,
}

pub struct World {
    pub clock: SimClock,
    pub time: TimeSource,
    pub arbiter: Arbiter,
    pub home: String,
    pub cwd: String,
    pub workspaces: Vec<Workspace>,
    pub roles: Vec<RoleEntry>,
    pub instances: Vec<Instance>,
    pub daemons: BTreeMap<InstanceId, Daemon>,
    pub global: GlobalConfig,
    pub accounts: AccountRegistry,
    pub op: Box<dyn OnePassword>,
    pub backend: Arc<dyn Backend>,
    pub fs: Vec<FsEntry>,
    pub github: Vec<GithubRepo>,
    pub clipboard: Option<String>,
    pub jobs: Vec<Job>,
    pub daemon_health: DaemonHealth,
    pub last_refresh_secs: i64,

    pub refresh_fails: bool,

    pub takeover_at_ms: Option<Takeover>,

    pub session_accounts: BTreeMap<(InstanceId, u64), AccountId>,

    pub discovery_pending: bool,

    pub surfaces: Surfaces,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Takeover {
    pub at_ms: i64,
    pub by: String,
}

pub trait WallSource: std::fmt::Debug + Send + Sync {
    fn epoch_millis(&self) -> i64;

    fn utc_offset_secs(&self) -> i64;
}

#[derive(Debug, Clone, Default)]
pub enum TimeSource {
    #[default]
    Sim,
    Wall(Arc<dyn WallSource>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Snapshot {
    pub workspaces: Vec<Workspace>,
    pub roles: Vec<RoleEntry>,
    pub instances: Vec<Instance>,
    pub global: GlobalConfig,
    pub accounts: AccountRegistry,
    pub surfaces: Surfaces,
    pub github: Vec<GithubRepo>,
    pub index_error: Option<DiscoveryError>,
}

pub struct WorldParts {
    pub clock: SimClock,
    pub time: TimeSource,
    pub arbiter: Arbiter,
    pub home: String,
    pub cwd: String,
    pub workspaces: Vec<Workspace>,
    pub roles: Vec<RoleEntry>,
    pub instances: Vec<Instance>,
    pub global: GlobalConfig,
    pub accounts: AccountRegistry,
    pub op: Box<dyn OnePassword>,
    pub backend: Arc<dyn Backend>,
    pub fs: Vec<FsEntry>,
    pub github: Vec<GithubRepo>,
    pub last_refresh_secs: i64,
    pub surfaces: Surfaces,
}

impl WorldParts {
    pub fn new(
        clock: SimClock,
        home: String,
        cwd: String,
        op: Box<dyn OnePassword>,
        backend: Arc<dyn Backend>,
        surfaces: Surfaces,
    ) -> Self {
        Self {
            last_refresh_secs: clock.now_secs(),
            clock,
            time: TimeSource::Sim,
            arbiter: Arbiter::new(0),
            home,
            cwd,
            workspaces: vec![],
            roles: vec![],
            instances: vec![],
            global: GlobalConfig::default(),
            accounts: AccountRegistry::default(),
            op,
            backend,
            fs: vec![],
            github: vec![],
            surfaces,
        }
    }

    pub fn wall(mut self, source: Arc<dyn WallSource>) -> Self {
        let ms = source.epoch_millis();
        self.clock.epoch_secs = ms.div_euclid(1000);
        self.clock.now_ms = ms.rem_euclid(1000);
        self.clock.utc_offset_secs = source.utc_offset_secs();
        self.last_refresh_secs = self.clock.now_secs();
        self.time = TimeSource::Wall(source);
        self
    }
}

impl World {
    pub fn new(parts: WorldParts) -> Self {
        let WorldParts {
            clock,
            time,
            arbiter,
            home,
            cwd,
            workspaces,
            roles,
            instances,
            global,
            accounts,
            op,
            backend,
            fs,
            github,
            last_refresh_secs,
            surfaces,
        } = parts;
        let mut world = Self {
            clock,
            time,
            arbiter,
            home,
            cwd,
            workspaces,
            roles,
            instances,
            daemons: BTreeMap::new(),
            global,
            accounts,
            op,
            backend,
            fs,
            github,
            clipboard: None,
            jobs: vec![],
            daemon_health: DaemonHealth::Healthy,
            last_refresh_secs,
            refresh_fails: false,
            takeover_at_ms: None,
            session_accounts: BTreeMap::new(),
            discovery_pending: false,
            surfaces,
        };
        world.sync_arbiter();
        world
    }

    pub fn now_secs(&self) -> i64 {
        self.clock.now_secs()
    }

    pub fn now_ms(&self) -> i64 {
        self.clock.now_ms
    }

    pub fn perform(&mut self, effect: Effect) {
        let backend = self.backend.clone();
        if let Some(done) = backend.perform(&effect, self) {
            match done.reply {
                Reply::Ready(msg) => self.schedule(done.after_ms, msg),
                Reply::Settle => self.enqueue(done.after_ms, Work::Settle(effect)),
            }
        }
    }

    pub fn schedule(&mut self, delay_ms: i64, msg: Msg) {
        self.enqueue(delay_ms, Work::Msg(msg));
    }

    fn enqueue(&mut self, delay_ms: i64, work: Work) {
        let due = self.clock.now_ms + delay_ms;
        self.jobs.push(Job { due_ms: due, work });
        self.jobs.sort_by_key(|j| j.due_ms);
    }

    /// The replies that are due at the current time, without advancing the
    /// clock. A reply with no delay is delivered this way, so it does not wait
    /// for a tick (and arrives under a paused clock too).
    pub fn take_due(&mut self) -> Vec<Msg> {
        let now = self.clock.now_ms;
        let mut due = vec![];
        self.jobs.retain(|j| {
            if j.due_ms <= now {
                due.push(j.work.clone());
                false
            } else {
                true
            }
        });
        let backend = self.backend.clone();
        due.into_iter()
            .filter_map(|work| match work {
                Work::Msg(msg) => Some(msg),
                Work::Settle(effect) => backend.settle(&effect, self),
            })
            .collect()
    }

    pub fn health_view(&self) -> HealthView {
        match self
            .surfaces
            .doctor
            .as_ref()
            .and_then(|d| d.report().map(|r| (d, r)))
        {
            Some((doctor, report)) => HealthView {
                noun: doctor.noun.clone(),
                health: if report.failing() > 0 {
                    Health::Failing
                } else if report.stale(self.now_secs()) {
                    Health::Stale
                } else {
                    Health::Healthy
                },
                at_secs: report.at_secs,
            },
            None => HealthView {
                noun: self.surfaces.doctor.as_ref().map_or_else(
                    || self.surfaces.vocabulary.health_noun.clone(),
                    |d| d.noun.clone(),
                ),
                health: match self.daemon_health {
                    DaemonHealth::Healthy => Health::Healthy,
                    DaemonHealth::Stale => Health::Stale,
                },
                at_secs: self.last_refresh_secs,
            },
        }
    }

    pub fn about_props(&self) -> Vec<junie_tui::widgets::props::Prop> {
        use junie_tui::widgets::props::Prop;
        self.surfaces
            .about
            .iter()
            .map(|(label, value)| Prop::new(label, value.as_str()))
            .collect()
    }

    pub fn docker_on(&self, ws: Option<WorkspaceId>, role: &str) -> bool {
        self.surfaces.docker.as_ref().is_some_and(|d| {
            d.role_enabled.contains(role)
                || ws
                    .and_then(|id| self.workspace(id))
                    .is_some_and(|w| w.docker)
        })
    }

    pub fn apply(&mut self, snapshot: Snapshot) {
        let Snapshot {
            workspaces,
            roles,
            instances,
            global,
            accounts,
            surfaces,
            github,
            index_error,
        } = snapshot;
        self.workspaces = workspaces;
        self.roles = roles;
        self.instances = instances;
        self.global = global;
        self.accounts = accounts;
        self.surfaces = surfaces;
        self.github = github;
        refresh_snapshots(self);
        self.arbiter.discovery = match index_error {
            Some(error) => Err(error),
            None => Ok(self.running_count()),
        };
    }

    pub fn absorb(&mut self, msg: &Msg) {
        if let (Msg::DoctorFinished(report), Some(d)) = (msg, self.surfaces.doctor.as_mut()) {
            d.state = DoctorState::Done(report.clone());
        }
    }

    pub fn tick(&mut self, interval_ms: i64) -> Vec<Msg> {
        match &self.time {
            TimeSource::Sim => self.clock.advance(interval_ms),
            TimeSource::Wall(source) if self.clock.running => {
                self.clock.now_ms = source.epoch_millis() - self.clock.epoch_secs * 1000;
                self.clock.utc_offset_secs = source.utc_offset_secs();
            }
            TimeSource::Wall(_) => {}
        }
        if !self.clock.running {
            return vec![];
        }
        let now = self.clock.now_ms;
        let mut due = self.take_due();
        for (id, d) in self.daemons.iter_mut() {
            if self
                .instances
                .iter()
                .any(|i| i.id == *id && i.status == InstanceStatus::Running)
            {
                d.tick(now);
            }
        }
        if self.takeover_at_ms.as_ref().is_some_and(|t| now >= t.at_ms)
            && let Some(Takeover { by, .. }) = self.takeover_at_ms.take()
            && let Some(i) = self
                .instances
                .iter()
                .find(|i| i.status == InstanceStatus::Running)
        {
            due.push(Msg::Takeover {
                instance: i.id.clone(),
                by,
            });
        }
        due
    }

    pub fn workspace(&self, id: WorkspaceId) -> Option<&Workspace> {
        self.workspaces.iter().find(|w| w.id == id)
    }

    pub fn workspace_mut(&mut self, id: WorkspaceId) -> Option<&mut Workspace> {
        self.workspaces.iter_mut().find(|w| w.id == id)
    }

    pub fn cwd_workspace(&self) -> Option<&Workspace> {
        self.workspaces.iter().find(|w| {
            w.mounts.iter().any(|m| {
                m.source_label() == self.cwd || expand(&self.home, m.source_label()) == self.cwd
            })
        })
    }

    pub fn workspace_label(&self, i: &Instance) -> String {
        i.workspace
            .and_then(|id| self.workspace(id))
            .map(|ws| ws.name.clone())
            .unwrap_or_else(|| {
                self.cwd
                    .rsplit('/')
                    .next()
                    .unwrap_or("directory")
                    .to_owned()
            })
    }

    pub fn workspace_repo(&self, id: WorkspaceId) -> Option<&RepoRef> {
        self.instances
            .iter()
            .filter(|i| i.workspace == Some(id))
            .find_map(|i| i.github.as_ref())
    }

    pub fn instance(&self, id: &str) -> Option<&Instance> {
        self.instances.iter().find(|i| i.id == id)
    }

    pub fn instance_mut(&mut self, id: &str) -> Option<&mut Instance> {
        self.instances.iter_mut().find(|i| i.id == id)
    }

    pub fn running_count(&self) -> usize {
        self.instances
            .iter()
            .filter(|i| i.status == InstanceStatus::Running)
            .count()
    }

    pub fn running(&self) -> Vec<&Instance> {
        self.instances
            .iter()
            .filter(|i| i.status == InstanceStatus::Running)
            .collect()
    }

    pub fn instances_of(&self, ws: Option<WorkspaceId>) -> Vec<&Instance> {
        self.instances
            .iter()
            .filter(|i| i.workspace == ws && !i.status.hidden())
            .collect()
    }

    pub fn sync_arbiter(&mut self) {
        if self.arbiter.discovery.is_ok() {
            let n = self.running_count();
            self.arbiter.set_running(n);
        }
    }

    pub fn mask_path(&self, path: &str) -> String {
        mask_path(&self.home, path)
    }

    pub fn tilde(&self, path: &str) -> String {
        if let Some(rest) = path.strip_prefix(&self.home) {
            format!("~{rest}")
        } else {
            path.to_owned()
        }
    }

    pub fn config_file(&self) -> String {
        self.tilde(self.surfaces.config_path.as_deref().unwrap_or_default())
    }

    pub fn new_instance_id(&self) -> String {
        let n = self.instances.len() as u32 + 0x7f3a + 0x1234 * 3;
        format!("jk-{:04x}", n & 0xffff)
    }

    pub fn account_for(
        &self,
        provider: Provider,
        ws: Option<&Workspace>,
        role: Option<&str>,
        session: Option<&AccountId>,
    ) -> crate::domain::precedence::ResolvedAccount {
        let (order, binding) = match &self.surfaces.roles {
            Some(admin) => (
                admin.order,
                role.and_then(|r| admin.binding_for_provider(r, provider)),
            ),
            None => (AccountOrder::Demo, None),
        };
        crate::domain::precedence::resolve_account(
            provider,
            ws,
            role,
            session.map(|s| (s, self.surfaces.vocabulary.session_choice.as_str())),
            &self.accounts,
            order,
            binding,
        )
    }

    pub fn agent_mode(&self, agent: Agent) -> AuthMode {
        self.global
            .agent_modes
            .get(&agent)
            .copied()
            .unwrap_or(AuthMode::Sync)
    }

    pub fn eligible_accounts(
        &self,
        agent: Agent,
        ws: Option<&Workspace>,
        role: Option<&str>,
    ) -> Vec<AccountId> {
        let offer = self.offer_for(agent, ws, role);
        offer.accounts
    }

    pub fn offer_for(
        &self,
        agent: Agent,
        ws: Option<&Workspace>,
        role: Option<&str>,
    ) -> AgentOffer {
        let provider = agent.provider();
        let mut ready: Vec<AccountId> = vec![];
        let mut blocked: Vec<(AccountId, String)> = vec![];
        match ws {
            Some(w) => {
                for e in w.effective_accounts(&self.accounts) {
                    if e.provider != provider {
                        continue;
                    }
                    match &e.usable {
                        Usability::Ready => ready.push(e.id.clone()),
                        u => blocked.push((e.id.clone(), u.label(self.surfaces.fixed.as_ref()))),
                    }
                }
            }
            None => {
                if let Some(a) = self.accounts.default_for(provider) {
                    match crate::domain::workspace::usability_of(a) {
                        Usability::Ready => ready.push(a.id.clone()),
                        u => blocked.push((a.id.clone(), u.label(self.surfaces.fixed.as_ref()))),
                    }
                }
                if ready.is_empty()
                    && let Some(a) = self.accounts.discovered_current(provider)
                    && !ready.contains(&a.id)
                {
                    ready.push(a.id.clone());
                }
            }
        }
        if let Some(limit) = &self.surfaces.agent_blocked
            && agent != limit.launchable
        {
            return AgentOffer {
                accounts: vec![],
                preselected: None,
                blocked: Some(limit.text.clone()),
                configured: !ready.is_empty() || !blocked.is_empty(),
            };
        }
        if self.agent_mode(agent) == AuthMode::Ignore {
            return AgentOffer {
                accounts: vec![],
                preselected: None,
                blocked: Some("agent runtime mode is ignore · set in Settings › Agents".into()),
                configured: !ready.is_empty() || !blocked.is_empty(),
            };
        }

        let preselected = self
            .account_for(provider, ws, role, None)
            .account
            .filter(|id| ready.contains(id))
            .or_else(|| ready.first().cloned());
        if let Some(p) = &preselected
            && let Some(i) = ready.iter().position(|x| x == p)
            && i > 0
        {
            let x = ready.remove(i);
            ready.insert(0, x);
        }
        let block = if ready.is_empty() {
            blocked.first().map(|(id, why)| {
                let launch_block = self.accounts.get(id).and_then(|a| a.launch_blocked);
                if launch_block.is_some_and(|b| b.text(self.surfaces.fixed.as_ref()) == *why) {
                    return why.clone();
                }
                let name = self
                    .accounts
                    .get(id)
                    .map(|a| a.title())
                    .unwrap_or(id.clone());
                format!("{name} · {why}")
            })
        } else {
            None
        };
        AgentOffer {
            configured: !ready.is_empty() || !blocked.is_empty(),
            accounts: ready,
            preselected,
            blocked: block,
        }
    }

    pub fn launch_role(&self, from: LaunchSource) -> LaunchRole {
        match self.surfaces.launch_role {
            LaunchRoleRule::Demo => LaunchRole::Role(match from {
                LaunchSource::Row(ws) => ws
                    .and_then(|x| x.roles.default.clone().or(x.roles.last.clone()))
                    .unwrap_or_else(|| "the-architect".into()),
                LaunchSource::CurrentDir(_) => "the-architect".into(),
            }),
            LaunchRoleRule::Configured => {
                let (LaunchSource::Row(ws) | LaunchSource::CurrentDir(ws)) = from;
                if let Some(default) = ws.and_then(|x| x.roles.default.clone()) {
                    return LaunchRole::Role(default);
                }
                let mut allowed = self
                    .roles
                    .iter()
                    .filter(|r| ws.is_none_or(|x| x.roles.allows(&r.name)))
                    .map(|r| r.name.clone());
                match (allowed.next(), allowed.next()) {
                    (Some(only), None) => LaunchRole::Role(only),
                    _ if ws.is_some() => LaunchRole::Unavailable(NO_DEFAULT_ROLE.into()),
                    _ => LaunchRole::Unavailable(NO_SAVED_WORKSPACE.into()),
                }
            }
        }
    }

    pub fn offered_agents(
        &self,
        ws: Option<&Workspace>,
        role: Option<&str>,
    ) -> Vec<(Agent, AgentOffer)> {
        Agent::ALL
            .iter()
            .map(|a| (*a, self.offer_for(*a, ws, role)))
            .filter(|(_, o)| o.configured)
            .collect()
    }
}

#[derive(Debug, Clone, Copy)]
pub enum LaunchSource<'a> {
    Row(Option<&'a Workspace>),
    CurrentDir(Option<&'a Workspace>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentOffer {
    pub accounts: Vec<AccountId>,
    pub preselected: Option<AccountId>,

    pub blocked: Option<String>,

    pub configured: bool,
}

pub fn expand(home: &str, path: &str) -> String {
    if let Some(rest) = path.strip_prefix('~') {
        format!("{home}{rest}")
    } else {
        path.to_owned()
    }
}

pub fn mask_path(home: &str, path: &str) -> String {
    let last = path.rsplit('/').next().unwrap_or(path);
    if let Some(rest) = path.strip_prefix(home) {
        let segs: Vec<&str> = rest.split('/').filter(|s| !s.is_empty()).collect();
        match segs.len() {
            0 => "~".into(),
            1 => format!("~/{}", segs[0]),
            _ => format!("~/…/{last}"),
        }
    } else if path.starts_with('/') {
        format!("…/{last}")
    } else {
        path.to_owned()
    }
}

pub fn refresh_snapshots(w: &mut World) {
    let snaps: Vec<(String, DaemonSnapshot)> = w
        .daemons
        .iter()
        .map(|(id, d)| (id.clone(), d.snapshot()))
        .collect();
    for (id, s) in snaps {
        if let Some(i) = w.instance_mut(&id) {
            i.daemon = s;
        }
    }
    for i in w.instances.iter_mut() {
        if i.status != InstanceStatus::Running {
            i.daemon = DaemonSnapshot::Unavailable;
        }
    }
}
