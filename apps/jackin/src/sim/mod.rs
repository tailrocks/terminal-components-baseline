use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicUsize, Ordering};

use crate::backend::{
    Backend, ChangeSet, Completion, Effect, FolderProbe, OnePassword, OpSession, ValidationOutcome,
};
use crate::domain::account::CredentialSource;
use crate::domain::agent::Provider;
use crate::domain::launch::LaunchContent;
use crate::domain::vocabulary::fill_in;
use crate::domain::workspace::{Workspace, WorkspaceId};
use crate::launch::{LaunchPlan, LaunchTarget};
use crate::panes::SessionScript;
use crate::world::{Msg, Snapshot, World};

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
            Effect::SaveGlobal => (900, Msg::GlobalSaved { ok: self.save_ok() }),
            Effect::Prewarm { workspace } => (
                2_400,
                Msg::Prewarmed {
                    workspace: *workspace,
                },
            ),
            Effect::Stop { instance } => (
                1_800,
                Msg::Stopped {
                    instance: instance.clone(),
                },
            ),
            Effect::Purge { instance } => (
                2_200,
                Msg::Purged {
                    instance: instance.clone(),
                },
            ),
            Effect::Refresh => (0, Msg::Refreshed { ok: false }),
            Effect::RefreshAccount {
                account,
                stagger_ms,
            } => {
                let base = world
                    .accounts
                    .get(account)
                    .map(provider::refresh_duration_ms)?;
                (
                    base + stagger_ms,
                    Msg::AccountRefreshed {
                        account: account.clone(),
                    },
                )
            }
            Effect::ValidateAccount { account } => (
                900,
                Msg::AccountValidated {
                    account: account.clone(),
                },
            ),
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
            Effect::LoadRole { role } => {
                let error = world
                    .roles
                    .iter()
                    .find(|r| r.name == *role)
                    .and_then(|r| r.fetch_error.clone());
                (
                    1_100,
                    Msg::RoleLoaded {
                        role: role.clone(),
                        ok: error.is_none(),
                        error,
                    },
                )
            }
        };
        Some(Completion::ready(after_ms, msg))
    }

    fn settle(&self, effect: &Effect, world: &World) -> Option<Msg> {
        match effect {
            Effect::SaveWorkspace { id, workspace, .. } => {
                Some(self.settle_workspace_save(*id, workspace, world))
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
