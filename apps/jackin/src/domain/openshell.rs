//! The one definition of every Fixed value of the OpenShell world.
//!
//! The fixture world of `openshell-fixed` and jackin's projection both call
//! these functions, so the two cannot disagree. The class of each field is in
//! `fixed_fields`.

use super::account::{Account, AccountOrigin, AccountRegistry, EditBlocked};
use super::agent::{Agent, AuthMode};
use super::fixed::Fixed;
use super::fixed_fields::SurfaceFixed;
use super::instance::Instance;
use super::surfaces::Surfaces;
use super::workspace::{DirtyExitPolicy, Isolation, Mount, RoleEntry, Workspace};
use crate::world::GlobalConfig;

pub const PROJECT_DIR: &str = "/sandbox/project";

pub fn account(a: &mut Account) {
    a.enabled = true;
    a.origin = AccountOrigin::Registered;
}

/// Edit… is refused for an account that reads a host variable: the account
/// form has no such source. The variable name is cut to 16 columns plus `…`,
/// so the text stays within `Reason::TEXT_MAX`.
pub fn env_edit_block(var: &str) -> EditBlocked {
    let var = junie_tui::ui::text::truncate(var, 17);
    EditBlocked::new(
        &format!("reads {var} · change it with jackin account add"),
        "env account",
    )
}

/// `host_path` is Data going in. `accounts` is the registry with `enabled` and
/// `default_for_provider` already set. The set `accounts.enabled` is written
/// here: it holds every account that is not a provider default, so the
/// workspace's effective set is the whole registry.
pub fn workspace(ws: &mut Workspace, host_path: &str, accounts: &AccountRegistry) {
    ws.workdir = PROJECT_DIR.into();
    ws.mounts = vec![Mount::host(host_path, PROJECT_DIR).isolation(Isolation::Clone)];
    ws.keep_awake = false;
    ws.git_pull = false;
    ws.dirty_policy = DirtyExitPolicy::Ask;
    ws.accounts.disabled_defaults.clear();
    ws.accounts.enabled = accounts
        .accounts
        .iter()
        .filter(|a| !a.default_for_provider)
        .map(|a| a.id.clone())
        .collect();
    ws.accounts.role_preferred.clear();
}

/// Every agent but the launchable one is `Ignore`; the launchable agent keeps
/// its mode, or gets `Sync` when it has none. `env` is left alone.
pub fn global(g: &mut GlobalConfig) {
    let launchable = Fixed::openshell().launchable;
    g.coauthor_trailer = false;
    g.dco_signoff = false;
    g.mounts.clear();
    g.role_env.clear();
    g.agent_modes = Agent::ALL
        .into_iter()
        .map(|a| {
            let mode = if a == launchable {
                g.agent_modes.get(&a).copied().unwrap_or(AuthMode::Sync)
            } else {
                AuthMode::Ignore
            };
            (a, mode)
        })
        .collect();
    for row in &mut g.trust {
        row.trusted = true;
    }
}

pub fn role(r: &mut RoleEntry) {
    r.trusted = true;
}

pub fn instance(i: &mut Instance) {
    i.workdir = PROJECT_DIR.into();
}

pub fn surfaces(s: &mut Surfaces) {
    let SurfaceFixed {
        vocabulary,
        fixed,
        account_form,
        agent_blocked,
        wizard,
        launch_role,
    } = SurfaceFixed::openshell();
    s.vocabulary = vocabulary;
    s.fixed = fixed;
    s.account_form = account_form;
    s.agent_blocked = agent_blocked;
    s.wizard = wizard;
    s.launch_role = launch_role;
}
