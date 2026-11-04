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
    pub github_no_url: String,
    pub github_menu: String,
    pub trust_browser: String,
    pub trust_reveal: String,
    pub export_placeholder: String,
    pub trailer_sample: String,

    pub env_plain_label: String,
    pub env_plain_placeholder: String,
    pub env_plain_help: Option<String>,
    pub env_op_label: String,
    pub trust_hint: String,
    pub agent_no_credentials: String,

    pub op_reference_note: String,

    pub about_label: String,
    pub paste_label: String,
    pub paste_empty: String,
    pub paste_empty_reason: String,
    pub command_unavailable: String,
    pub detail_unavailable: String,
    pub pane_starting: String,
    pub pane_cleared: String,
    pub close_tab_body: String,
    pub close_agent_body: String,
    pub close_shell_body: String,
    pub export_done: String,
    pub export_revealed: Option<String>,
    pub export_opened: Option<String>,
    pub host_log: Option<String>,
    pub workspace_config: Option<String>,
    pub global_config: Option<String>,
    pub save_error_workspace: String,
    pub save_error_global: String,
    pub save_error_next: String,
    pub sandbox_id: Option<String>,
    pub invocation_id: Option<String>,
    pub attached_status: String,
}

impl Vocabulary {
    pub fn openshell() -> Self {
        Self {
            construct: "the sandbox".into(),
            instances_where: "on the gateway".into(),
            still_inside: "Still running on the gateway".into(),
            health_noun: "gateway".into(),
            capsule: "Capsule".into(),
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
                inspect_label: "Capsule".into(),
                inspect_unavailable: "gateway unreachable".into(),
                no_tabs: "No Capsule tabs open".into(),
                unavailable_running: "Gateway unreachable — showing saved sessions".into(),
                unavailable_stopped: "No live Capsule (sandbox not running)".into(),
                topology_source: "gateway · {ago}".into(),
                topology_unavailable: "unreachable".into(),
                card_unavailable: "running {duration} · gateway unreachable".into(),
                list_tabs: "capsule · {tabs} · {panes}".into(),
                list_no_tabs: "capsule · no tabs".into(),
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
            github_no_url: "No GitHub URL for this sandbox".into(),
            github_menu: "Show GitHub URL".into(),
            trust_browser: "Source · {source}".into(),
            trust_reveal: "Source · {source}".into(),
            export_placeholder: "path in /sandbox/project".into(),
            trailer_sample: "None · jackin adds no trailers to commits".into(),

            env_plain_label: "Plain text".into(),
            env_plain_placeholder: "type the value".into(),
            env_plain_help: Some("value is stored as plain text".into()),
            env_op_label: "1Password item / field".into(),
            trust_hint: "Show source".into(),
            agent_no_credentials: "no credentials handed to the sandbox".into(),

            op_reference_note:
                "only the reference is saved · read here for usage only".into(),

            about_label: "About jackin".into(),
            paste_label: "Paste last copy".into(),
            paste_empty: "Nothing copied in this console yet".into(),
            paste_empty_reason: "Nothing copied yet".into(),
            command_unavailable: "{label}: not supported".into(),
            detail_unavailable: "Not supported".into(),
            pane_starting: "Starting…".into(),
            pane_cleared: "Scrollback cleared".into(),
            close_tab_body: "Closes every pane in this tab. Codex panes keep running; shells end."
                .into(),
            close_agent_body: "Codex keeps running in the sandbox. This pane detaches.".into(),
            close_shell_body: "The shell ends. Unsaved state in it is lost.".into(),
            export_done: "Exported to ~/Downloads/jackin/{workspace}/{path}".into(),
            export_revealed: None,
            export_opened: None,
            host_log: None,
            workspace_config: Some("{config}".into()),
            global_config: Some("{config}".into()),
            save_error_workspace: "Could not write {config}.".into(),
            save_error_global: "Could not write {config}.".into(),
            save_error_next: "check disk space and permissions, then Save again".into(),
            sandbox_id: None,
            invocation_id: None,
            attached_status: "Attached to {name}".into(),
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
            ("github_no_url", &self.github_no_url),
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
            ("env_op_label", &self.env_op_label),
            ("trust_hint", &self.trust_hint),
            ("agent_no_credentials", &self.agent_no_credentials),
            ("op_reference_note", &self.op_reference_note),
            ("about_label", &self.about_label),
            ("paste_label", &self.paste_label),
            ("paste_empty", &self.paste_empty),
            ("paste_empty_reason", &self.paste_empty_reason),
            ("command_unavailable", &self.command_unavailable),
            ("detail_unavailable", &self.detail_unavailable),
            ("pane_starting", &self.pane_starting),
            ("pane_cleared", &self.pane_cleared),
            ("close_tab_body", &self.close_tab_body),
            ("close_agent_body", &self.close_agent_body),
            ("close_shell_body", &self.close_shell_body),
            ("export_done", &self.export_done),
            (
                "export_revealed",
                self.export_revealed.as_deref().unwrap_or(""),
            ),
            ("export_opened", self.export_opened.as_deref().unwrap_or("")),
            ("host_log", self.host_log.as_deref().unwrap_or("")),
            (
                "workspace_config",
                self.workspace_config.as_deref().unwrap_or(""),
            ),
            ("global_config", self.global_config.as_deref().unwrap_or("")),
            ("save_error_workspace", &self.save_error_workspace),
            ("save_error_global", &self.save_error_global),
            ("save_error_next", &self.save_error_next),
            ("sandbox_id", self.sandbox_id.as_deref().unwrap_or("")),
            ("invocation_id", self.invocation_id.as_deref().unwrap_or("")),
            ("attached_status", &self.attached_status),
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

pub fn fill_in(template: &str, pairs: &[(&str, &str)]) -> String {
    pairs
        .iter()
        .fold(template.to_owned(), |text, (key, value)| {
            text.replace(&format!("{{{key}}}"), value)
        })
}
