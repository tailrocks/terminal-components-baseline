//! Jackin tabs, splits, zoom, rename, and close, driven through the real binary.
//!
//! Tuiscotti test ids are `[a-z0-9_-]` segments, so the baseline directory is
//! `pre_refactor/j06_tabs/`. The shots are the open Close pane? dialog. They
//! do not nudge. `Launch::new` leaves the clock off. `App::new` would enable
//! it for jackin.

use std::path::PathBuf;
use std::time::{Duration, Instant};

use tui_test::TerminalSnapshot;
use tuiscotti_latest::check::CheckOutcome;

use crate::support::{App, JACKIN, Launch, Point, find_all, text, try_launch};

const WORKSPACE: &str = "payments-platform";
const CLAUDE_VERSION: &str = "Claude Code v2.1.14";
const CODEX_TITLE: &str = "Codex (Primary)";
const CODEX_TAIL: &str = "12.4k tokens";
const PERMISSION: &str = "Allow edit to src/settlement/config.rs? (y/n)";
const SHELL_BOOT: &str = "git status -sb";
const SHELL_PROMPT: &str = "payments-platform ❯";
const BRANCH: &str = "feature/settlement-backoff";
const OUTRO: &str = "You were in the Construct";
const LAUNCH_FAILED: &str = "Launch failed";
const NETWORK: &str = "network jackin-net not found";
const SHELL_MARK: &str = "zztabshell";
const KEEP: &str = "zzkeep";
const RIGHT: &str = "zzright";
const BELOW: &str = "zzbelow";
const LABEL: &str = "zzren";
const STARTED_SHELL: &str = "Started Shell";
const STARTED_GROK: &str = "Started Grok Build · account Team";
const STARTED_OPEN: &str = "Started OpenCode · account Go subscription";
const GROK_BODY: &str = "Grok Build 0.3";
const OPENCODE: &str = "opencode 0.5.11";
const ZOOMED: &str = "Zoomed · z restores the layout";
const UNZOOMED: &str = "Unzoomed";
const UNZOOM_RESIZE: &str = "Unzoom to resize";
const ONE_PANE: &str = "Nothing to zoom: one pane";
const NO_DIR: &str = "Nothing to resize in that direction";
const MIN_SIZE: &str = "Cannot resize: pane at minimum size";
const SPLIT_RESIZED: &str = "Split resized";
const RENAMED: &str = "Tab renamed";
const CLOSE_PANE: &str = "Close pane?";
const CLOSE_BODY: &str = "Reap the focused pane's agent";
const CLOSE_TAB: &str = "Close tab?";
const PANE_CLOSED: &str = "Pane closed";
const TAB_CLOSED: &str = "Tab closed";
const STILL: &str = "Still inside the Construct · 1 instance remains";
const WIDE: &str = "J06.tabs.layout.120x40";
const NARROW: &str = "J06.tabs.layout.narrow";

fn visual_id(checkpoint: &str) -> String {
    format!(
        "pre_refactor/j06_tabs/{}",
        checkpoint.replace('.', "_").to_ascii_lowercase()
    )
}

fn open(name: &str) -> App {
    let mut launch = Launch::new(name, JACKIN);
    launch.args = vec!["--scenario".to_string(), "capsule-multi".to_string()];
    launch.cols = 120;
    launch.rows = 40;
    launch.color = Some("truecolor".to_string());
    launch.motion = Some("reduced".to_string());
    // `Launch::new` leaves the clock off. `App::new` would enable it for jackin.
    launch.clock = false;
    try_launch(launch).unwrap_or_else(|error| panic!("{error}"))
}

fn glyph<'a>(screen: &'a TerminalSnapshot, x: u16, y: u16) -> &'a str {
    screen
        .cell_at(u32::from(x), u32::from(y))
        .map(|cell| cell.text())
        .unwrap_or("")
}

fn is_vbar(value: &str) -> bool {
    value == "│" || value == "┃"
}

fn in_body(screen: &TerminalSnapshot, needle: &str) -> bool {
    let rows = u16::try_from(screen.rows()).unwrap_or(0);
    find_all(screen, needle)
        .iter()
        .any(|hit| hit.y >= 4 && hit.y + 1 < rows)
}

fn body_hits(screen: &TerminalSnapshot, needle: &str) -> Vec<Point> {
    let rows = u16::try_from(screen.rows()).unwrap_or(0);
    find_all(screen, needle)
        .into_iter()
        .filter(|hit| hit.y >= 4 && hit.y + 1 < rows)
        .collect()
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

fn prefix(app: &App, cmd: &str) {
    app.key("ctrl+b");
    app.wait_screen("prefix", |screen| text(screen).contains("prefix…"));
    app.key(cmd);
}

fn pick(app: &App, title: &str, downs: usize, started: &str) {
    app.wait_screen(title, |screen| {
        let visible = text(screen);
        visible.contains(title) && visible.contains("Shell") && visible.contains("zsh")
    });
    for _ in 0..downs {
        app.key("down");
    }
    app.key("enter");
    app.wait_screen(started, |screen| {
        let visible = text(screen);
        visible.contains(started) && !visible.contains(title)
    });
}

fn capsule_ready(screen: &TerminalSnapshot) -> bool {
    let visible = text(screen);
    screen.cols() == 120
        && screen.rows() == 40
        && visible.contains(CLAUDE_VERSION)
        && visible.contains(PERMISSION)
        && visible.contains(CODEX_TITLE)
        && visible.contains(CODEX_TAIL)
        && visible.contains(SHELL_BOOT)
        && visible.contains(SHELL_PROMPT)
        && visible.contains(BRANCH)
        && visible.contains(WORKSPACE)
        && !visible.contains(OUTRO)
        && !visible.contains(LAUNCH_FAILED)
        && !visible.contains(NETWORK)
}

fn vertical_seam(screen: &TerminalSnapshot, left: &str, right: &str) -> Option<u16> {
    let left_hits = body_hits(screen, left);
    let right_hits = body_hits(screen, right);
    if left_hits.is_empty() || right_hits.is_empty() {
        return None;
    }
    let left_end = left_hits
        .iter()
        .map(|hit| hit.x.saturating_add(u16::try_from(left.len()).unwrap_or(0)))
        .max()?;
    let right_start = right_hits.iter().map(|hit| hit.x).min()?;
    if left_end >= right_start {
        return None;
    }
    let rows = u16::try_from(screen.rows()).unwrap_or(0);
    for y in 0..rows {
        for x in left_end..right_start {
            if is_vbar(glyph(screen, x, y)) {
                return Some(x);
            }
        }
    }
    None
}

fn assert_separated(screen: &TerminalSnapshot, left: &str, right: &str) {
    let seam = vertical_seam(screen, left, right).unwrap_or_else(|| {
        panic!(
            "{left} and {right} are not separated by a seam\n{}",
            text(screen)
        )
    });
    let left_width = u16::try_from(left.len()).unwrap_or(0);
    for hit in body_hits(screen, left) {
        assert!(
            hit.x.saturating_add(left_width) <= seam,
            "{left} crosses the seam at x={seam}\n{}",
            text(screen)
        );
    }
    for hit in body_hits(screen, right) {
        assert!(
            hit.x > seam,
            "{right} crosses the seam at x={seam}\n{}",
            text(screen)
        );
    }
}

fn seam_point(screen: &TerminalSnapshot) -> Point {
    let x = vertical_seam(screen, KEEP, RIGHT)
        .unwrap_or_else(|| panic!("no seam between the markers\n{}", text(screen)));
    let rows = u16::try_from(screen.rows()).unwrap_or(0);
    for y in 0..rows {
        if is_vbar(glyph(screen, x, y)) {
            return Point { x, y };
        }
    }
    panic!("seam column {x} has no bar\n{}", text(screen));
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

fn close_dialog(screen: &TerminalSnapshot, cols: u32, rows: u32) -> bool {
    if screen.cols() != cols || screen.rows() != rows {
        return false;
    }
    let visible = text(screen);
    visible.contains(CLOSE_PANE)
        && visible.contains(CLOSE_BODY)
        && visible.contains("Cancel")
        && visible.contains(KEEP)
        && !visible.contains(CLOSE_TAB)
        && !visible.contains(OUTRO)
}

fn shrink_to_minimum(app: &App) {
    for _ in 0..20 {
        app.key("ctrl+b");
        app.wait_screen("prefix", |screen| text(screen).contains("prefix…"));
        let before = vertical_seam(&app.screen(), KEEP, RIGHT);
        app.key("H");
        let screen = app.wait_screen("resize", |screen| {
            text(screen).contains(MIN_SIZE) || vertical_seam(screen, KEEP, RIGHT) != before
        });
        if text(&screen).contains(MIN_SIZE) {
            require(&screen, MIN_SIZE);
            assert_separated(&screen, KEEP, RIGHT);
            assert_separated(&screen, KEEP, BELOW);
            return;
        }
    }
    panic!("resize never refused the minimum\n{}", text(&app.screen()));
}

fn close_remaining_tabs(app: &App) {
    let mut saw_tab_closed = false;
    for _ in 0..8 {
        if text(&app.screen()).contains(STILL) {
            break;
        }
        app.key("ctrl+b");
        app.wait_screen("prefix before close tab", |screen| {
            let visible = text(screen);
            visible.contains("prefix…") || visible.contains(STILL)
        });
        if text(&app.screen()).contains(STILL) {
            break;
        }
        app.key("&");
        app.wait_screen("close tab dialog", |screen| {
            text(screen).contains(CLOSE_TAB)
        });
        app.key("right");
        app.key("enter");
        let closed = app.wait_screen("tab closed", |screen| {
            let visible = text(screen);
            !visible.contains(CLOSE_TAB)
                && (visible.contains(TAB_CLOSED) || visible.contains(STILL))
        });
        if text(&closed).contains(TAB_CLOSED) {
            saw_tab_closed = true;
        }
    }
    let left = app.wait_screen("left the capsule", |screen| {
        let visible = text(screen);
        visible.contains(STILL)
            && visible.contains("Workspaces")
            && !visible.contains(OUTRO)
            && !visible.contains(PERMISSION)
            && !visible.contains("Unsaved work")
    });
    assert!(
        saw_tab_closed,
        "no tab closed before the last session\n{}",
        text(&left)
    );
    require(&left, STILL);
    require(&left, "Workspaces");
    require(&left, WORKSPACE);
    refuse(&left, OUTRO);
    refuse(&left, PERMISSION);
    refuse(&left, "Unsaved work");
    refuse(&left, CLOSE_PANE);
    assert!(app.child_running(), "the process quit");
}

#[test]
fn j06_tabs_route() {
    let app = open("j06-tabs-route");
    let capsule = app.wait_screen("capsule", capsule_ready);
    require(&capsule, CLAUDE_VERSION);
    require(&capsule, PERMISSION);
    require(&capsule, CODEX_TITLE);
    require(&capsule, CODEX_TAIL);
    require(&capsule, SHELL_PROMPT);
    require(&capsule, WORKSPACE);
    assert_eq!((capsule.cols(), capsule.rows()), (120, 40));
    refuse(&capsule, OUTRO);
    refuse(&capsule, LAUNCH_FAILED);
    prefix(&app, "d");
    let manager = app.wait_screen("detached", |screen| {
        let visible = text(screen);
        visible.contains("Detached ·")
            && visible.contains("keeps running")
            && visible.contains("Workspaces")
            && !visible.contains(PERMISSION)
            && !visible.contains(OUTRO)
    });
    require(&manager, "Detached ·");
    require(&manager, "keeps running");
    require(&manager, WORKSPACE);
    require(&manager, "Workspaces");
    refuse(&manager, PERMISSION);
    refuse(&manager, OUTRO);
    assert!(app.child_running(), "detach quit the process");
}

#[test]
fn j06_tabs_layout() {
    let app = open("j06-tabs-layout");
    app.wait_screen("capsule", capsule_ready);

    prefix(&app, "c");
    pick(&app, "New tab", 4, STARTED_SHELL);
    app.type_text(SHELL_MARK);
    let shell = app.wait_screen("shell marker", |screen| in_body(screen, SHELL_MARK));
    require(&shell, STARTED_SHELL);
    assert!(in_body(&shell, SHELL_PROMPT));
    prefix(&app, "z");
    app.wait_screen("one pane zoom", |screen| text(screen).contains(ONE_PANE));
    prefix(&app, "H");
    app.wait_screen("no split", |screen| text(screen).contains(NO_DIR));

    prefix(&app, "1");
    let returned = app.wait_screen("original tab", |screen| {
        in_body(screen, PERMISSION) && !in_body(screen, SHELL_MARK)
    });
    require(&returned, CLAUDE_VERSION);
    require(&returned, CODEX_TAIL);

    prefix(&app, "c");
    pick(&app, "New tab", 3, STARTED_GROK);
    let grok = app.wait_screen("grok tab", |screen| in_body(screen, GROK_BODY));
    require(&grok, STARTED_GROK);
    refuse(&grok, "could not start");
    prefix(&app, "4");
    app.wait_screen("shell survived", |screen| {
        in_body(screen, SHELL_MARK) && !in_body(screen, GROK_BODY)
    });

    prefix(&app, "c");
    pick(&app, "New tab", 2, STARTED_OPEN);
    let open_tab = app.wait_screen("opencode tab", |screen| in_body(screen, OPENCODE));
    require(&open_tab, STARTED_OPEN);
    refuse(&open_tab, "could not start");
    prefix(&app, "4");
    app.wait_screen("shell still there", |screen| {
        in_body(screen, SHELL_MARK) && !in_body(screen, OPENCODE)
    });
    prefix(&app, "6");
    app.wait_screen("opencode returned", |screen| {
        in_body(screen, OPENCODE) && !in_body(screen, SHELL_MARK)
    });

    prefix(&app, "5");
    app.wait_screen("grok returned", |screen| {
        in_body(screen, GROK_BODY) && !in_body(screen, SHELL_MARK)
    });
    app.type_text(KEEP);
    app.wait_screen("keep marker", |screen| in_body(screen, KEEP));

    prefix(&app, "%");
    pick(&app, "Split → Right", 4, STARTED_SHELL);
    let split_right = app.wait_screen("split right", |screen| in_body(screen, KEEP));
    require(&split_right, KEEP);
    app.type_text(RIGHT);
    let right = app.wait_screen("right marker", |screen| in_body(screen, RIGHT));
    assert_separated(&right, KEEP, RIGHT);

    prefix(&app, "\"");
    pick(&app, "Split ↓ Below", 4, STARTED_SHELL);
    let split_below = app.wait_screen("split below", |screen| {
        in_body(screen, KEEP) && in_body(screen, RIGHT)
    });
    require(&split_below, KEEP);
    require(&split_below, RIGHT);
    app.type_text(BELOW);
    let below = app.wait_screen("below marker", |screen| in_body(screen, BELOW));
    assert_separated(&below, KEEP, BELOW);
    assert_separated(&below, KEEP, RIGHT);

    prefix(&app, "k");
    prefix(&app, "j");
    prefix(&app, "h");
    let focused_left = app.wait_screen("focus left", |screen| in_body(screen, KEEP));
    assert_separated(&focused_left, KEEP, RIGHT);
    assert_separated(&focused_left, KEEP, BELOW);
    prefix(&app, "l");
    prefix(&app, "h");
    let back = app.wait_screen("focus back", |screen| in_body(screen, KEEP));
    assert_separated(&back, KEEP, RIGHT);

    prefix(&app, "z");
    let zoomed = app.wait_screen("zoomed", |screen| text(screen).contains(ZOOMED));
    require(&zoomed, ZOOMED);
    require(&zoomed, KEEP);
    assert!(
        body_hits(&zoomed, RIGHT).is_empty(),
        "zoomed frame still shows {RIGHT}\n{}",
        text(&zoomed)
    );
    prefix(&app, "H");
    let blocked = app.wait_screen("zoom blocks resize", |screen| {
        text(screen).contains(UNZOOM_RESIZE)
    });
    require(&blocked, UNZOOM_RESIZE);
    require(&blocked, KEEP);
    prefix(&app, "z");
    let restored = app.wait_screen("unzoomed", |screen| {
        text(screen).contains(UNZOOMED) && in_body(screen, RIGHT) && in_body(screen, KEEP)
    });
    require(&restored, UNZOOMED);
    assert_separated(&restored, KEEP, RIGHT);

    let seam = seam_point(&app.screen());
    let held = Point {
        x: seam.x.saturating_add(4),
        y: seam.y,
    };
    app.move_to(seam);
    app.down(seam);
    app.drag_to(held);
    app.up(held);
    let dragged = app.wait_screen("split resized", |screen| {
        text(screen).contains(SPLIT_RESIZED)
    });
    require(&dragged, SPLIT_RESIZED);
    assert_separated(&dragged, KEEP, RIGHT);
    assert_separated(&dragged, KEEP, BELOW);

    shrink_to_minimum(&app);

    prefix(&app, ",");
    app.wait_screen("rename dialog", |screen| {
        text(screen).contains("Change tab title")
    });
    app.type_text(LABEL);
    app.wait_screen("rename typed", |screen| text(screen).contains(LABEL));
    app.key("enter");
    let renamed = app.wait_screen("renamed", |screen| text(screen).contains(RENAMED));
    require(&renamed, RENAMED);
    require(&renamed, LABEL);
    prefix(&app, ",");
    app.wait_screen("rename again", |screen| {
        text(screen).contains("Change tab title")
    });
    app.type_text("x");
    app.wait_screen("rename dirty", |screen| {
        let visible = text(screen);
        visible.contains("Change tab title")
            && (visible.contains("zzrenx") || visible.contains("▎ x"))
    });
    app.key("esc");
    let kept = app.wait_screen("rename cancelled", |screen| {
        let visible = text(screen);
        !visible.contains("Change tab title")
            && !visible.contains("zzrenx")
            && visible.contains(LABEL)
    });
    require(&kept, LABEL);
    refuse(&kept, "zzrenx");
    require(&kept, KEEP);

    prefix(&app, "l");
    prefix(&app, "x");
    // Shell listings and the rename status keep changing the frame until both
    // finish. The last shell line is the ls entry, and Tab renamed expires.
    app.wait_screen("close pane dialog", |screen| {
        let visible = text(screen);
        close_dialog(screen, 120, 40)
            && visible.contains("e2-standard-8")
            && find_all(screen, "0003-retry-policy.md").len() >= 2
            && !visible.contains(RENAMED)
    });

    shot(&app, WIDE, |screen| close_dialog(screen, 120, 40));
    app.resize(80, 24);
    shot(&app, NARROW, |screen| close_dialog(screen, 80, 24));
    app.resize(120, 40);
    wait_settled(
        &app,
        "wide restored",
        Duration::from_millis(200),
        Duration::from_secs(8),
        |screen| close_dialog(screen, 120, 40),
    );

    app.key("esc");
    let cancelled = app.wait_screen("close cancelled", |screen| {
        let visible = text(screen);
        !visible.contains(CLOSE_PANE)
            && in_body(screen, KEEP)
            && in_body(screen, RIGHT)
            && visible.contains(LABEL)
    });
    refuse(&cancelled, CLOSE_PANE);
    refuse(&cancelled, CLOSE_BODY);
    assert_separated(&cancelled, KEEP, RIGHT);
    assert_separated(&cancelled, KEEP, BELOW);

    prefix(&app, "x");
    app.wait_screen("close pane again", |screen| {
        text(screen).contains(CLOSE_PANE)
    });
    app.key("right");
    app.key("enter");
    let closed = app.wait_screen("pane closed", |screen| {
        let visible = text(screen);
        visible.contains(PANE_CLOSED) && !visible.contains(CLOSE_PANE) && in_body(screen, KEEP)
    });
    require(&closed, PANE_CLOSED);
    require(&closed, KEEP);
    let survivor = if in_body(&closed, RIGHT) {
        RIGHT
    } else {
        BELOW
    };
    assert!(
        in_body(&closed, survivor),
        "the other pane lost its output\n{}",
        text(&closed)
    );
    assert_separated(&closed, KEEP, survivor);
    refuse(&closed, OUTRO);

    close_remaining_tabs(&app);
}
