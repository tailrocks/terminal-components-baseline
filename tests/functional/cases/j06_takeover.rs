//! Jackin attach takeover, driven through the real binary.
//!
//! Tuiscotti test ids are `[a-z0-9_-]` segments, so the baseline directory is
//! `pre_refactor/j06_takeover/`. The shots are the takeover overlay on
//! `jk-7f3a`, before Enter and before Esc. They do not nudge. `Launch::new`
//! leaves the clock off. `App::new` would enable it for jackin.
//!
//! Hard-cases emits one `Takeover` when sim time reaches 45_000 ms
//! (`World::tick` in `src/bin/jackin/sim/world.rs`). Reduced motion ticks the
//! manager and the capsule by 80 ms and advances the sim clock by that same
//! interval (`Route::tick_ms` in `src/bin/jackin/app.rs`). The functional
//! profile ends a test after 90s, so the stale session and the overlay
//! session run together. Each is still a fresh process.

use std::path::PathBuf;
use std::time::{Duration, Instant};

use tui_test::TerminalSnapshot;
use tuiscotti_latest::check::CheckOutcome;

use crate::support::{App, JACKIN, Launch, row_text, text, try_launch};

const PAYMENTS: &str = "payments-platform";
const INFRA: &str = "infra-control-plane";
const TARGET: &str = "7f3a";
const OTHER: &str = "9b02";
const IDENTITY: &str = "payments-platform › the-architect";
const INFRA_IDENTITY: &str = "infra-control-plane › sre";
const TAKEN: &str = "Attach taken over";
const AGO: &str = "tty004 · MacBook attached to payments-platform 3 s ago.";
const KEEPS: &str = "The instance, tabs and panes keep running.";
const RECONNECTED: &str = "Reconnected · the other client was displaced";
const DETACHED: &str = "Detached · payments-platform keeps running";
const DETAIL: &str = "7f3a · payments-platform";
const PAGE: &str = "Workspaces";
const CURRENT: &str = "Current directory";
const OUTRO: &str = "You were in the Construct";
const WIDE: &str = "J06.takeover.takeover.120x40";
const NARROW: &str = "J06.takeover.takeover.narrow";

/// Wall time after the first frame. The sim clock is still near zero there,
/// and 50s covers the 45_000 ms deadline with the 80 ms tick.
const SIM_HOLD: Duration = Duration::from_secs(50);

/// `StatusLine::DEFAULT_TTL_MS` is 5_000. The overlay copy does not move.
/// Waiting this long lets the footer status expire before a shot.
const STATUS_CLEAR: Duration = Duration::from_secs(6);

fn visual_id(checkpoint: &str) -> String {
    format!(
        "pre_refactor/j06_takeover/{}",
        checkpoint.replace('.', "_").to_ascii_lowercase()
    )
}

fn open(name: &str) -> App {
    let mut launch = Launch::new(name, JACKIN);
    launch.args = vec!["--scenario".to_string(), "hard-cases".to_string()];
    launch.cols = 120;
    launch.rows = 40;
    launch.color = Some("truecolor".to_string());
    launch.motion = Some("reduced".to_string());
    // `Launch::new` leaves the clock off. `App::new` would enable it for jackin.
    launch.clock = false;
    try_launch(launch).unwrap_or_else(|error| panic!("{error}"))
}

fn focused_line(screen: &TerminalSnapshot) -> String {
    (0..screen.rows())
        .map(|row| row_text(screen, row))
        .find(|line| line.contains('▎'))
        .unwrap_or_default()
}

/// The tree side of the focused row. The detail pane shares that terminal row.
fn tree_focus(screen: &TerminalSnapshot) -> String {
    focused_line(screen).chars().take(32).collect()
}

fn require(screen: &TerminalSnapshot, needle: &str) {
    assert!(
        text(screen).contains(needle),
        "missing {needle}\n{}",
        text(screen)
    );
}

fn refuse(screen: &TerminalSnapshot, needle: &str) {
    assert!(
        !text(screen).contains(needle),
        "{needle} present\n{}",
        text(screen)
    );
}

fn manager_up(screen: &TerminalSnapshot) -> bool {
    let visible = text(screen);
    screen.cols() == 120
        && screen.rows() == 40
        && visible.contains(PAGE)
        && visible.contains(CURRENT)
        && visible.contains(PAYMENTS)
        && visible.contains(INFRA)
        && !visible.contains(OUTRO)
        && !visible.contains(TAKEN)
}

fn overlay_screen(screen: &TerminalSnapshot, cols: u32, rows: u32) -> bool {
    let visible = text(screen);
    screen.cols() == cols
        && screen.rows() == rows
        && visible.contains(TAKEN)
        && visible.contains(AGO)
        && visible.contains(KEEPS)
        && visible.contains("Reconnect")
        && !visible.contains(PAGE)
        && !visible.contains(OUTRO)
}

fn wide_overlay(screen: &TerminalSnapshot) -> bool {
    overlay_screen(screen, 120, 40)
}

fn narrow_overlay(screen: &TerminalSnapshot) -> bool {
    overlay_screen(screen, 80, 24)
}

fn wait_for(
    app: &App,
    reason: &str,
    budget: Duration,
    mut also: impl FnMut(),
    accept: impl Fn(&TerminalSnapshot) -> bool,
) -> TerminalSnapshot {
    let deadline = Instant::now() + budget;
    loop {
        also();
        let screen = app.screen();
        if accept(&screen) {
            return screen;
        }
        assert!(
            Instant::now() < deadline && app.child_running(),
            "{reason} not seen\n{}",
            text(&screen)
        );
        std::thread::sleep(Duration::from_millis(20));
    }
}

fn wait_settled(
    app: &App,
    reason: &str,
    hold: Duration,
    budget: Duration,
    accept: impl Fn(&TerminalSnapshot) -> bool,
) -> TerminalSnapshot {
    let deadline = Instant::now() + budget;
    let mut last: Option<TerminalSnapshot> = None;
    let mut since: Option<Instant> = None;
    loop {
        let screen = app.screen();
        if accept(&screen) {
            let same = last.as_ref().is_some_and(|prev| prev == &screen);
            if same {
                if since.expect("stability start").elapsed() >= hold {
                    return screen;
                }
            } else {
                last = Some(screen);
                since = Some(Instant::now());
            }
        } else {
            last = None;
            since = None;
        }
        assert!(
            Instant::now() < deadline && app.child_running(),
            "{reason} frame not seen\n{}",
            text(&app.screen())
        );
        std::thread::sleep(Duration::from_millis(20));
    }
}

fn select_workspace(app: &App, workspace: &str) {
    let first = app.wait_screen("manager", manager_up);
    let line = tree_focus(&first);
    assert!(
        line.contains(CURRENT),
        "initial selection is not Current directory\n{line}\n{}",
        text(&first)
    );
    app.key("down");
    app.wait_screen("payments-platform row", |screen| {
        let focus = tree_focus(screen);
        focus.contains(PAYMENTS) && !focus.contains(TARGET) && !focus.contains(OTHER)
    });
    if workspace == INFRA {
        app.key("down");
        app.wait_screen("infra-control-plane row", |screen| {
            let focus = tree_focus(screen);
            focus.contains(INFRA) && !focus.contains(TARGET) && !focus.contains(OTHER)
        });
    }
    let focus = tree_focus(&app.screen());
    assert!(
        focus.contains(workspace) && !focus.contains('▾'),
        "workspace {workspace} is not collapsed\n{focus}\n{}",
        text(&app.screen())
    );
    app.key("right");
    app.wait_screen("expanded workspace", |screen| {
        let focus = tree_focus(screen);
        focus.contains('▾') && focus.contains(workspace)
    });
}

fn select_instance(app: &App, workspace: &str, id: &str) {
    select_workspace(app, workspace);
    if tree_focus(&app.screen()).contains(id) {
        return;
    }
    app.key("down");
    app.wait_screen("instance row", |screen| tree_focus(screen).contains(id));
    let focus = tree_focus(&app.screen());
    assert!(
        focus.contains(id) && !focus.contains(if id == TARGET { OTHER } else { TARGET }),
        "focused row is not {id}\n{focus}\n{}",
        text(&app.screen())
    );
}

fn attach_payments(app: &App) -> Instant {
    let started = app.wait_screen("manager", manager_up);
    let _ = started;
    let clock = Instant::now();
    select_instance(app, PAYMENTS, TARGET);
    app.key("enter");
    app.wait_screen("attached payments-platform", |screen| {
        let visible = text(screen);
        visible.contains(IDENTITY)
            && visible.contains("Attached to payments-platform")
            && !visible.contains(TAKEN)
            && !visible.contains(PAGE)
            && !visible.contains(INFRA)
    });
    clock
}

fn attach_infra(app: &App) -> Instant {
    let _ = app.wait_screen("manager", manager_up);
    let clock = Instant::now();
    select_instance(app, INFRA, OTHER);
    app.key("enter");
    app.wait_screen("attached infra-control-plane", |screen| {
        let visible = text(screen);
        visible.contains(INFRA_IDENTITY)
            && visible.contains("Attached to infra-control-plane")
            && !visible.contains(TAKEN)
            && !visible.contains(PAGE)
            && !visible.contains(PAYMENTS)
    });
    clock
}

fn guard_stale(stale: &App, polls: &mut u32) {
    assert!(stale.child_running(), "stale process quit");
    let screen = stale.screen();
    let visible = text(&screen);
    assert!(
        !visible.contains(TAKEN),
        "stale takeover painted the overlay\n{visible}"
    );
    assert!(
        visible.contains(INFRA) && visible.contains(INFRA_IDENTITY),
        "stale capsule left infra-control-plane\n{visible}"
    );
    assert!(
        !visible.contains(PAYMENTS),
        "stale takeover switched to payments-platform\n{visible}"
    );
    assert!(
        !visible.contains(TARGET),
        "stale takeover showed 7f3a\n{visible}"
    );
    *polls += 1;
}

fn shot(app: &App, checkpoint: &str, accept: impl Fn(&TerminalSnapshot) -> bool) {
    let test_id = visual_id(checkpoint);
    let screen = wait_settled(
        app,
        &test_id,
        Duration::from_millis(200),
        Duration::from_secs(8),
        accept,
    );
    check_frame(app, &screen, &test_id);
}

fn check_frame(app: &App, screen: &TerminalSnapshot, test_id: &str) {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/functional/baselines");
    let sample = tuiscotti_latest::sample::Sample::state(
        test_id,
        screen.clone(),
        tuiscotti_latest::sample::Observation::empty(),
    )
    .expect("build Tuiscotti state sample");
    let bytes = sample.to_bytes().expect("serialize Tuiscotti sample");
    let artifacts = app.evidence_dir().join("check-artifacts");
    let store = tuiscotti_latest::check::FrozenStore::new(&root, &artifacts)
        .expect("open Tuiscotti frozen store");
    let outcome = store.check(&bytes).expect("run Tuiscotti frozen check");
    if let CheckOutcome::Failure(failure) = outcome {
        panic!(
            "{test_id} {} sha {} path {} cells {}",
            failure.reason(),
            failure.candidate_sha256(),
            failure.candidate_path().display(),
            failure.changed_cell_total()
        );
    }
}

#[test]
fn j06_takeover_route() {
    let app = open("j06-takeover-route");
    let _clock = attach_payments(&app);
    assert!(
        _clock.elapsed() < Duration::from_secs(15),
        "payments attach was too late to catch the takeover"
    );
    let overlay = wait_for(
        &app,
        "takeover overlay",
        Duration::from_secs(70),
        || {},
        wide_overlay,
    );
    require(&overlay, TAKEN);
    require(&overlay, AGO);
    require(&overlay, KEEPS);
    refuse(&overlay, PAGE);
    refuse(&overlay, OUTRO);

    app.key("esc");
    let manager = app.wait_screen("left takeover", |screen| {
        let visible = text(screen);
        let focus = tree_focus(screen);
        screen.cols() == 120
            && screen.rows() == 40
            && visible.contains(PAGE)
            && visible.contains(DETACHED)
            && visible.contains(DETAIL)
            && focus.contains(TARGET)
            && !focus.contains(OTHER)
            && !visible.contains(TAKEN)
            && !visible.contains(OUTRO)
            && !visible.contains("7f3a · infra-control-plane")
    });
    require(&manager, DETACHED);
    require(&manager, DETAIL);
    require(&manager, PAYMENTS);
    refuse(&manager, TAKEN);
    refuse(&manager, OUTRO);
    assert!(
        tree_focus(&manager).contains(TARGET),
        "leave selected another row\n{}",
        text(&manager)
    );
    assert!(app.child_running(), "leave quit the process");
}

#[test]
fn j06_takeover_takeover() {
    let stale = open("j06-takeover-stale");
    let stale_clock = attach_infra(&stale);
    assert!(
        stale_clock.elapsed() < Duration::from_secs(15),
        "infra attach was too late to watch the takeover deadline"
    );

    let overlay = open("j06-takeover-overlay");
    let mut polls = 0u32;
    select_instance(&overlay, PAYMENTS, TARGET);
    assert!(
        stale_clock.elapsed() < Duration::from_secs(20),
        "stale polling started after the takeover could already have fired"
    );
    guard_stale(&stale, &mut polls);
    overlay.key("enter");
    wait_for(
        &overlay,
        "attached payments-platform",
        Duration::from_secs(8),
        || guard_stale(&stale, &mut polls),
        |screen| {
            let visible = text(screen);
            visible.contains(IDENTITY)
                && !visible.contains(TAKEN)
                && !visible.contains(PAGE)
                && !visible.contains(INFRA)
        },
    );

    let mut overlay_seen: Option<Instant> = None;
    let budget = Instant::now() + Duration::from_secs(80);
    loop {
        guard_stale(&stale, &mut polls);
        let screen = overlay.screen();
        if wide_overlay(&screen) {
            if overlay_seen.is_none() {
                overlay_seen = Some(Instant::now());
            }
        } else {
            let visible = text(&screen);
            assert!(
                visible.contains(IDENTITY) && !visible.contains(TAKEN) && !visible.contains(INFRA),
                "overlay session left payments-platform before the takeover\n{visible}"
            );
            overlay_seen = None;
        }
        let held = stale_clock.elapsed() >= SIM_HOLD;
        let cleared = overlay_seen.is_some_and(|seen| seen.elapsed() >= STATUS_CLEAR);
        if held && cleared {
            break;
        }
        assert!(
            Instant::now() < budget && overlay.child_running() && stale.child_running(),
            "takeover did not settle\nstale elapsed {:?}\n{}",
            stale_clock.elapsed(),
            text(&overlay.screen())
        );
        std::thread::sleep(Duration::from_millis(20));
    }
    assert!(
        polls >= 200,
        "stale hold polled {polls} frames, not the whole wait"
    );
    guard_stale(&stale, &mut polls);

    shot(&overlay, WIDE, wide_overlay);
    overlay.resize(80, 24);
    shot(&overlay, NARROW, narrow_overlay);
    overlay.resize(120, 40);
    let restored = wait_settled(
        &overlay,
        "wide overlay restored",
        Duration::from_millis(200),
        Duration::from_secs(8),
        wide_overlay,
    );
    require(&restored, TAKEN);
    require(&restored, AGO);
    refuse(&restored, PAGE);

    overlay.key("enter");
    let again = wait_for(
        &overlay,
        "reconnected",
        Duration::from_secs(8),
        || {},
        |screen| {
            let visible = text(screen);
            visible.contains(RECONNECTED)
                && visible.contains(IDENTITY)
                && !visible.contains(TAKEN)
                && !visible.contains(INFRA)
                && !visible.contains(PAGE)
                && !visible.contains(OUTRO)
        },
    );
    require(&again, RECONNECTED);
    require(&again, IDENTITY);
    refuse(&again, TAKEN);
    refuse(&again, INFRA);
    refuse(&again, OUTRO);
    guard_stale(&stale, &mut polls);
    assert!(overlay.child_running(), "reconnect quit the process");
    assert!(stale.child_running(), "the stale process quit");
}
