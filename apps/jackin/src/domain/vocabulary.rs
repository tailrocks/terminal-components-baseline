use super::instance::InstanceStatus;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StatusWords {
    pub running: String,
    pub clean_exited: String,
    pub crashed: String,
    pub preserved_dirty: String,
    pub preserved_unpushed: String,
    pub restore_available: String,
    pub superseded: String,
    pub purged: String,
    pub failed_setup: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Vocabulary {
    pub construct: String,
    pub capsule: String,
    pub last_session_ended: String,
    pub never_reached: String,
    pub session_choice: String,
    pub status: StatusWords,

    pub project_tab: String,
    pub project_one: String,
    pub project_many: String,
    pub project_card: String,
    pub first_use: String,
    pub role_access: String,
    pub destination_heading: String,
    pub git_url_help: [String; 2],
    pub read_only_label: String,

    pub github_workspace: String,
    pub github_git_mount: String,
    pub github_repo_mount: String,
    pub github_pull_request: String,
    pub github_menu: String,
    pub trust_browser: String,
    pub trust_reveal: String,
    pub export_placeholder: String,
    pub trailer_sample: String,

    pub env_plain_label: String,
    pub env_plain_placeholder: String,
    pub env_plain_help: Option<String>,
}

impl Vocabulary {
    pub fn openshell() -> Self {
        Self {
            construct: "the sandbox".into(),
            capsule: "Terminal".into(),
            last_session_ended: "Last session ended · the sandbox keeps running".into(),
            never_reached: "never became ready".into(),
            session_choice: "Session choice · picked for this session in the terminal".into(),
            status: StatusWords {
                running: "The sandbox is running.".into(),
                clean_exited: "The final session ended cleanly; the sandbox is stopped.".into(),
                crashed: "The main process stopped unexpectedly; the sandbox is kept for inspection."
                    .into(),
                preserved_dirty:
                    "Stopped with uncommitted changes in the Workspace; kept until you decide."
                        .into(),
                preserved_unpushed:
                    "Stopped with commits that were never pushed; kept until you decide.".into(),
                restore_available: "Stopped cleanly; start it again to continue.".into(),
                superseded: "Replaced by a newer instance for the same Workspace.".into(),
                purged: "The sandbox and its records were removed.".into(),
                failed_setup: "Launch failed before the sandbox was ready; nothing to attach to."
                    .into(),
            },

            project_tab: "Project".into(),
            project_one: "project".into(),
            project_many: "projects".into(),
            project_card: "{source} · uploaded".into(),
            first_use: "{path} is uploaded to /sandbox/project. Enter launches with defaults; n creates a saved workspace.".into(),
            role_access: "The role can access the uploaded project files.".into(),
            destination_heading: "Project destination".into(),
            git_url_help: [
                "jackin uploads a folder from this machine.".into(),
                "Clone the repository first, then choose its folder.".into(),
            ],
            read_only_label: "Read-only".into(),

            github_workspace: "GitHub · {url}".into(),
            github_git_mount: "GitHub · https://{url}".into(),
            github_repo_mount: "GitHub · {path}".into(),
            github_pull_request: "GitHub · the pull request".into(),
            github_menu: "Show GitHub URL".into(),
            trust_browser: "Source · {source}".into(),
            trust_reveal: "Source · {source}".into(),
            export_placeholder: "path in /sandbox/project".into(),
            trailer_sample: "None · jackin adds no trailers to commits".into(),

            env_plain_label: "Plain text".into(),
            env_plain_placeholder: "type the value".into(),
            env_plain_help: Some("value is stored as plain text".into()),
        }
    }

    pub fn fields(&self) -> Vec<(&'static str, &str)> {
        let s = &self.status;
        vec![
            ("construct", self.construct.as_str()),
            ("capsule", &self.capsule),
            ("last_session_ended", &self.last_session_ended),
            ("never_reached", &self.never_reached),
            ("session_choice", &self.session_choice),
            ("status.running", &s.running),
            ("status.clean_exited", &s.clean_exited),
            ("status.crashed", &s.crashed),
            ("status.preserved_dirty", &s.preserved_dirty),
            ("status.preserved_unpushed", &s.preserved_unpushed),
            ("status.restore_available", &s.restore_available),
            ("status.superseded", &s.superseded),
            ("status.purged", &s.purged),
            ("status.failed_setup", &s.failed_setup),
            ("project_tab", &self.project_tab),
            ("project_one", &self.project_one),
            ("project_many", &self.project_many),
            ("project_card", &self.project_card),
            ("first_use", &self.first_use),
            ("role_access", &self.role_access),
            ("destination_heading", &self.destination_heading),
            ("git_url_help.0", &self.git_url_help[0]),
            ("git_url_help.1", &self.git_url_help[1]),
            ("read_only_label", &self.read_only_label),
            ("github_workspace", &self.github_workspace),
            ("github_git_mount", &self.github_git_mount),
            ("github_repo_mount", &self.github_repo_mount),
            ("github_pull_request", &self.github_pull_request),
            ("github_menu", &self.github_menu),
            ("trust_browser", &self.trust_browser),
            ("trust_reveal", &self.trust_reveal),
            ("export_placeholder", &self.export_placeholder),
            ("trailer_sample", &self.trailer_sample),
            ("env_plain_label", &self.env_plain_label),
            ("env_plain_placeholder", &self.env_plain_placeholder),
            (
                "env_plain_help",
                self.env_plain_help.as_deref().unwrap_or(""),
            ),
        ]
    }

    pub fn status_description(&self, status: InstanceStatus) -> &str {
        let s = &self.status;
        match status {
            InstanceStatus::Running => &s.running,
            InstanceStatus::CleanExited => &s.clean_exited,
            InstanceStatus::Crashed => &s.crashed,
            InstanceStatus::PreservedDirty => &s.preserved_dirty,
            InstanceStatus::PreservedUnpushed => &s.preserved_unpushed,
            InstanceStatus::RestoreAvailable => &s.restore_available,
            InstanceStatus::Superseded => &s.superseded,
            InstanceStatus::Purged => &s.purged,
            InstanceStatus::FailedSetup => &s.failed_setup,
        }
    }

    pub fn count(&self, n: usize) -> String {
        junie_tui::ui::text::plural(n, &self.project_one, &self.project_many)
    }
}

impl Default for Vocabulary {
    fn default() -> Self {
        Self {
            construct: "the Construct".into(),
            capsule: "Capsule".into(),
            last_session_ended: "Last session ended · the Capsule shuts down".into(),
            never_reached: "never reached the Capsule".into(),
            session_choice: "Session choice · picked for this session in Capsule".into(),
            status: StatusWords {
                running: "Container and Capsule daemon are live.".into(),
                clean_exited: "The final session ended cleanly; the container is stopped.".into(),
                crashed:
                    "The Capsule daemon stopped unexpectedly; the container is kept for inspection."
                        .into(),
                preserved_dirty:
                    "Stopped with uncommitted changes in the Workspace; kept until you decide."
                        .into(),
                preserved_unpushed:
                    "Stopped with commits that were never pushed; kept until you decide.".into(),
                restore_available: "Stopped cleanly with a restorable Capsule layout.".into(),
                superseded: "Replaced by a newer instance for the same Workspace.".into(),
                purged: "Container and records were removed.".into(),
                failed_setup: "Launch failed before the Capsule was ready; nothing to attach to."
                    .into(),
            },

            project_tab: "Mounts".into(),
            project_one: "mount".into(),
            project_many: "mounts".into(),
            project_card: "{source} · {mode} {isolation}".into(),
            first_use: "{path} is mounted at its own path inside the Construct. Enter launches with defaults; n creates a saved workspace.".into(),
            role_access: "The role can access mounted workspace files.".into(),
            destination_heading: "Mount destination".into(),
            git_url_help: [
                "Enter a GitHub repository URL. The repository is cloned into the".into(),
                "Construct at launch; the branch is chosen in the mount row.".into(),
            ],
            read_only_label: "Mount read-only".into(),

            github_workspace: "Opened {url} on the host".into(),
            github_git_mount: "Opened https://{url} in the browser".into(),
            github_repo_mount: "Opened {path} on GitHub".into(),
            github_pull_request: "Opened the pull request on the host (gated host action)".into(),
            github_menu: "Open on GitHub".into(),
            trust_browser: "Opened https://{source} in the browser".into(),
            trust_reveal: "Revealed {source} in Finder".into(),
            export_placeholder: "workspace path or /jackin/run/…".into(),
            trailer_sample: "Co-authored-by: <agent> <noreply@…>   ·   Signed-off-by: Alexey Zhokhov <alexey@chainargos.com>".into(),

            env_plain_label: "Plain text  (masked)".into(),
            env_plain_placeholder: "typed once · masked afterwards".into(),
            env_plain_help: None,
        }
    }
}

pub fn fill_in(template: &str, pairs: &[(&str, &str)]) -> String {
    pairs
        .iter()
        .fold(template.to_owned(), |text, (key, value)| {
            text.replace(&format!("{{{key}}}"), value)
        })
}
