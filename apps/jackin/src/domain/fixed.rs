use junie_tui::core::refusal::Reason;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Fixed {
    pub keep_awake: Reason,
    pub git_pull: Reason,
    pub dirty_exit: Reason,
    pub mounts: Reason,
    pub workdir: Reason,
    pub enable_workspace: Reason,

    pub last_role: String,
}

impl Fixed {
    pub fn openshell() -> Self {
        Self {
            keep_awake: Reason::new("Always off · set sleep on the gateway host"),
            git_pull: Reason::new("Always off · never edits your checkout"),
            dirty_exit: Reason::new("Always ask · work is never dropped silently"),
            mounts: Reason::new("jackin uploads one project and mounts nothing"),
            workdir: Reason::new("Always /sandbox/project · jackin uploads the project there"),
            enable_workspace: Reason::new("All accounts work here · press p to prefer one"),
            last_role: "Keep one role allowed, or allow all".into(),
        }
    }
}
