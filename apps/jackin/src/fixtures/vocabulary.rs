use crate::domain::vocabulary::{DaemonWords, LifecycleWords, StatusWords, Vocabulary};

impl Vocabulary {
    pub fn demo() -> Self {
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
            github_no_url: "No GitHub URL to open for this instance".into(),
            github_menu: "Open on GitHub".into(),
            trust_browser: "Opened https://{source} in the browser".into(),
            trust_reveal: "Revealed {source} in Finder".into(),
            export_placeholder: "workspace path or /jackin/run/…".into(),
            trailer_sample: "Co-authored-by: <agent> <noreply@…>   ·   Signed-off-by: Alexey Zhokhov <alexey@chainargos.com>".into(),

            env_plain_label: "Plain text  (masked)".into(),
            env_plain_placeholder: "typed once · masked afterwards".into(),
            env_plain_help: None,
            env_op_label: "1Password item / field  (recommended for secrets)".into(),
            trust_hint: "Open source".into(),
            agent_no_credentials: "no credentials handed to the container".into(),

            about_label: "About jackin-preview".into(),
            paste_label: "Paste clipboard".into(),
            paste_empty: "Preview clipboard is empty".into(),
            command_unavailable: "{label}: not available in the preview".into(),
            detail_unavailable: "Not in this preview".into(),
            pane_starting: "spawning".into(),
            pane_cleared: "Pane cleared".into(),
            close_tab_body: "Reap every pane in this tab. Unsaved state across all panes is lost."
                .into(),
            close_agent_body: "Reap the focused pane's agent. Unsaved state in that pane is lost."
                .into(),
            close_shell_body: "Reap the focused pane's agent. Unsaved state in that pane is lost."
                .into(),
            export_done: "Exported to ~/Downloads/jackin/payments-platform/{path}".into(),
            export_revealed: Some(" · revealed in Finder".into()),
            export_opened: Some(" · opened on host".into()),
            host_log: Some("file:///Users/alexey/.jackin/logs/{run}.log".into()),
            workspace_config: Some("{home}/.jackin/workspaces/{name}.toml".into()),
            global_config: Some("{home}/.jackin/config.toml".into()),
            save_error_workspace: "write failed: ~/.jackin/workspaces is not writable (EACCES)"
                .into(),
            save_error_global: "write failed: ~/.jackin/config.toml is not writable (EACCES)"
                .into(),
        }
    }
}
