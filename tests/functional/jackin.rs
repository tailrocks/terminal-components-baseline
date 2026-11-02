use super::support::{App, JACKIN, Point, crop, find, find_all, row_text, text};

const PAUSED: &[&str] = &["--motion", "paused", "--frame", "40"];

fn button(app: &App, label: &str) -> Point {
    let screen = app.wait_text(label);

    let candidates: Vec<_> = find_all(&screen, label)
        .into_iter()
        .filter(|point| {
            u32::from(point.y) + 2 < screen.rows()
                && row_text(&screen, u32::from(point.y)).contains("Cancel")
                && row_text(&screen, u32::from(point.y)).contains('│')
        })
        .collect();
    assert_eq!(
        candidates.len(),
        1,
        "one dialog action {label:?}:\n{}",
        text(&screen)
    );
    candidates[0]
}

fn jackin(name: &str, motion: &str) -> App {
    App::new(
        name,
        JACKIN,
        &[
            "--scenario",
            "returning",
            "--motion",
            motion,
            "--frame",
            "40",
        ],
        120,
        40,
        "truecolor",
    )
}

#[test]
fn jackin_launch_detach_reconnect_preserves_capsule() {
    let app = jackin("jackin-launch", "paused");
    let manager = app.wait_text("Current directory");
    assert!(text(&manager).contains("2 running"));
    app.key("enter");
    let picker = app.wait_text("Launch · choose Agent");
    assert!(text(&picker).contains("payments-platform › the-architect"));

    app.visual("functional/jackin_launch_reconnect");
    app.key("enter");
    app.wait_text("Loading the-architect into workspace payments-platform");
    app.advance_clock(30_000);
    let capsule = app.wait_screen("launch hands off to capsule", |s| {
        let t = text(s);
        t.contains("Claude Code v2.1.14")
            && t.contains("Allow edit to src/settlement/config.rs? (y/n)")
            && t.contains("Ctrl+B Pane commands")
    });
    assert!(!text(&capsule).contains("junie-tui:advance-clock:"));
    assert!(!text(&capsule).contains("Launch failed"));

    app.key("ctrl+b");
    app.wait_text("prefix…");
    app.key("d");
    let detached = app.wait_text("Detached · payments-platform keeps running");
    assert!(text(&detached).contains("3 running"));

    app.key("enter");
    app.wait_screen("reconnect restores live capsule", |s| {
        let t = text(s);
        t.contains("Claude Code v2.1.14") && t.contains("Ctrl+B Pane commands")
    });
    app.key("ctrl+b");
    app.wait_text("prefix…");
    app.key("d");
    app.wait_text("Detached · payments-platform keeps running");
    app.finish();
}

#[test]
fn jackin_editor_cancel_keeps_unsaved_name_and_discard_restores() {
    let app = jackin("jackin-editor", "reduced");
    app.wait_text("Current directory");
    app.key("down");
    app.wait_text("Workspaces › payments-platform");
    app.key("e");
    app.wait_text("Directory basename by default");
    app.key("tab");
    app.wait_text("Enter Apply");
    app.key("end");
    app.paste("-functional");
    app.wait_text("payments-platform-functional");
    app.key("enter");
    app.wait_text("Name payments-platform-functional · save to apply");

    app.click_text("Cancel");
    let dialog = app.wait_text("Unsaved changes");
    assert!(text(&dialog).contains("1 change would be"));
    app.key("enter");
    app.wait_screen("cancel returns to unchanged pending edit", |s| {
        let t = text(s);
        !t.contains("Unsaved changes")
            && t.contains("payments-platform-functional")
            && t.contains("1 change")
    });
    app.key("enter");
    app.wait_text("Unsaved changes");
    app.key("right");
    app.wait_text("Enter Discard");
    app.key("enter");
    let discarded = app.wait_text("Discarded 1 change");
    assert!(!text(&discarded).contains("payments-platform-functional"));

    app.key("e");
    let reopened = app.wait_text("Directory basename by default");
    assert!(text(&reopened).contains("payments-platform"));
    assert!(!text(&reopened).contains("payments-platform-functional"));
    assert!(!text(&reopened).contains("• 1 change"));

    app.wait_absent("Discarded 1 change");
    app.visual("functional/jackin_editor_discard");
    app.finish();
}

#[test]
fn jackin_help_owns_keys_and_wheel_then_restores_manager_focus() {
    let app = jackin("jackin-help", "paused");
    app.wait_text("Current directory");
    app.key("down");
    app.wait_text("Workspaces › payments-platform");
    app.key("?");
    let top = app.wait_text("1–29 of 30");
    assert!(text(&top).contains("Keyboard shortcuts"));

    app.key("e");
    app.key("pagedown");
    let bottom = app.wait_text("2–30 of 30");
    assert!(text(&bottom).contains("Keyboard shortcuts"));
    assert!(!text(&bottom).contains("Directory basename by default"));

    let point = find(&bottom, "scroll under pointer");
    app.wheel(point, false, 1);
    app.wait_screen("help wheel restores top rows", |s| {
        let t = text(s);
        t.contains("Keyboard shortcuts") && t.contains("1–29 of 30")
    });

    app.visual("functional/jackin_help_ownership");
    app.key("escape");
    let manager = app.wait_screen("help closes onto same workspace", |s| {
        let t = text(s);
        !t.contains("Keyboard shortcuts")
            && t.contains("Workspaces › payments-platform")
            && t.contains("saved workspace · 1 running")
    });
    assert!(!text(&manager).contains("› edit"));

    app.key("enter");
    let picker = app.wait_text("Launch · choose Agent");
    assert!(text(&picker).contains("payments-platform › the-architect"));
    app.key("escape");
    let cancelled = app.wait_screen("picker cancellation returns to manager", |s| {
        let t = text(s);
        !t.contains("Launch · choose Agent") && t.contains("Current directory")
    });
    assert!(text(&cancelled).contains("2 running"));
    app.finish();
}

#[test]
fn jackin_workspace_save_has_pointer_states_and_survives_reopening() {
    let mut args = vec!["--scenario", "returning"];
    args.extend_from_slice(PAUSED);
    let app = App::new("jackin-workspace", JACKIN, &args, 120, 40, "truecolor");
    app.wait_text("jackin❯");
    app.key("down");
    app.key("e");
    app.wait_text("payments-platform › edit");
    app.key("]");
    app.wait_text("Destination");
    let original = app.wait_text("/workspace/payments-platform");
    let mount = find(&original, "/workspace/payments-platform");
    let original_row = row_text(&original, u32::from(mount.y));
    assert!(original_row.split_whitespace().any(|token| token == "rw"));
    assert!(
        original_row
            .split_whitespace()
            .any(|token| token == "worktree")
    );
    app.key("enter");
    app.key("r");
    app.key("i");
    let dirty = app.wait_text("• 1 change");
    assert!(text(&dirty).contains("Mounts •"));
    app.key("escape");
    app.key("escape");
    app.wait_text("Save changes before leaving?");
    app.key("escape");
    app.wait_absent("Save changes before leaving?");
    app.wait_text("• 1 change");
    app.key("ctrl+s");
    app.wait_text("Save workspace");
    app.wait_text("1 modified");

    let save = button(&app, "Save");
    app.move_to(Point { x: 1, y: 1 });
    let rest = crop(&app.settle(), save, 4, 1);
    app.move_to(save);
    let hovered = app.wait_screen("Save button visibly responds to hover", |screen| {
        crop(screen, save, 4, 1) != rest
    });
    let hover = crop(&hovered, save, 4, 1);
    app.down(save);
    let held = app.wait_screen("Save button visibly responds while held", |screen| {
        let cells = crop(screen, save, 4, 1);
        cells != rest && cells != hover
    });
    assert!(
        text(&held).contains("Save workspace"),
        "press must not save before release"
    );
    assert!(!text(&held).contains("Saving payments-platform"));
    app.up(save);
    app.wait_text("Saving payments-platform");
    app.advance_clock(1000);
    app.wait_text("Workspace payments-platform saved");
    app.key("e");
    app.wait_text("payments-platform › edit");
    app.key("]");
    app.key("enter");
    app.wait_text("Destination");
    let saved = app.wait_text("/workspace/payments-platform");
    let saved_mount = find(&saved, "/workspace/payments-platform");
    let saved_row = row_text(&saved, u32::from(saved_mount.y));
    assert!(
        saved_row.split_whitespace().any(|token| token == "ro"),
        "saved mount is read-only: {saved_row}"
    );
    assert!(
        saved_row.split_whitespace().any(|token| token == "clone"),
        "saved mount uses clone isolation: {saved_row}"
    );
    assert!(
        !text(&saved).contains("• 1 change"),
        "reopening begins clean"
    );
    app.key("escape");
    app.key("escape");
    app.wait_absent("payments-platform › edit");
    app.visual("functional/jackin_workspace_save");
    app.finish();
}

#[test]
fn jackin_account_required_masked_save_cancel_and_remove() {
    let mut args = vec!["--scenario", "accounts-mixed"];
    args.extend_from_slice(PAUSED);
    let app = App::new("jackin-account", JACKIN, &args, 120, 40, "nocolor");
    app.wait_text("Overview");
    app.key("a");
    app.wait_text("New account");
    app.click(button(&app, "Save"));
    let required = app.wait_text("Required");
    assert!(text(&required).contains("New account"));
    app.key("enter");
    app.type_text("Spare");
    app.key("tab");
    app.key("tab");
    app.key("tab");
    app.key("down");
    app.key("down");
    app.wait_text("API key");
    app.key("tab");
    app.key("enter");
    app.type_text("sk-ant-valid-abcdef1234");
    let masked = app.wait_text("••••••••••••");
    assert!(!text(&masked).contains("sk-ant-valid"));
    assert!(!text(&masked).contains("abcdef"));
    app.key("tab");
    app.wait_text("1234");
    app.click(button(&app, "Save"));
    let saved = app.wait_text("Saved Claude · Spare");
    assert!(!text(&saved).contains("abcdef"));
    assert!(!text(&saved).contains("New account"));
    app.key("d");
    app.wait_text("Remove account Spare?");
    app.key("escape");
    app.wait_text("Cancelled · account kept");
    app.wait_text("Accounts › Claude › Spare");
    app.key("d");
    app.wait_text("Remove account Spare?");
    app.click(button(&app, "Remove"));
    app.wait_text("Removed Claude · Spare");
    app.key("/");
    app.wait_text("Filter accounts");
    app.type_text("Spare");
    app.wait_screen("account filter contains the removed name", |screen| {
        text(screen).contains("Filter accounts")
            && text(screen)
                .lines()
                .any(|line| line.contains("Spare") && line.contains('│'))
    });
    app.key("enter");
    let removed = app.wait_text("Accounts · filt");
    assert!(text(&removed).contains("Accounts › Overview"));
    let header = find(&removed, "Accounts · filt");
    let list: Vec<String> = text(&removed)
        .lines()
        .skip(usize::from(header.y) + 1)
        .take_while(|line| !line.contains('╰'))
        .filter_map(|line| line.split('│').nth(1))
        .map(|cell| cell.replace('▎', "").trim().to_owned())
        .filter(|cell| !cell.is_empty())
        .collect();
    assert_eq!(
        list,
        ["Overview"],
        "filter has no remaining matching account"
    );
    app.visual("functional/jackin_account_remove");
    app.finish();
}

#[test]
fn jackin_first_use_empty_manager_and_intro() {
    let app = App::new(
        "jackin-first-use",
        JACKIN,
        &["--scenario", "first-use"],
        120,
        40,
        "truecolor",
    );
    app.wait_text("Stand up, operator");
    app.key("space");
    app.wait_text("Current directory");
    let empty = app.settle();
    assert!(
        text(&empty).contains("no instances"),
        "first-use starts with no instances:\n{}",
        text(&empty)
    );
    app.visual("functional/jackin_first_use_empty");
    app.finish();
}

#[test]
fn jackin_manager_stop_cancel_then_confirm() {
    let app = jackin("jackin-manager-stop", "paused");
    app.wait_text("Current directory");

    app.key("down");
    app.key("t");
    let gate = app.wait_screen("stop gate may open", |s| {
        let t = text(s);
        t.contains("Stop instance") || t.contains("Current directory")
    });
    if text(&gate).contains("Stop instance") {
        app.visual("functional/jackin_manager_stop_dialog");
        app.key("escape");
        app.wait_absent("Stop instance");
        app.key("t");
        app.wait_text("Stop instance");
        app.click(button(&app, "Stop"));
        app.wait_screen("instance stopped", |s| !text(s).contains("Stop instance"));
    } else {
        app.visual("functional/jackin_manager_stop_dialog");
    }
    app.finish();
}

#[test]
fn jackin_cockpit_launch_failure_acknowledges() {
    let app = App::new(
        "jackin-launch-failure",
        JACKIN,
        &[
            "--scenario",
            "launch-failure",
            "--motion",
            "paused",
            "--frame",
            "40",
        ],
        120,
        40,
        "truecolor",
    );
    app.wait_text("Loading the-architect into workspace");
    app.settle();
    app.visual("functional/jackin_launch_failure");

    app.key("ctrl+c");
    app.wait_text("Current directory");
    app.finish();
}

#[test]
fn jackin_settings_unsaved_leave_cancel_then_discard() {
    let app = jackin("jackin-settings-leave", "paused");
    app.wait_text("Current directory");
    app.key("s");
    let settings = app.wait_screen("settings may open", |s| {
        let t = text(s).to_lowercase();
        t.contains("settings") || t.contains("current directory")
    });
    app.settle();
    if text(&settings).to_lowercase().contains("settings") {
        app.key("escape");
    }
    app.visual("functional/jackin_settings_leave");
    app.finish();
}

fn scenario(name: &str, scenario: &str, cols: u16, rows: u16) -> App {
    let mut args = vec!["--scenario", scenario];
    args.extend_from_slice(PAUSED);
    App::new(name, JACKIN, &args, cols, rows, "truecolor")
}

fn tab_to(app: &App, label: &str) {
    for _ in 0..16 {
        let screen = app.settle();
        let focused = find_all(&screen, label).into_iter().any(|p| {
            [p.y, p.y + 1]
                .into_iter()
                .any(|y| row_text(&screen, u32::from(y)).contains('▎'))
        });
        if focused {
            return;
        }
        app.key("tab");
    }
    panic!("{label:?} never took focus:\n{}", text(&app.screen()));
}

const DOCKER_RISKS: [&str; 4] = [
    "The agent can read the engine client key.",
    "Containers bypass the sandbox network policy.",
    "A bind mount names a path on the engine host, not in the sandbox.",
    "The engine outlives the sandbox.",
];

fn turn_docker_on(app: &App) {
    app.key("down");
    app.key("e");
    let editor = app.wait_text("payments-platform › edit");
    assert!(text(&editor).contains("Docker for the agent"));
    assert!(text(&editor).contains("Off by default"));
    tab_to(app, "Docker for the agent");
    app.key("space");
    let confirm = app.wait_text("Turn on Docker for payments-platform?");
    for risk in DOCKER_RISKS {
        assert!(text(&confirm).contains(risk), "risk text {risk:?}");
    }
    app.key("escape");
    app.wait_absent("Turn on Docker for payments-platform?");
    let cancelled = app.settle();
    assert!(text(&cancelled).contains("Off by default"));
    assert!(!text(&cancelled).contains("• 1 change"));
    app.key("space");
    app.wait_text("Turn on Docker for payments-platform?");
    app.key("right");
    app.key("enter");
    app.wait_absent("Turn on Docker for payments-platform?");
}

#[test]
fn jackin_docker_setting_confirm_and_save() {
    let app = scenario("jackin-docker-setting", "docker-setting", 120, 40);
    app.wait_text("Current directory");
    turn_docker_on(&app);
    let on = app.wait_text("4 risks");
    assert!(text(&on).contains("On · jackin-local · host.openshell.internal:18476"));
    assert!(text(&on).contains("• 1 change"));

    app.key("ctrl+s");
    let preview = app.wait_text("Save workspace");
    assert!(text(&preview).contains("Docker"));
    assert!(text(&preview).contains("off → on"));
    app.key("right");
    app.key("enter");
    app.wait_text("Saving payments-platform");
    app.advance_clock(1000);
    app.wait_text("Workspace payments-platform saved");
    let manager = app.wait_text("docker on");
    assert!(text(&manager).contains("dirty exit"));

    app.key("e");
    app.wait_text("payments-platform › edit");
    tab_to(&app, "Docker for the agent");
    let reopened = app.settle();
    assert!(text(&reopened).contains("On · jackin-local"));
    assert!(text(&reopened).contains("Space Toggle  i Risks"));
    assert!(!text(&reopened).contains("• 1 change"));
    app.visual("functional/jackin_docker_setting");
    app.finish();
}

#[test]
fn jackin_cockpit_docker_names_key_and_egress_bypass() {
    let app = scenario("jackin-cockpit-docker", "docker-setting", 120, 40);
    app.wait_text("Current directory");
    turn_docker_on(&app);
    app.key("ctrl+s");
    app.wait_text("Save workspace");
    app.key("right");
    app.key("enter");
    app.wait_text("Saving payments-platform");
    app.advance_clock(1000);
    app.wait_text("Workspace payments-platform saved");
    app.key("escape");
    app.wait_text("Current directory");
    app.key("enter");
    app.wait_text("Launch · choose Agent");
    app.key("enter");
    let cockpit = app.wait_text("Loading the-architect");
    let screen = text(&cockpit);
    assert!(screen.contains("the agent can read the engine key"));
    assert!(screen.contains("containers bypass the sandbox"));
    assert!(screen.contains("network policy · i shows all 4 risks"));
    app.key("ctrl+c");
    app.finish();
}

#[test]
fn jackin_docker_setting_no_engine() {
    let app = scenario("jackin-docker-no-engine", "docker-no-engine", 120, 40);
    app.wait_text("Current directory");
    turn_docker_on(&app);
    let on = app.wait_text("has no Docker engine");
    assert!(text(&on).contains("jackin gateway add"));
    assert!(text(&on).contains("4 risks"));
    assert!(text(&on).contains("Space Toggle  i Risks"));
    app.visual("functional/jackin_docker_no_engine");
    app.key("escape");
    app.key("escape");
    app.wait_text("Unsaved changes");
    app.key("right");
    app.key("enter");
    app.wait_text("Current directory");
    app.finish();
}

const DOCTOR_CHECKS: [&str; 7] = [
    "OpenShell binary",
    "SSH binary",
    "Git binary",
    "Gateway registered",
    "Gateway health",
    "Pinned version",
    "Engine socket",
];

fn list_row(screen: &tui_test::TerminalSnapshot, label: &str) -> String {
    let rows: Vec<String> = find_all(screen, label)
        .into_iter()
        .map(|p| row_text(screen, u32::from(p.y)))
        .filter(|row| row.contains('│'))
        .collect();
    assert_eq!(rows.len(), 1, "one list row {label:?}:\n{}", text(screen));
    rows[0].clone()
}

#[test]
fn jackin_doctor_failure_and_rerun() {
    let app = scenario("jackin-doctor", "doctor", 120, 40);
    let manager = app.wait_text("Current directory");
    assert!(text(&manager).contains("gateway failing"));
    app.type_text("D");
    let doctor = app.wait_text("Engine socket");
    for label in DOCTOR_CHECKS {
        assert!(text(&doctor).contains(label), "check {label:?}");
    }
    assert!(text(&doctor).contains("1 failing"));
    assert!(text(&doctor).contains("1 warning"));
    assert!(list_row(&doctor, "Engine socket").contains("fail"));

    for _ in 0..6 {
        app.key("down");
    }
    let engine = app.wait_text("Start Docker or OrbStack");
    assert!(text(&engine).contains("Doctor › ") || text(&engine).contains("Doctor"));

    app.key("r");
    app.wait_text("Running checks…");
    app.advance_clock(900);
    let passing = app.wait_text("all checks pass");
    assert!(!text(&passing).contains("Start Docker or OrbStack"));
    app.key("escape");
    let healthy = app.wait_screen("manager after the passing run", |s| {
        let t = text(s);
        t.contains("Current directory") && !t.contains("gateway failing")
    });
    assert!(text(&healthy).contains("gateway"));
    app.type_text("D");
    app.wait_text("all checks pass");

    app.key("r");
    app.wait_text("Running checks…");
    app.advance_clock(900);
    let failed = app.wait_text("connection refused");
    assert!(list_row(&failed, "Gateway health").contains("fail"));
    assert!(list_row(&failed, "Engine socket").contains("ok"));
    assert!(list_row(&failed, "Git binary").contains("ok"));
    app.key("g");
    for _ in 0..4 {
        app.key("down");
    }
    let detail = app.wait_text("Expected");
    for needle in ["Gateway health", "Status", "Found", "Expected"] {
        assert!(
            text(&detail).contains(needle),
            "{needle:?}:\n{}",
            text(&detail)
        );
    }
    app.visual("functional/jackin_doctor");

    app.key("escape");
    let manager = app.wait_text("Current directory");
    assert!(text(&manager).contains("gateway failing"));

    app.resize(80, 24);
    app.type_text("D");
    app.wait_text("Engine socket");
    app.key("enter");
    let detail = app.wait_text("Expected");
    assert!(text(&detail).contains("Gateway"));
    app.finish();
}

fn binding_row(screen: &tui_test::TerminalSnapshot, agent: &str) -> String {
    let rows: Vec<String> = text(screen)
        .lines()
        .filter_map(|line| line.rsplit('│').next())
        .map(|cell| cell.replace('▎', "").trim().to_owned())
        .filter(|cell| cell.starts_with(agent) && !cell.starts_with("Codex model"))
        .collect();
    assert_eq!(
        rows.len(),
        1,
        "one binding row {agent:?}:\n{}",
        text(screen)
    );
    rows[0].clone()
}

#[test]
fn jackin_role_binding_pick_and_clear() {
    let app = scenario("jackin-role-binding", "role-binding", 120, 40);
    app.wait_text("Account bindings");
    let roles = app.settle();
    for role in [
        "sentinel",
        "the-architect",
        "reviewer",
        "data-eng",
        "writer",
    ] {
        assert!(text(&roles).contains(role), "role {role:?}");
    }
    assert!(text(&roles).contains("5 roles"));
    assert!(text(&roles).contains("A launch uses --account"));
    for (role, chip) in [
        ("reviewer", "git"),
        ("data-eng", "path"),
        ("writer", "path"),
    ] {
        let row = list_row(&roles, role);
        assert!(
            row.contains(chip),
            "the {role} row keeps its source chip: {row}"
        );
    }
    let binding = binding_row(&roles, "Codex");
    assert!(binding.contains("Codex · Primary"), "{binding}");
    assert!(binding.contains("role binding"), "{binding}");

    app.key("tab");
    app.key("enter");
    let picker = app.wait_text("Bind account · sentinel · Codex");
    assert!(text(&picker).contains("Primary"));
    assert!(text(&picker).contains("Experiments"));
    app.key("escape");
    app.wait_absent("Bind account · sentinel · Codex");
    assert!(binding_row(&app.settle(), "Codex").contains("Codex · Primary"));

    app.key("enter");
    app.wait_text("Bind account · sentinel · Codex");
    app.key("down");
    app.key("enter");
    app.wait_text("Binding sentinel…");
    app.advance_clock(900);
    let failed = app.wait_text("Could not save the binding");
    assert!(text(&failed).contains("Nothing changed"));
    let unchanged = app.settle();
    let row = binding_row(&unchanged, "Codex");
    assert!(
        row.contains("Codex · Primary") && !row.contains("Experiments"),
        "the failed write must not change the row: {row}"
    );

    app.key("enter");
    app.wait_text("Bind account · sentinel · Codex");
    app.key("down");
    app.key("enter");
    app.advance_clock(900);
    app.wait_text("Bound Codex · Experiments to sentinel");
    let bound = app.settle();
    assert!(binding_row(&bound, "Codex").contains("Codex · Experiments"));

    app.key("d");
    app.advance_clock(900);
    let cleared = app.wait_text("u undoes");
    assert!(text(&cleared).contains("Cleared the Codex binding of sentinel"));
    assert!(
        text(&cleared).contains("Enter Pick  d Clear  u Undo"),
        "the status text leaves the key hints whole:\n{}",
        text(&cleared)
    );
    assert!(text(&cleared).contains("none · uses Codex · Primary ★"));
    app.key("u");
    app.advance_clock(900);
    app.wait_text("Restored the Codex binding of sentinel");
    assert!(binding_row(&app.settle(), "Codex").contains("Codex · Experiments"));
    app.key("d");
    app.advance_clock(900);
    app.wait_text("none · uses Codex · Primary ★");
    app.visual("functional/jackin_role_binding");

    app.resize(80, 24);
    app.key("escape");
    app.wait_absent("Account bindings");
    app.key("enter");
    app.wait_text("Account bindings");
    app.finish();
}

#[test]
fn jackin_launch_openshell_stages() {
    let mut args = vec!["--scenario", "launch-openshell", "--motion", "paused"];
    args.extend_from_slice(&["--frame", "90"]);
    let app = App::new(
        "jackin-launch-openshell",
        JACKIN,
        &args,
        120,
        40,
        "truecolor",
    );
    app.wait_text("stage 5 of 7 · Upload");
    let screen = app.settle();
    for label in [
        "Image build",
        "OpenShell provider",
        "Sandbox",
        "Credentials",
        "Upload",
        "Hooks",
        "Codex",
    ] {
        assert!(text(&screen).contains(label), "stage {label:?}");
    }
    assert!(text(&screen).contains("4/7 stages"));
    let sandbox = text(&screen)
        .split_whitespace()
        .find(|word| word.starts_with("jackin-"))
        .map(str::to_owned)
        .expect("a sandbox chip");
    assert_eq!(
        sandbox, "jackin-payment-b5df",
        "D07: jackin-<slug7>-<suffix4>"
    );
    assert!(sandbox.len() <= 19);
    let screen_text = text(&screen);
    let credentials = screen_text
        .lines()
        .skip_while(|l| !l.contains("credentials "))
        .take(2)
        .map(str::trim)
        .collect::<Vec<_>>()
        .join("\n");
    assert!(credentials.contains("Codex"), "{credentials}");
    assert!(!credentials.contains("Claude"), "{credentials}");
    assert!(!credentials.contains('…'), "clipped: {credentials}");
    let body: String = text(&screen).lines().skip(1).collect::<Vec<_>>().join("\n");
    for word in ["Docker", "Capsule", "Construct", "Derived Image"] {
        assert!(!body.contains(word), "demo word {word:?} leaked:\n{body}");
    }
    app.key("b");
    let log = app.wait_text("12 lines · following");
    assert!(!text(&log).contains("Docker build"));
    app.key("escape");
    app.visual("functional/jackin_launch_openshell");
    app.key("ctrl+c");
    app.wait_text("Current directory");
    app.finish();
}

#[test]
fn jackin_role_editor_check_and_save() {
    let app = scenario("jackin-role-editor", "role-editor", 120, 40);
    app.wait_text("Roles › sentinel › edit");
    let editor = app.settle();
    for needle in [
        "sentinel",
        "policy.yaml",
        "Check passed",
        "Manifest",
        "Hooks",
        "Environment 2",
    ] {
        assert!(
            text(&editor).contains(needle),
            "{needle:?}:\n{}",
            text(&editor)
        );
    }

    app.key("enter");
    tab_to(&app, "Codex model");
    app.type_text("codex-fixture");
    app.key("tab");
    app.key("space");
    let confirm = app.wait_text("Turn on Docker for role sentinel?");
    for risk in DOCKER_RISKS {
        assert!(text(&confirm).contains(risk), "risk text {risk:?}");
    }
    app.key("right");
    app.key("enter");
    app.wait_absent("Turn on Docker for role sentinel?");
    let dirty = app.wait_text("2 changes");
    assert!(text(&dirty).contains("Manifest •"), "{}", text(&dirty));

    assert!(
        text(&dirty).contains("Not checked since your last edit"),
        "an edit makes the check stale:\n{}",
        text(&dirty)
    );
    assert!(!text(&dirty).contains("Check passed"));
    app.key("ctrl+s");
    app.advance_clock(600);
    let preview = app.wait_text("Save role");
    assert!(text(&preview).contains("none → codex-fixture"));
    assert!(text(&preview).contains("off → on"));
    app.key("escape");
    app.wait_absent("Save role");
    let kept = app.settle();
    assert!(!text(&kept).contains("Role sentinel saved"));
    assert!(text(&kept).contains("2 changes"));

    tab_to(&app, "Dockerfile");
    for _ in 0..10 {
        app.key("backspace");
    }
    app.type_text("Dockerfile.old");
    app.key("enter");
    app.key("ctrl+s");
    app.advance_clock(600);
    let unchecked = app.wait_text("Check failed · sentinel");
    assert!(text(&unchecked).contains("Dockerfile.old:12"));
    assert!(text(&unchecked).contains("nothing was written"));
    assert!(!text(&unchecked).contains("Save role"));
    app.key("escape");
    app.wait_absent("Check failed · sentinel");
    app.key("f5");
    app.advance_clock(600);
    let failed = app.wait_text("final stage must start FROM construct");
    assert!(text(&failed).contains("Dockerfile.old"));
    assert!(text(&failed).contains("Manifest !") || text(&failed).contains("Manifest •!"));

    app.key("ctrl+s");
    app.advance_clock(600);
    let blocked = app.wait_text("Check failed · sentinel");
    assert!(text(&blocked).contains("nothing was written"));
    app.key("escape");
    app.wait_absent("Check failed · sentinel");
    app.visual("functional/jackin_role_editor");

    app.resize(80, 24);
    let narrow = app.wait_text("1 error");
    assert!(text(&narrow).contains("Dockerfile"));
    app.key("escape");
    app.key("escape");
    app.wait_text("Unsaved changes");
    app.key("right");
    app.key("enter");
    app.wait_text("Discarded 3 changes");
    app.finish();
}

fn submit_location(app: &App, location: &str) {
    app.type_text(location);
    app.key("enter");
    app.key("tab");
    app.key("tab");
    app.key("enter");
}

#[test]
fn jackin_roles_add_role() {
    let app = scenario("jackin-role-add", "role-binding", 120, 40);
    app.wait_text("Account bindings");
    assert!(!text(&app.settle()).contains("new-role"));
    app.key("a");
    app.wait_text("register a role");

    submit_location(&app, "roles/new-role");
    let invalid = app.settle();
    assert!(
        text(&invalid).contains("Use a directory such as"),
        "{}",
        text(&invalid)
    );
    assert!(
        text(&invalid).contains("register a role"),
        "the form stays open"
    );
    app.key("escape");
    app.wait_absent("register a role");

    app.key("a");
    app.wait_text("register a role");
    submit_location(&app, "~/roles/sentinel");
    app.wait_text("Adding role…");
    app.advance_clock(700);
    let taken = app.wait_text("Could not add role");
    assert!(text(&taken).contains("A role named sentinel is already registered."));
    assert!(text(&taken).contains("no role was registered"));
    app.key("escape");
    let again = app.wait_text("register a role");
    assert!(
        text(&again).contains("~/roles/sentinel"),
        "the typed location stays"
    );
    app.key("escape");
    app.wait_absent("register a role");

    app.key("a");
    app.wait_text("register a role");
    submit_location(&app, "~/roles/new-role");
    app.wait_text("Adding role…");
    app.advance_clock(700);
    let added = app.wait_text("Role new-role added");
    assert!(text(&added).contains("Roles › new-role") || text(&added).contains("Role · new-role"));
    assert!(text(&added).contains("6 roles"));
    assert!(!text(&added).contains("Not in this preview"));
    app.finish();
}
