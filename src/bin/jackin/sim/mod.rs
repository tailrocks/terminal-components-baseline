use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

use junie_tui::console::backend::{
    Backend, ChangeSet, Completion, Effect, FolderProbe, OnePassword, ValidationOutcome,
};
use junie_tui::console::domain::account::CredentialSource;
use junie_tui::console::domain::agent::Provider;
use junie_tui::console::launch::LaunchContent;
use junie_tui::console::panes::SessionScript;
use junie_tui::console::world::{Msg, World};

pub mod changes;
pub mod launch;
pub mod onepassword;
pub mod provider;
pub mod scripts;
pub mod surfaces;

#[derive(Debug, Default)]
pub struct SimBackend {
    save_fails_once: AtomicBool,
    bind_fails_once: AtomicBool,
    doctor_runs: AtomicUsize,
    openshell: bool,
}

impl SimBackend {
    pub fn shared() -> Arc<dyn Backend> {
        Arc::new(SimBackend::default())
    }

    pub fn failing_first_save() -> Arc<dyn Backend> {
        Arc::new(SimBackend {
            save_fails_once: AtomicBool::new(true),
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
            Effect::SaveWorkspace { id } => (
                900,
                Msg::WorkspaceSaved {
                    id: *id,
                    ok: self.save_ok(),
                },
            ),
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
        Some(Completion { after_ms, msg })
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

    fn session_script(&self) -> Arc<dyn SessionScript> {
        scripts::SimScripts::shared()
    }
}
