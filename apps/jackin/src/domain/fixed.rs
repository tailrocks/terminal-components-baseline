use junie_tui::core::refusal::Reason;

use super::agent::Agent;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Fixed {
    pub keep_awake: Reason,
    pub git_pull: Reason,
    pub dirty_exit: Reason,
    pub mounts: Reason,
    pub workdir: Reason,
    pub enable_workspace: Reason,

    pub last_role: String,

    pub commits: Reason,
    pub trust: Reason,
    pub env_sources: Reason,
    pub role_env: Reason,
    pub launchable: Agent,
    pub agent_launchable: Reason,
    pub agent_other: Reason,
    pub agent_notes: [String; 2],
    pub env_secret_note: [String; 2],
    pub env_reference: String,
}

impl Fixed {
    pub fn openshell() -> Self {
        Self {
            keep_awake: Reason::new("Always off · set sleep on the gateway host"),
            git_pull: Reason::new("Always off · never edits your checkout"),
            dirty_exit: Reason::new("Always ask · work is never dropped silently"),
            mounts: Reason::new("jackin uploads one project and mounts nothing"),
            workdir: Reason::new("Always /sandbox/project · jackin uploads the project there"),
            enable_workspace: Reason::new("All accounts are active here · press p to prefer one"),
            last_role: "Keep one role allowed · allow another first".into(),
            commits: Reason::new("Both stay off · jackin does not edit commit messages"),
            trust: Reason::new("Sources you add are trusted · remove the role to revoke"),
            env_sources: Reason::new("Plain values only · use an account for credentials"),
            role_env: Reason::new("Set role variables in a workspace instead"),
            launchable: Agent::Codex,
            agent_launchable: Reason::new("Set by each account · change it in Accounts (c)"),
            agent_other: Reason::new("No credentials · jackin launches Codex only"),
            agent_notes: [
                "The mode comes from each account's credential source.".into(),
                "jackin launches Codex only; other agents get no credentials.".into(),
            ],
            env_secret_note: [
                "Keep secrets out of env. Values are stored as plain text.".into(),
                "Use an account for credentials.".into(),
            ],
            env_reference: "Plain values only · use an account for credentials".into(),
        }
    }
}
