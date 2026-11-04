use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicUsize, Ordering};

use crate::backend::{
    Backend, ChangeSet, Completion, Effect, FolderProbe, OnePassword, OpSession, Validation,
    ValidationOutcome,
};
use crate::domain::account::{
    Account, AccountOrigin, AccountRegistry, CredentialSource, ValidationLevel,
};
use crate::domain::agent::Provider;
use crate::domain::instance::{Instance, InstanceStatus};
use crate::domain::launch::LaunchContent;
use crate::domain::vocabulary::fill_in;
use crate::domain::workspace::{RoleSource, Workspace, WorkspaceId};
use crate::launch::{LaunchPlan, LaunchTarget};
use crate::panes::SessionScript;
use crate::world::{GlobalConfig, Msg, Snapshot, World};

pub mod changes;
pub mod launch;
pub mod onepassword;
pub mod provider;
pub mod scripts;
pub mod surfaces;

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
enum LaunchRule {
    #[default]
    Clean,
    FailNetwork,
    HardCases,
}

#[derive(Debug)]
pub struct SimBackend {
    launch: LaunchRule,
    save_fails_once: AtomicBool,
    bind_fails_once: AtomicBool,
    doctor_runs: AtomicUsize,
    openshell: bool,
    next_workspace_id: AtomicU32,
}

impl Default for SimBackend {
    fn default() -> Self {
        Self {
            launch: LaunchRule::default(),
            save_fails_once: AtomicBool::default(),
            bind_fails_once: AtomicBool::default(),
            doctor_runs: AtomicUsize::default(),
            openshell: false,
            next_workspace_id: AtomicU32::new(100),
        }
    }
}

impl SimBackend {
    pub fn shared() -> Arc<dyn Backend> {
        Arc::new(SimBackend::default())
    }

    pub fn failing_launch() -> Arc<dyn Backend> {
        Arc::new(SimBackend {
            launch: LaunchRule::FailNetwork,
            ..SimBackend::default()
        })
    }

    pub fn hard_cases() -> Arc<dyn Backend> {
        Arc::new(SimBackend {
            save_fails_once: AtomicBool::new(true),
            launch: LaunchRule::HardCases,
            ..SimBackend::default()
        })
    }

    pub fn failing_first_bind() -> Arc<dyn Backend> {
        Arc::new(SimBackend {
            bind_fails_once: AtomicBool::new(true),
            ..SimBackend::default()
        })
    }

    pub fn openshell() -> Arc<dyn Backend> {
        Arc::new(SimBackend {
            openshell: true,
            ..SimBackend::default()
        })
    }

    fn save_ok(&self) -> bool {
        !self.save_fails_once.swap(false, Ordering::SeqCst)
    }

    fn free_workspace_id(&self, taken: &[Workspace]) -> WorkspaceId {
        let above = taken.iter().map(|w| w.id + 1).max().unwrap_or(0);
        let id = self.next_workspace_id.load(Ordering::SeqCst).max(above);
        self.next_workspace_id.store(id + 1, Ordering::SeqCst);
        id
    }

    fn settle_global_save(&self, global: &GlobalConfig, world: &World) -> Msg {
        if !self.save_ok() {
            return Msg::GlobalSaved {
                result: Err(fill_in(
                    &world.surfaces.vocabulary.save_error_global,
                    &[("config", &world.config_file())],
                )),
            };
        }
        let mut snapshot = snapshot(world);
        snapshot.global = global.clone();
        Msg::GlobalSaved {
            result: Ok(Box::new(snapshot)),
        }
    }

    fn settle_workspace_save(
        &self,
        id: Option<WorkspaceId>,
        workspace: &Workspace,
        world: &World,
    ) -> Msg {
        if !self.save_ok() {
            return Msg::WorkspaceSaved {
                id,
                result: Err(fill_in(
                    &world.surfaces.vocabulary.save_error_workspace,
                    &[("config", &world.config_file())],
                )),
            };
        }
        let mut snapshot = snapshot(world);
        let mut stored = workspace.clone();
        let assigned = match id {
            Some(id) => {
                if let Some(slot) = snapshot.workspaces.iter_mut().find(|w| w.id == id) {
                    stored.id = id;
                    *slot = stored;
                }
                id
            }
            None => {
                let id = self.free_workspace_id(&snapshot.workspaces);
                stored.id = id;
                snapshot.workspaces.push(stored);
                id
            }
        };
        Msg::WorkspaceSaved {
            id: Some(assigned),
            result: Ok(Box::new(snapshot)),
        }
    }
}

impl Backend for SimBackend {
    fn validate(
        &self,
        provider: Provider,
        source: &CredentialSource,
        plain_value: Option<&str>,
        op: &dyn OnePassword,
        now: i64,
    ) -> ValidationOutcome {
        provider::validate(provider, source, plain_value, op, now)
    }

    fn probe_folder(&self, path: &str) -> FolderProbe {
        provider::probe_folder(path)
    }

    fn perform(&self, effect: &Effect, world: &World) -> Option<Completion> {
        let (after_ms, msg) = match effect {
            Effect::SaveWorkspace { .. } => return Some(Completion::settle(900)),
            Effect::SaveGlobal { .. } => return Some(Completion::settle(900)),
            Effect::SaveAccounts { .. } => return Some(Completion::settle(0)),
            Effect::TrustRole { .. } => return Some(Completion::settle(0)),
            Effect::DeleteWorkspace { .. } | Effect::Restore { .. } => {
                return Some(Completion::settle(0));
            }
            Effect::Prewarm { workspace } => (
                2_400,
                Msg::Prewarmed {
                    workspace: *workspace,
                    result: Ok(()),
                },
            ),
            Effect::Stop { .. } => return Some(Completion::settle(1_800)),
            Effect::Purge { .. } => return Some(Completion::settle(2_200)),
            Effect::Refresh => (0, Msg::Refreshed { ok: false }),
            Effect::RefreshAccount {
                account,
                stagger_ms,
            } => {
                let base = world
                    .accounts
                    .get(account)
                    .map(provider::refresh_duration_ms)?;
                return Some(Completion::settle(base + stagger_ms));
            }
            Effect::ValidateAccount { .. } => return Some(Completion::settle(900)),
            Effect::CheckRole { role, doc } => (
                600,
                Msg::RoleChecked {
                    role: role.clone(),
                    findings: surfaces::check_role(doc),
                },
            ),
            Effect::SaveRole { role, .. } => (
                900,
                Msg::RoleSaved {
                    role: role.clone(),
                    ok: true,
                    error: None,
                },
            ),
            Effect::BindRole { role, agent, .. } => {
                let ok = !self.bind_fails_once.swap(false, Ordering::SeqCst);
                (
                    900,
                    Msg::RoleBound {
                        role: role.clone(),
                        agent: *agent,
                        ok,
                        error: (!ok).then(|| {
                            "could not write config.toml: permission denied (EACCES)".into()
                        }),
                    },
                )
            }
            Effect::AddRole { location } => (
                700,
                Msg::RoleAdded {
                    location: location.clone(),
                    result: surfaces::add_role(location, |name| {
                        world
                            .surfaces
                            .roles
                            .as_ref()
                            .is_some_and(|a| a.role(name).is_some())
                    })
                    .map(Box::new),
                },
            ),
            Effect::RunDoctor => {
                let run = self.doctor_runs.fetch_add(1, Ordering::SeqCst) + 2;
                (
                    900,
                    Msg::DoctorFinished(surfaces::doctor_report(run, world.now_secs() + 1)),
                )
            }
            Effect::LoadRole { .. } => return Some(Completion::settle(1_100)),
        };
        Some(Completion::ready(after_ms, msg))
    }

    fn settle(&self, effect: &Effect, world: &World) -> Option<Msg> {
        match effect {
            Effect::SaveWorkspace { id, workspace, .. } => {
                Some(self.settle_workspace_save(*id, workspace, world))
            }
            Effect::Stop { instance } => Some(Msg::Stopped {
                instance: instance.clone(),
                result: Ok(Box::new(left_running(world, instance, |i| {
                    if i.is_dirty() {
                        InstanceStatus::PreservedDirty
                    } else {
                        InstanceStatus::RestoreAvailable
                    }
                }))),
            }),
            Effect::Purge { instance } => Some(Msg::Purged {
                instance: instance.clone(),
                result: Ok(Box::new(left_running(world, instance, |_| {
                    InstanceStatus::Purged
                }))),
            }),
            Effect::RefreshAccount { account, .. } => {
                let refreshed = world.accounts.get(account).map(|a| {
                    provider::refresh(
                        a,
                        world.now_secs(),
                        world.op.session() != OpSession::SignedIn,
                        world.refresh_fails,
                    )
                });
                Some(Msg::AccountRefreshed {
                    account: account.clone(),
                    result: refreshed
                        .map(Box::new)
                        .ok_or_else(|| format!("{account} is not in the registry")),
                })
            }
            Effect::ValidateAccount { account } => {
                let result = match world.accounts.get(account) {
                    None => Err(format!("{account} is not in the registry")),
                    Some(a) => Ok(match &a.source {
                        CredentialSource::PlainApiKey { .. } => Validation::Fingerprint {
                            level: fingerprint_level(a),
                        },
                        source => Validation::Checked(Box::new(self.validate(
                            a.provider,
                            source,
                            None,
                            world.op.as_ref(),
                            world.now_secs(),
                        ))),
                    }),
                };
                Some(Msg::AccountValidated {
                    account: account.clone(),
                    result,
                })
            }
            Effect::TrustRole { role } => {
                let mut snapshot = snapshot(world);
                let result = match snapshot.roles.iter_mut().find(|r| r.name == *role) {
                    Some(r) => {
                        r.trusted = true;
                        let source = match &r.source {
                            RoleSource::Git { url, .. } => url.clone(),
                            RoleSource::Local { path } => path.clone(),
                        };
                        for row in snapshot
                            .global
                            .trust
                            .iter_mut()
                            .filter(|t| t.source == source)
                        {
                            row.trusted = true;
                        }
                        Ok(Box::new(snapshot))
                    }
                    None => Err(format!("{role} is not in the registry")),
                };
                Some(Msg::RoleTrusted {
                    role: role.clone(),
                    result,
                })
            }
            Effect::LoadRole { role } => {
                let found = world.roles.iter().find(|r| r.name == *role);
                let result = match found.and_then(|r| r.fetch_error.clone()) {
                    Some(error) => Err(error),
                    None => {
                        let mut snapshot = snapshot(world);
                        if let Some(r) = snapshot.roles.iter_mut().find(|r| r.name == *role) {
                            r.load_error = None;
                            r.in_registry = true;
                            r.trusted = true;
                        }
                        Ok(Box::new(snapshot))
                    }
                };
                Some(Msg::RoleLoaded {
                    role: role.clone(),
                    result,
                })
            }
            Effect::DeleteWorkspace { id } => {
                let mut snapshot = snapshot(world);
                snapshot.workspaces.retain(|w| w.id != *id);
                for i in &mut snapshot.instances {
                    if i.workspace == Some(*id) {
                        i.workspace = None;
                    }
                }
                Some(Msg::WorkspaceDeleted {
                    id: *id,
                    result: Ok(Box::new(snapshot)),
                })
            }
            Effect::Restore { instance } => {
                let mut snapshot = snapshot(world);
                let result = match snapshot.instances.iter_mut().find(|i| i.id == *instance) {
                    Some(i) => {
                        i.status = InstanceStatus::Running;
                        i.last_seen_secs = world.clock.now_secs();
                        Ok(Box::new(snapshot))
                    }
                    None => Err(format!("{instance} is not in the roster")),
                };
                Some(Msg::Restored {
                    instance: instance.clone(),
                    result,
                })
            }
            Effect::SaveGlobal { global, .. } => Some(self.settle_global_save(global, world)),
            Effect::SaveAccounts { base, registry } => {
                Some(settle_accounts_save(base, registry, world))
            }
            _ => None,
        }
    }

    fn changes(
        &self,
        instance_id: &str,
        touched: &[String],
        uncommitted: usize,
        unpushed: usize,
    ) -> ChangeSet {
        changes::changes_for(instance_id, touched, uncommitted, unpushed)
    }

    fn launch_content(&self) -> Arc<LaunchContent> {
        if self.openshell {
            launch::openshell_content()
        } else {
            launch::content()
        }
    }

    fn launch_plan(&self, world: &World, target: &LaunchTarget) -> LaunchPlan {
        match self.launch {
            LaunchRule::Clean => LaunchPlan::Clean,
            LaunchRule::FailNetwork => LaunchPlan::FailNetwork,
            LaunchRule::HardCases => {
                if world.op.session() == OpSession::Locked
                    && target.account.as_deref().is_some_and(|a| {
                        a.contains("work") || a.contains("grok") || a.contains("codex-primary")
                    })
                {
                    LaunchPlan::CredentialsLocked
                } else if target.role == "sre" {
                    LaunchPlan::BlockedSidecar
                } else {
                    LaunchPlan::Clean
                }
            }
        }
    }

    fn session_script(&self) -> Arc<dyn SessionScript> {
        scripts::SimScripts::shared()
    }
}

/// A removed account also leaves every workspace that named it. A discovered
/// account that the registry drops (the twin of a folder that was registered)
/// leaves them as they are.
fn settle_accounts_save(base: &AccountRegistry, registry: &AccountRegistry, world: &World) -> Msg {
    let mut snapshot = snapshot(world);
    for gone in base
        .accounts
        .iter()
        .filter(|a| a.origin == AccountOrigin::Registered && registry.get(&a.id).is_none())
    {
        for ws in &mut snapshot.workspaces {
            ws.accounts.enabled.remove(&gone.id);
            ws.accounts.disabled_defaults.remove(&gone.id);
            ws.accounts.preferred.retain(|_, v| *v != gone.id);
            ws.accounts.role_preferred.retain(|_, v| *v != gone.id);
        }
    }
    snapshot.accounts = registry.clone();
    Msg::AccountsSaved {
        result: Ok(Box::new(snapshot)),
    }
}

/// The roster after `instance` stopped running: its status, the time it was
/// last seen, and nothing else.
fn left_running(
    world: &World,
    instance: &str,
    status: impl Fn(&Instance) -> InstanceStatus,
) -> Snapshot {
    let mut snapshot = snapshot(world);
    let now = world.clock.now_secs();
    if let Some(i) = snapshot.instances.iter_mut().find(|i| i.id == instance) {
        i.status = status(i);
        i.last_seen_secs = now;
    }
    snapshot
}

/// How far a key that is held as a fingerprint can be vouched for.
fn fingerprint_level(account: &Account) -> ValidationLevel {
    if account.usage.windows.iter().any(|w| w.has_meter()) {
        ValidationLevel::QuotaReadable
    } else if account.identity.subject.is_some() {
        ValidationLevel::IdentityAuthenticated
    } else {
        ValidationLevel::MaterialDiscovered
    }
}

pub fn snapshot(world: &World) -> Snapshot {
    Snapshot {
        workspaces: world.workspaces.clone(),
        roles: world.roles.clone(),
        instances: world.instances.clone(),
        global: world.global.clone(),
        accounts: world.accounts.clone(),
        surfaces: world.surfaces.clone(),
        github: world.github.clone(),
        index_error: world.arbiter.discovery.err(),
    }
}
