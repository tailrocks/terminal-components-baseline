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
pub struct DaemonWords {
    pub inspect_label: String,
    pub inspect_unavailable: String,
    pub no_tabs: String,
    pub unavailable_running: String,
    pub unavailable_stopped: String,
    pub topology_source: String,
    pub topology_unavailable: String,
    pub card_unavailable: String,
    pub list_tabs: String,
    pub list_no_tabs: String,
    pub list_unavailable: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LifecycleWords {
    pub restored: String,
    pub stop_body: String,
    pub purge_action: String,
    pub purge_scope: String,
    pub copy_dirty: String,
    pub copy_clean: String,
    pub risk_dirty: String,
    pub risk_clean: String,
    pub purged: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Vocabulary {
    pub construct: String,
    pub instances_where: String,
    pub still_inside: String,
    pub health_noun: String,
    pub capsule: String,
    pub version_label: String,
    pub container_noun: String,
    pub quit_lines: [String; 2],
    pub unexported_word: String,
    pub quit_dirty_keep: String,
    pub quit_dirty_discard: String,
    pub quit_discard_title: String,
    pub quit_discard_action: String,
    pub quit_discard_button: String,
    pub last_session_ended: String,
    pub never_reached: String,
    pub session_choice: String,
    pub status: StatusWords,
    pub daemon: DaemonWords,
    pub lifecycle: LifecycleWords,

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
    pub github_repo_mount: Option<String>,
    pub github_copies_url: bool,
    pub github_open_label: String,
    pub github_open_meta: String,
    pub github_copied: String,
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
            instances_where: "on the gateway".into(),
            still_inside: "Still running on the gateway".into(),
            health_noun: "gateway".into(),
            capsule: "Terminal".into(),
            version_label: "jackin".into(),
            container_noun: "Sandbox".into(),
            quit_lines: [
                "! Quitting detaches; the sandbox keeps running.".into(),
                "! Work stays in the sandbox until you export it.".into(),
            ],
            unexported_word: "unexported".into(),
            quit_dirty_keep: "Quit & keep the sandbox".into(),
            quit_dirty_discard: "Quit & delete the sandbox".into(),
            quit_discard_title: "Delete the sandbox and quit?".into(),
            quit_discard_action: "delete the sandbox and all work in it".into(),
            quit_discard_button: "Delete".into(),
            last_session_ended: "Last session ended · the sandbox keeps running".into(),
            never_reached: "never got ready".into(),
            session_choice: "Session choice · picked for this session only".into(),
            status: StatusWords {
                running: "The sandbox is running.".into(),
                clean_exited: "The final session ended cleanly; nothing to attach to.".into(),
                crashed: "The agent exited with an error; the sandbox is kept for inspection."
                    .into(),
                preserved_dirty: "Has uncommitted changes in the sandbox; kept until you decide."
                    .into(),
                preserved_unpushed: "Has commits that were never exported; kept until you decide."
                    .into(),
                restore_available: "Stopped cleanly; restore it to continue.".into(),
                superseded: "Replaced by a newer instance for the same Workspace.".into(),
                purged: "The sandbox was deleted.".into(),
                failed_setup: "Launch failed before the sandbox was ready; nothing to attach to."
                    .into(),
            },
            daemon: DaemonWords {
                inspect_label: "Terminal".into(),
                inspect_unavailable: "gateway unreachable".into(),
                no_tabs: "No terminal tabs open".into(),
                unavailable_running: "Gateway unreachable — showing saved sessions".into(),
                unavailable_stopped: "No live terminal (sandbox not running)".into(),
                topology_source: "gateway · {ago}".into(),
                topology_unavailable: "unreachable".into(),
                card_unavailable: "running {duration} · gateway unreachable".into(),
                list_tabs: "terminal · {tabs} · {panes}".into(),
                list_no_tabs: "terminal · no tabs".into(),
                list_unavailable: "gateway unreachable".into(),
            },
            lifecycle: LifecycleWords {
                restored: "Restored {id} · sandbox restarted".into(),
                stop_body: "The sandbox stops and every session in it ends. You can restore it later.".into(),
                purge_action: "delete the sandbox".into(),
                purge_scope: "{sessions} · {copy}".into(),
                copy_dirty: "changed project copy".into(),
                copy_clean: "clean project copy".into(),
                risk_dirty: "uncommitted or unexported work is lost".into(),
                risk_clean: "the sandbox and its files are deleted".into(),
                purged: "Purged {id} · the sandbox was deleted".into(),
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
            github_repo_mount: None,
            github_copies_url: true,
            github_open_label: "Copy URL".into(),
            github_open_meta: "Enter copies the row · y copies".into(),
            github_copied: "Copied the GitHub URL".into(),
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
        let (s, d, l) = (&self.status, &self.daemon, &self.lifecycle);
        vec![
            ("construct", self.construct.as_str()),
            ("instances_where", &self.instances_where),
            ("still_inside", &self.still_inside),
            ("health_noun", &self.health_noun),
            ("capsule", &self.capsule),
            ("version_label", &self.version_label),
            ("container_noun", &self.container_noun),
            ("quit_lines.0", &self.quit_lines[0]),
            ("quit_lines.1", &self.quit_lines[1]),
            ("unexported_word", &self.unexported_word),
            ("quit_dirty_keep", &self.quit_dirty_keep),
            ("quit_dirty_discard", &self.quit_dirty_discard),
            ("quit_discard_title", &self.quit_discard_title),
            ("quit_discard_action", &self.quit_discard_action),
            ("quit_discard_button", &self.quit_discard_button),
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
            ("daemon.inspect_label", &d.inspect_label),
            ("daemon.inspect_unavailable", &d.inspect_unavailable),
            ("daemon.no_tabs", &d.no_tabs),
            ("daemon.unavailable_running", &d.unavailable_running),
            ("daemon.unavailable_stopped", &d.unavailable_stopped),
            ("daemon.topology_source", &d.topology_source),
            ("daemon.topology_unavailable", &d.topology_unavailable),
            ("daemon.card_unavailable", &d.card_unavailable),
            ("daemon.list_tabs", &d.list_tabs),
            ("daemon.list_no_tabs", &d.list_no_tabs),
            ("daemon.list_unavailable", &d.list_unavailable),
            ("lifecycle.restored", &l.restored),
            ("lifecycle.stop_body", &l.stop_body),
            ("lifecycle.purge_action", &l.purge_action),
            ("lifecycle.purge_scope", &l.purge_scope),
            ("lifecycle.copy_dirty", &l.copy_dirty),
            ("lifecycle.copy_clean", &l.copy_clean),
            ("lifecycle.risk_dirty", &l.risk_dirty),
            ("lifecycle.risk_clean", &l.risk_clean),
            ("lifecycle.purged", &l.purged),
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
            (
                "github_repo_mount",
                self.github_repo_mount.as_deref().unwrap_or(""),
            ),
            ("github_open_label", &self.github_open_label),
            ("github_open_meta", &self.github_open_meta),
            ("github_copied", &self.github_copied),
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

    pub fn status_label(&self, status: InstanceStatus) -> String {
        match status {
            InstanceStatus::PreservedUnpushed => format!("preserved · {}", self.unexported_word),
            other => other.label().to_owned(),
        }
    }

    pub fn dirty_summary(&self, i: &super::instance::Instance) -> String {
        let word = &self.unexported_word;
        match (i.uncommitted, i.unpushed) {
            (0, 0) => "clean".into(),
            (u, 0) => format!("{u} uncommitted"),
            (0, p) => format!("{p} {word}"),
            (u, p) => format!("{u} uncommitted · {p} {word}"),
        }
    }

    pub fn count(&self, n: usize) -> String {
        junie_tui::ui::text::plural(n, &self.project_one, &self.project_many)
    }

    pub fn info_label(&self) -> String {
        format!("{} info", self.container_noun)
    }

    pub fn noun_lower(&self) -> String {
        self.container_noun.to_lowercase()
    }
}

impl Default for Vocabulary {
    fn default() -> Self {
        Self {
            construct: "the Construct".into(),
            instances_where: "in the Construct".into(),
            still_inside: "Still inside the Construct".into(),
            health_noun: "daemon".into(),
            capsule: "Capsule".into(),
            version_label: "Capsule".into(),
            container_noun: "Container".into(),
            quit_lines: [
                "! Quitting force-stops the container immediately.".into(),
                "! Work not saved outside the container will be lost.".into(),
            ],
            unexported_word: "unpushed".into(),
            quit_dirty_keep: "Quit & keep changes".into(),
            quit_dirty_discard: "Quit & discard changes".into(),
            quit_discard_title: "Quit and discard changes?".into(),
            quit_discard_action: "discard every uncommitted change and unpushed commit".into(),
            quit_discard_button: "Discard".into(),
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
            daemon: DaemonWords {
                inspect_label: "Daemon".into(),
                inspect_unavailable: "unavailable".into(),
                no_tabs: "Daemon reports no tabs".into(),
                unavailable_running: "Daemon unavailable — showing manifest sessions".into(),
                unavailable_stopped: "No live daemon (instance not running)".into(),
                topology_source: "daemon · {ago}".into(),
                topology_unavailable: "unavailable".into(),
                card_unavailable: "running {duration} · daemon unavailable".into(),
                list_tabs: "daemon · {tabs} · {panes}".into(),
                list_no_tabs: "daemon reports no tabs".into(),
                list_unavailable: "daemon unavailable".into(),
            },
            lifecycle: LifecycleWords {
                restored: "Restored {id} · container restarted".into(),
                stop_body: "The container stops and every session in it ends. The instance record stays reconnectable ({state}).".into(),
                purge_action: "purge the instance record and its container".into(),
                purge_scope: "1 container · {sessions} · {copy}".into(),
                copy_dirty: "worktree with changes".into(),
                copy_clean: "clean worktree".into(),
                risk_dirty: "uncommitted changes in the worktree are lost".into(),
                risk_clean: "the container and its recovery state are removed".into(),
                purged: "Purged {id} · container, sidecar, volume and recovery state removed".into(),
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
            github_repo_mount: Some("Opened {url} on GitHub".into()),
            github_copies_url: false,
            github_open_label: "Open PR…".into(),
            github_open_meta: "open uses a gated host action".into(),
            github_copied: "Opened the pull request on the host (gated host action)".into(),
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
