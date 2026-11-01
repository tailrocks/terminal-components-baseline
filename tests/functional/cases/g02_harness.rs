use std::panic::AssertUnwindSafe;
use std::path::PathBuf;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use tui_test::profile::Rgb;
use tui_test::{
    SnapshotCell, SnapshotColor, SnapshotCursor, SnapshotCursorShape, SnapshotMetadata,
    SnapshotParts, SnapshotScope, SnapshotUnderline, TerminalSnapshot,
};
use tuiscotti_latest::check::{CheckOutcome, MismatchReason};

use crate::support::{
    App, Launch, Point, SHOWCASE, file_sha256, find_all, find_in, text, try_launch,
};

const CHANGING: &str = r#"i=0; while :; do printf '%s\n' "$i"; i=$((i+1)); done"#;
const HISTORY: &str = r#"printf 'HISTORY=%s\n' "$HOLLA_NO_HISTORY""#;
const CLOCK_PASTE: &str = "junie-tui:advance-clock:";

fn shell(name: &str, script: &str, clock: bool, history: bool) -> App {
    let mut launch = Launch::new(name, "/bin/sh");
    launch.args = vec!["-c".to_string(), script.to_string()];
    launch.cols = 80;
    launch.rows = 24;
    launch.clock = clock;
    launch.history = history;
    try_launch(launch).expect("launch shell fixture")
}

fn panic_text(payload: Box<dyn std::any::Any + Send>) -> String {
    if let Some(message) = payload.downcast_ref::<&str>() {
        (*message).to_string()
    } else if let Some(message) = payload.downcast_ref::<String>() {
        message.clone()
    } else {
        "non-string panic".to_string()
    }
}

fn production_baseline() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/functional/baselines/functional/showcase_popover_resize.tui.json")
}

fn temp_root(name: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock")
        .as_nanos();
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("target/functional")
        .join(format!("g02-store-{name}-{}-{nanos}", std::process::id()));
    std::fs::create_dir_all(&path).expect("temp baseline root");
    path
}

fn showcase(name: &str) -> App {
    let mut launch = Launch::new(name, SHOWCASE);
    launch.args = vec![
        "--page".to_string(),
        "overview".to_string(),
        "--frame".to_string(),
        "40".to_string(),
    ];
    launch.motion = Some("paused".to_string());
    launch.color = Some("truecolor".to_string());
    launch.cols = 120;
    launch.rows = 40;
    try_launch(launch).expect("launch showcase")
}

fn assert_candidate_logged(app: &App, test_id: &str) {
    let actions =
        std::fs::read_to_string(app.evidence_dir().join("actions.txt")).expect("action trace");
    let line = actions
        .lines()
        .find(|line| line.contains(&format!("visual {test_id} candidate ")))
        .unwrap_or_else(|| panic!("missing candidate log\n{actions}"));
    let sha = line.rsplit(' ').next().expect("candidate sha");
    assert_eq!(sha.len(), 64, "{line}");
    assert!(sha.chars().all(|c| c.is_ascii_hexdigit()), "{line}");
    assert!(
        actions.lines().any(|line| {
            let Some(fp) = line.rsplit(' ').next() else {
                return false;
            };
            line.contains("checkpoint ")
                && fp.len() == 16
                && fp.chars().all(|c| c.is_ascii_hexdigit())
        }),
        "{actions}"
    );
}

fn locator_screen() -> TerminalSnapshot {
    let cell = |text: &str, width: u16| {
        SnapshotCell::new(
            text.to_string(),
            width,
            SnapshotColor::Default,
            SnapshotColor::Default,
            tui_test::terminal::cell::Attrs::empty(),
            SnapshotUnderline::None,
            SnapshotColor::Default,
            None,
            None,
        )
        .expect("cell")
    };
    let label = ["L", "a", "b", "e", "l"].map(|glyph| cell(glyph, 1));
    let mut row0 = label.to_vec();
    row0.extend([cell(" ", 1), cell(" ", 1), cell(" ", 1)]);
    let mut row1 = vec![cell("你", 2), cell("", 0)];
    row1.extend(label);
    row1.push(cell(" ", 1));
    let cells = [row0, row1].into_iter().flatten().collect();
    TerminalSnapshot::try_new(SnapshotParts {
        cols: 8,
        rows: 2,
        scope: SnapshotScope::Viewport,
        viewport_origin: 0,
        cells,
        palette: [Rgb::new(0, 0, 0); 256],
        default_foreground: Rgb::new(255, 255, 255),
        default_background: Rgb::new(0, 0, 0),
        default_cursor: Rgb::new(255, 255, 255),
        cursor: SnapshotCursor::new(
            0,
            0,
            true,
            true,
            SnapshotCursorShape::Block,
            Rgb::new(255, 255, 255),
            None,
        ),
        title: None,
        metadata: SnapshotMetadata::default(),
    })
    .expect("locator snapshot")
}

#[test]
fn harness_changing_screen_fails_stability_deadline() {
    let app = shell("harness-changing", CHANGING, false, false);
    let evidence = app.evidence_dir().to_path_buf();
    let result = std::panic::catch_unwind(AssertUnwindSafe(|| {
        let _ = app.stable_screen();
    }));
    let message =
        panic_text(result.expect_err("stability deadline: stable_screen returned a frame"));
    assert!(message.starts_with("stability deadline"), "{message}");
    assert!(
        message.contains(&evidence.display().to_string()),
        "{message}"
    );
    let actions = std::fs::read_to_string(evidence.join("actions.txt")).expect("action trace");
    let last = std::fs::read_to_string(evidence.join("last.txt")).expect("last screen");
    assert!(actions.contains("spawn"), "{actions}");
    assert!(!last.is_empty(), "last screen text is empty");
}

#[test]
fn harness_dead_child_fails_wait() {
    let app = shell(
        "harness-dead",
        r#"printf 'dead-child-marker\n'; exit 0"#,
        false,
        false,
    );
    let deadline = Instant::now() + Duration::from_secs(2);
    loop {
        let visible = text(&app.screen());
        if visible.contains("dead-child-marker") && !app.child_running() {
            break;
        }
        assert!(Instant::now() < deadline, "child did not exit:\n{visible}");
        std::thread::sleep(Duration::from_millis(20));
    }
    let waited = std::panic::catch_unwind(AssertUnwindSafe(|| {
        app.wait_screen("dead child", |screen| {
            text(screen).contains("dead-child-marker")
        });
    }));
    let waited_message = panic_text(waited.expect_err("dead child satisfied a wait"));
    assert!(waited_message.contains("exited"), "{waited_message}");
    let stable = std::panic::catch_unwind(AssertUnwindSafe(|| {
        let _ = app.stable_screen();
    }));
    let stable_message = panic_text(stable.expect_err("dead child satisfied stability"));
    assert!(stable_message.contains("exited"), "{stable_message}");
    assert!(
        !stable_message.starts_with("stability deadline"),
        "{stable_message}"
    );
}

#[test]
fn harness_missing_baseline_fails() {
    let before = file_sha256(&production_baseline());
    let app = showcase("harness-missing-baseline");
    app.wait_text("120×40");
    let root = temp_root("missing");
    let outcome = app.check_visual(&root, "functional/harness_missing_screen");
    match outcome {
        CheckOutcome::Failure(failure) => {
            assert_eq!(failure.reason(), MismatchReason::Missing);
        }
        CheckOutcome::Pass(success) => panic!("missing baseline passed: {success:?}"),
    }
    assert_candidate_logged(&app, "functional/harness_missing_screen");
    assert_eq!(file_sha256(&production_baseline()), before);
    app.finish();
}

#[test]
fn harness_changed_baseline_fails() {
    let before = file_sha256(&production_baseline());
    let app = showcase("harness-changed-baseline");
    app.wait_text("120×40");
    let root = temp_root("changed");
    let raw = std::fs::read_to_string(production_baseline()).expect("read production baseline");
    assert_eq!(raw.matches("functional/showcase_popover_resize").count(), 1);
    let rewritten = raw.replacen(
        "functional/showcase_popover_resize",
        "functional/harness_changed_screen",
        1,
    );
    let dest = root.join("functional");
    std::fs::create_dir_all(&dest).expect("temp baseline dir");
    std::fs::write(dest.join("harness_changed_screen.tui.json"), rewritten)
        .expect("write changed baseline");
    let outcome = app.check_visual(&root, "functional/harness_changed_screen");
    match outcome {
        CheckOutcome::Failure(failure) => {
            assert_ne!(failure.reason(), MismatchReason::Missing);
            assert!(
                matches!(
                    failure.reason(),
                    MismatchReason::State | MismatchReason::Dimensions
                ),
                "{:?}",
                failure.reason()
            );
        }
        CheckOutcome::Pass(success) => panic!("changed baseline passed: {success:?}"),
    }
    assert_candidate_logged(&app, "functional/harness_changed_screen");
    assert_eq!(file_sha256(&production_baseline()), before);
    app.finish();
}

#[test]
fn harness_killed_child_is_not_a_clean_exit() {
    let app = shell("harness-kill", "cat", false, false);
    let evidence = app.evidence_dir().to_path_buf();
    app.kill_child();
    let result = std::panic::catch_unwind(AssertUnwindSafe(|| {
        app.finish();
    }));
    let message = panic_text(result.expect_err("killed child must not finish cleanly"));
    assert!(message.contains("exited"), "{message}");
    assert!(!message.contains("clean exit"), "{message}");
    let actions = std::fs::read_to_string(evidence.join("actions.txt")).expect("action trace");
    assert!(actions.contains("kill child"), "{actions}");
    assert!(!actions.contains("clean exit"), "{actions}");
}

#[test]
fn harness_clock_paste_stays_out_of_user_text_without_capability() {
    let app = shell("harness-clock-off", "cat", false, false);
    app.advance_clock(250);
    let deadline = Instant::now() + Duration::from_millis(800);
    loop {
        let visible = text(&app.screen());
        assert!(
            !visible.contains(CLOCK_PASTE),
            "clock paste reached the terminal without the capability:\n{visible}"
        );
        if Instant::now() >= deadline {
            break;
        }
        std::thread::sleep(Duration::from_millis(25));
    }
    let actions =
        std::fs::read_to_string(app.evidence_dir().join("actions.txt")).expect("action trace");
    assert!(actions.contains("clock refused"), "{actions}");
}

#[test]
fn harness_clock_paste_reaches_a_capable_session() {
    let app = shell("harness-clock-on", "cat", true, false);
    let record: serde_json::Value = serde_json::from_slice(
        &std::fs::read(app.evidence_dir().join("binary.json")).expect("binary.json"),
    )
    .expect("binary record");
    let path = PathBuf::from(record["path"].as_str().expect("binary path"));
    assert!(path.is_absolute());
    let sha = file_sha256(&path);
    assert_eq!(record["sha256"].as_str(), Some(sha.as_str()));
    app.advance_clock(250);
    let screen = app.wait_text(CLOCK_PASTE);
    assert!(text(&screen).contains(CLOCK_PASTE));
}

#[test]
fn harness_history_fixture_omits_holla_no_history() {
    let off = shell("harness-history-off", HISTORY, false, false);
    let disabled = wait_for_output(&off, "HISTORY=");
    off.expect_exited(0);
    assert!(disabled.contains("HISTORY=1"), "{disabled}");

    let on = shell("harness-history-on", HISTORY, false, true);
    let enabled = wait_for_output(&on, "HISTORY=");
    on.expect_exited(0);
    assert!(enabled.contains("HISTORY="), "{enabled}");
    assert!(!enabled.contains("HISTORY=1"), "{enabled}");
}

fn wait_for_output(app: &App, needle: &str) -> String {
    let deadline = Instant::now() + Duration::from_secs(2);
    loop {
        let visible = text(&app.screen());
        // The process watcher publishes the exit after the output is on screen.
        if visible.contains(needle) && !app.child_running() {
            return visible;
        }
        assert!(Instant::now() < deadline, "missing {needle}:\n{visible}");
        std::thread::sleep(Duration::from_millis(20));
    }
}

#[test]
fn harness_scoped_locator_finds_the_label_inside_the_region() {
    let screen = locator_screen();
    let all = find_all(&screen, "Label");
    assert_eq!(all.len(), 2, "{all:?}");
    let hit = find_in(&screen, 2, 1, 6, 1, "Label");
    assert_eq!(hit, Point { x: 2, y: 1 });
}

#[test]
fn harness_wide_glyph_locator_uses_the_leading_cell() {
    let screen = locator_screen();
    let hit = find_in(&screen, 0, 1, 1, 1, "你");
    assert_eq!(hit, Point { x: 0, y: 1 });
    assert_eq!(screen.cell_at(0, 1).expect("leading").width(), 2);
    assert_eq!(screen.cell_at(1, 1).expect("continuation").text(), "");
    assert!(
        screen
            .cell_at(1, 1)
            .expect("continuation")
            .is_continuation()
    );
}

#[test]
fn harness_startup_failure_is_saved() {
    let mut launch = Launch::new("harness-startup-failure", "/no/such/junie-tui-g02-binary");
    launch.cols = 20;
    launch.rows = 5;
    let Err(error) = try_launch(launch) else {
        panic!("missing binary must fail");
    };
    let path = error.evidence.join("spawn-error.txt");
    let body = std::fs::read_to_string(&path).expect("spawn-error.txt");
    assert!(path.is_file());
    assert!(body.contains(&error.message), "{body}");
    assert!(!body.trim().is_empty());
}
