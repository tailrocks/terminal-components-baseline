//! Cross-application suspend, before the refactor.
//!
//! Suspend is `job_control` in `src/runtime.rs`: the SIGTSTP handler only
//! sets a flag, and the event loop calls `TerminalSession::suspend`.
//! `tests/functional/support.rs` keeps `App.session` private. The only public
//! signal is `kill_child`, which sends `Operation::Signal` with name `KILL`.
//! `App::key("ctrl+z")` is `Operation::Key`. That key stays on Overview, and
//! a later `]` still changes the page, so the child was not stopped.
//! `Launch::new` leaves the clock off.

use std::time::Duration;

use tui_test::TerminalSnapshot;

use crate::support::{App, Launch, SHOWCASE, text, try_launch};

const BLURB: &str = "Tokens and principles behind every component";
const CAPTION: &str = "the live widget";
const CHROME: &str = "App shell ›";
const BRAND: &str = "Brand lockup";

fn open(name: &str) -> App {
    let mut launch = Launch::new(name, SHOWCASE);
    launch.clock = false;
    launch.cols = 120;
    launch.rows = 40;
    launch.color = Some("truecolor".to_string());
    launch.motion = Some("reduced".to_string());
    launch.args = ["--page", "overview"]
        .into_iter()
        .map(str::to_string)
        .collect();
    try_launch(launch).unwrap_or_else(|error| panic!("{error}"))
}

fn require(screen: &TerminalSnapshot, needle: &str) {
    let visible = text(screen);
    assert!(visible.contains(needle), "missing {needle}\n{visible}");
}

fn refuse(screen: &TerminalSnapshot, needle: &str) {
    let visible = text(screen);
    assert!(!visible.contains(needle), "{needle} present\n{visible}");
}

fn overview(app: &App) -> TerminalSnapshot {
    app.wait_text(BLURB);
    let screen = app.settle();
    require(&screen, BLURB);
    require(&screen, CAPTION);
    require(&screen, "Playground");
    refuse(&screen, CHROME);
    assert_eq!((screen.cols(), screen.rows()), (120u32, 40u32));
    assert!(app.child_running(), "overview startup stopped showcase");
    screen
}

#[test]
fn x02_suspend_route() {
    let app = open("x02-suspend-route");
    overview(&app);
    app.finish();
}

#[test]
fn x02_suspend_ctrl_z_stays() {
    let app = open("x02-suspend-ctrl-z");
    overview(&app);
    app.key("ctrl+z");
    let stayed = app.try_sustained(Duration::from_millis(800), |screen| {
        let visible = text(screen);
        visible.contains(BLURB)
            && visible.contains(CAPTION)
            && !visible.contains(CHROME)
            && screen.cols() == 120u32
            && screen.rows() == 40u32
    });
    assert!(stayed, "ctrl+z left Overview\n{}", text(&app.screen()));
    assert!(app.child_running(), "ctrl+z stopped showcase");
    app.key("]");
    let chrome = app.wait_text(BRAND);
    require(&chrome, CHROME);
    refuse(&chrome, BLURB);
    assert!(
        app.child_running(),
        "the page key after ctrl+z did not reach a running process"
    );
    app.finish();
}
