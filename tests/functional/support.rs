use std::cell::{Cell, RefCell};
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use tui_test::{
    CheckedMouseAction, KeyAction, LaunchEnvMode, LaunchOptions, MouseOptions, OpenOptions,
    Operation, OperationResult, RunOptions, Session, SnapshotCell, SnapshotScope, TerminalSnapshot,
    WaitQuietOptions,
};

pub const SHOWCASE: &str = env!("CARGO_BIN_EXE_showcase");
pub const TABLEPRO: &str = env!("CARGO_BIN_EXE_tablepro");
pub const HOLLA: &str = env!("CARGO_BIN_EXE_holla");
pub const JACKIN: &str = env!("CARGO_BIN_EXE_jackin");
const TIMEOUT: Duration = Duration::from_secs(10);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Point {
    pub x: u16,
    pub y: u16,
}

/// Launch description for a functional session.
///
/// `color`: `None` passes no `--color` and sets no `NO_COLOR`. `Some("nocolor")`
/// sets `NO_COLOR=1`. Any other value is passed as `--color`.
/// `history`: `false` sets `HOLLA_NO_HISTORY=1`. `true` omits that variable.
/// `motion`: when set, `--motion` is appended. `clock` enables the test clock.
#[derive(Clone, Debug)]
pub struct Launch {
    pub name: String,
    pub bin: PathBuf,
    pub args: Vec<String>,
    pub cols: u16,
    pub rows: u16,
    pub color: Option<String>,
    pub platform: String,
    pub history: bool,
    pub motion: Option<String>,
    pub clock: bool,
    pub env: Vec<(String, String)>,
    pub term: String,
}

impl Launch {
    pub fn new(name: &str, bin: &str) -> Self {
        Self {
            name: name.to_string(),
            bin: PathBuf::from(bin),
            args: Vec::new(),
            cols: 80,
            rows: 24,
            color: None,
            platform: "macos".to_string(),
            history: false,
            motion: None,
            clock: false,
            env: Vec::new(),
            term: "xterm-256color".to_string(),
        }
    }
}

#[derive(Debug)]
pub struct LaunchError {
    pub evidence: PathBuf,
    pub message: String,
}

impl std::fmt::Display for LaunchError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "spawn failed: {} ({})",
            self.message,
            self.evidence.display()
        )
    }
}

impl std::error::Error for LaunchError {}

pub struct App {
    session: Session,
    directory: PathBuf,
    actions: RefCell<Vec<String>>,
    checkpoint: Cell<usize>,
    clock: bool,
}

impl App {
    pub fn new(name: &str, bin: &str, args: &[&str], cols: u16, rows: u16, color: &str) -> Self {
        let mut launch = Launch::new(name, bin);
        launch.args = args.iter().map(|arg| (*arg).to_string()).collect();
        launch.cols = cols;
        launch.rows = rows;
        launch.color = Some(color.to_string());
        // Only jackin overrides `advance_clock` (src/bin/jackin/app.rs). Elsewhere the
        // paste falls through to `on_input` (src/shell/app.rs), so the clock stays off.
        launch.clock = Path::new(bin).file_name().and_then(|file| file.to_str()) == Some("jackin");
        try_launch(launch).unwrap_or_else(|error| panic!("{error}"))
    }

    pub fn evidence_dir(&self) -> &Path {
        &self.directory
    }

    pub fn child_running(&self) -> bool {
        // `session::Session::is_alive` is crate-private. `Operation::State` is the
        // public view the process watcher fills from that same wait.
        match self
            .session
            .execute(Operation::State)
            .expect("process state")
        {
            OperationResult::State(state) => state.exited.is_none() && state.exit_signal.is_none(),
            _ => panic!("missing process state"),
        }
    }

    pub fn screen(&self) -> TerminalSnapshot {
        self.session
            .capture_snapshot(SnapshotScope::ActiveScreen)
            .expect("capture live terminal")
    }

    pub fn wait_screen(
        &self,
        reason: &str,
        predicate: impl Fn(&TerminalSnapshot) -> bool,
    ) -> TerminalSnapshot {
        let deadline = Instant::now() + TIMEOUT;
        loop {
            self.assert_running(reason);
            let screen = self.screen();
            if predicate(&screen) {
                self.record(reason, &screen);
                return screen;
            }
            assert!(
                Instant::now() < deadline,
                "timed out: {reason}\n{}",
                text(&screen)
            );
            std::thread::sleep(Duration::from_millis(25));
        }
    }

    pub fn stable_screen(&self) -> TerminalSnapshot {
        let deadline = Instant::now() + Duration::from_secs(5);
        self.assert_running("stability");
        let mut screen = self.screen();
        let mut stable_since = Instant::now();
        loop {
            if stable_since.elapsed() >= Duration::from_millis(700) {
                return screen;
            }
            if Instant::now() >= deadline {
                let visible = text(&screen);
                let _ = std::fs::write(self.directory.join("last.txt"), &visible);
                self.log(format!("stability deadline {}", self.directory.display()));
                panic!("stability deadline {}", self.directory.display());
            }
            std::thread::sleep(Duration::from_millis(100));
            self.assert_running("stability");
            let candidate = self.screen();
            if candidate != screen {
                screen = candidate;
                stable_since = Instant::now();
            }
        }
    }

    pub fn visual(&self, test_id: &str) {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/functional/baselines");
        let outcome = self.check_visual(&root, test_id);
        assert!(
            outcome.is_pass(),
            "visual mismatch for {test_id}: {outcome:?}"
        );
        self.log(format!("visual {test_id} pass"));
    }

    pub fn check_visual(
        &self,
        baseline_root: &Path,
        test_id: &str,
    ) -> tuiscotti_latest::check::CheckOutcome {
        let _ = self.settle();
        let screen = self.stable_screen();
        let sample = tuiscotti_latest::sample::Sample::state(
            test_id,
            screen,
            tuiscotti_latest::sample::Observation::empty(),
        )
        .expect("build Tuiscotti state sample");
        let bytes = sample.to_bytes().expect("serialize Tuiscotti sample");
        let artifacts = self.directory.join("check-artifacts");
        let store = tuiscotti_latest::check::FrozenStore::new(baseline_root, &artifacts)
            .expect("open Tuiscotti frozen store");
        let outcome = store.check(&bytes).expect("run Tuiscotti frozen check");
        let sha = match &outcome {
            tuiscotti_latest::check::CheckOutcome::Pass(success) => {
                success.candidate_sha256().to_string()
            }
            tuiscotti_latest::check::CheckOutcome::Failure(failure) => {
                failure.candidate_sha256().to_string()
            }
        };
        self.log(format!("visual {test_id} candidate {sha}"));
        outcome
    }

    pub fn try_sustained(
        &self,
        hold: Duration,
        predicate: impl Fn(&TerminalSnapshot) -> bool,
    ) -> bool {
        if !self.child_running() || !predicate(&self.screen()) {
            return false;
        }
        let deadline = Instant::now() + hold * 4 + Duration::from_secs(2);
        let mut since: Option<Instant> = None;
        loop {
            let screen = self.screen();
            if self.child_running() && predicate(&screen) {
                let start = *since.get_or_insert_with(Instant::now);
                if start.elapsed() >= hold {
                    return true;
                }
            } else {
                since = None;
            }
            if Instant::now() >= deadline {
                return false;
            }
            std::thread::sleep(Duration::from_millis(50));
        }
    }

    pub fn wait_text(&self, needle: &str) -> TerminalSnapshot {
        self.wait_screen(&format!("visible {needle}"), |screen| {
            text(screen).contains(needle)
        })
    }

    pub fn wait_absent(&self, needle: &str) -> TerminalSnapshot {
        self.wait_screen(&format!("absent {needle}"), |screen| {
            !text(screen).contains(needle)
        })
    }

    pub fn settle(&self) -> TerminalSnapshot {
        self.session
            .wait_quiet(&WaitQuietOptions::new(TIMEOUT, Duration::from_millis(150)).unwrap())
            .expect("terminal must settle after the semantic assertion");
        let screen = self.screen();
        self.record("settled", &screen);
        screen
    }

    pub fn point(&self, needle: &str) -> Point {
        find(&self.wait_text(needle), needle)
    }

    pub fn key(&self, key: &str) {
        self.log(format!("key {key}"));
        self.session
            .execute(Operation::Key {
                keys: vec![key.to_string()],
                action: KeyAction::Press,
            })
            .unwrap_or_else(|e| panic!("key {key}: {e}"));
    }

    pub fn type_text(&self, value: &str) {
        self.log(format!("type {} UTF-8 bytes", value.len()));
        self.session
            .write_bytes(value.as_bytes())
            .expect("type through terminal input");
    }

    pub fn paste(&self, value: &str) {
        self.log(format!("paste {} UTF-8 bytes", value.len()));
        self.session
            .paste(value)
            .expect("paste using negotiated terminal mode");
    }

    pub fn advance_clock(&self, milliseconds: u64) {
        if !self.clock {
            self.log("clock refused".to_string());
            return;
        }
        self.paste(&format!("junie-tui:advance-clock:{milliseconds}"));
    }

    pub fn move_to(&self, point: Point) {
        self.mouse(CheckedMouseAction::Motion {
            x: point.x,
            y: point.y,
        });
    }

    pub fn down(&self, point: Point) {
        self.mouse(CheckedMouseAction::Press {
            x: point.x,
            y: point.y,
            options: MouseOptions::default(),
        });
    }

    pub fn up(&self, point: Point) {
        self.mouse(CheckedMouseAction::Release {
            x: point.x,
            y: point.y,
            options: MouseOptions::default(),
        });
    }

    pub fn drag_to(&self, point: Point) {
        self.mouse(CheckedMouseAction::Drag {
            x: point.x,
            y: point.y,
            options: MouseOptions::default(),
        });
    }

    fn mouse(&self, action: CheckedMouseAction) {
        self.log(format!("mouse {action:?}"));
        self.session
            .mouse_checked(action)
            .expect("application must negotiate SGR mouse input for this action");
    }

    pub fn click(&self, point: Point) {
        self.down(point);
        self.up(point);
    }

    pub fn click_text(&self, needle: &str) {
        self.click(self.point(needle));
    }

    pub fn wheel(&self, point: Point, down: bool, amount: usize) {
        self.move_to(point);

        let bytes = tui_test::input::mouse::scroll(point.x, point.y, !down);
        self.log(format!("wheel {point:?} down={down} amount={amount}"));
        for _ in 0..amount {
            self.session
                .write_bytes(bytes.as_bytes())
                .expect("wheel through application PTY");
        }
    }

    pub fn resize(&self, cols: u16, rows: u16) {
        self.log(format!("resize {cols}x{rows}"));
        self.session
            .execute(Operation::Resize { cols, rows })
            .expect("resize PTY");
    }

    pub fn finish(&self) {
        self.assert_running("before Ctrl+Q");
        self.key("ctrl+q");
        self.session
            .execute(Operation::WaitExit {
                timeout_ms: Some(10_000),
            })
            .expect("application must exit after Ctrl+Q");
        let OperationResult::State(state) =
            self.session.execute(Operation::State).expect("exit state")
        else {
            panic!("missing exit state");
        };
        assert_eq!(state.exited, Some(0), "normal application exit");
        assert_eq!(state.exit_signal, None, "application must not be killed");
        assert_eq!(state.mouse_mode, "none", "mouse tracking restored on exit");
        assert!(state.cursor.visible, "terminal cursor restored on exit");
        for mode in ["alternate_screen", "bracketed_paste"] {
            assert_eq!(
                state.modes.get(mode),
                Some(&false),
                "terminal mode {mode} restored on exit"
            );
        }
        self.log("clean exit 0".to_string());
    }

    pub fn expect_exited(&self, code: i32) {
        let alive = self.child_running();
        let OperationResult::State(state) =
            self.session.execute(Operation::State).expect("exit state")
        else {
            panic!("missing exit state");
        };
        assert!(!alive, "process is still running");
        assert_eq!(state.exited, Some(code), "process exit code");
        assert_eq!(state.exit_signal, None, "process must not be killed");
        self.log(format!("observed exit {code}"));
    }

    pub fn kill_child(&self) {
        self.log("kill child".to_string());
        // `session::Session::kill` is crate-private. Signal KILL is the public
        // operation that terminates the child through the same PTY kill.
        self.session
            .execute(Operation::Signal {
                name: "KILL".to_string(),
            })
            .expect("kill child");
        let deadline = Instant::now() + Duration::from_secs(2);
        while self.child_running() {
            assert!(
                Instant::now() < deadline,
                "child survived Session::kill {}",
                self.directory.display()
            );
            std::thread::sleep(Duration::from_millis(20));
        }
    }

    fn assert_running(&self, reason: &str) {
        let alive = self.child_running();
        let OperationResult::State(state) = self
            .session
            .execute(Operation::State)
            .expect("process state")
        else {
            panic!("missing process state while {reason}");
        };
        if alive && state.exited.is_none() && state.exit_signal.is_none() {
            return;
        }
        let visible = self
            .session
            .capture_snapshot(SnapshotScope::ActiveScreen)
            .map(|screen| text(&screen))
            .unwrap_or_default();
        let _ = std::fs::write(self.directory.join("last.txt"), &visible);
        self.persist_actions();
        panic!(
            "application exited while {reason}: code={:?} signal={:?}\n{visible}",
            state.exited, state.exit_signal
        );
    }

    fn log(&self, message: String) {
        self.actions.borrow_mut().push(message);
        self.persist_actions();
    }

    fn persist_actions(&self) {
        std::fs::write(
            self.directory.join("actions.txt"),
            self.actions.borrow().join("\n"),
        )
        .expect("persist functional action trace");
    }

    fn record(&self, label: &str, screen: &TerminalSnapshot) {
        let number = self.checkpoint.get();
        self.checkpoint.set(number + 1);
        let mut hasher = std::hash::DefaultHasher::new();
        screen.hash(&mut hasher);
        self.log(format!(
            "checkpoint {number:03}: {label} {:016x}",
            hasher.finish()
        ));
        let path = self.directory.join(format!("{number:03}"));
        std::fs::write(
            path.with_extension("json"),
            serde_json::to_vec(screen).unwrap(),
        )
        .expect("save terminal cells");
        std::fs::write(path.with_extension("txt"), text(screen)).expect("save terminal text");
    }
}

impl Drop for App {
    fn drop(&mut self) {
        if let Ok(screen) = self.session.capture_snapshot(SnapshotScope::ActiveScreen) {
            let _ = std::fs::write(self.directory.join("last.txt"), text(&screen));
            if let Ok(json) = serde_json::to_vec(&screen) {
                let _ = std::fs::write(self.directory.join("last.json"), json);
            }
        }
        let _ = std::fs::write(
            self.directory.join("actions.txt"),
            self.actions.borrow().join("\n"),
        );
        let _ = self.session.close();
        let _ = std::fs::remove_dir_all(self.directory.join("home"));
        if std::thread::panicking() {
            eprintln!("functional evidence: {}", self.directory.display());
        }
    }
}

pub fn try_launch(launch: Launch) -> Result<App, LaunchError> {
    let directory = evidence_directory(&launch.name);
    let home = directory.join("home");
    std::fs::create_dir_all(&home).expect("create isolated application home");
    let args = child_args(&launch);
    let session = Session::new(&launch.name);
    let defaults = OpenOptions::default();
    let spawn = session.spawn_with(
        prepared_launch(&launch, &home),
        RunOptions {
            backend: defaults.backend,
            program: String::new(),
            args: Vec::new(),
            profile: defaults.profile,
            cols: launch.cols,
            rows: launch.rows,
            cwd: None,
            env: Vec::new(),
            wait_ready: Some(false),
            restart: false,
            timeouts: defaults.timeouts,
            recording: defaults.recording,
            style: defaults.style,
        },
    );
    match spawn {
        Ok(_opened) => {
            write_binary_record(&directory, &launch.bin);
            let app = App {
                session,
                directory,
                actions: RefCell::new(vec![format!(
                    "spawn {} {args:?} {}x{} color={:?} clock={}",
                    launch.bin.display(),
                    launch.cols,
                    launch.rows,
                    launch.color,
                    launch.clock
                )]),
                checkpoint: Cell::new(0),
                clock: launch.clock,
            };
            app.persist_actions();
            Ok(app)
        }
        Err(error) => {
            let message = format!("{}: {error}", launch.bin.display());
            std::fs::write(directory.join("spawn-error.txt"), &message)
                .expect("write spawn-error.txt");
            Err(LaunchError {
                evidence: directory,
                message,
            })
        }
    }
}

pub fn text(screen: &TerminalSnapshot) -> String {
    (0..screen.rows())
        .map(|row| row_text(screen, row))
        .collect::<Vec<_>>()
        .join("\n")
}

pub fn row_text(screen: &TerminalSnapshot, row: u32) -> String {
    screen
        .row(row)
        .expect("row in terminal")
        .iter()
        .map(SnapshotCell::text)
        .collect()
}

pub fn find_all(screen: &TerminalSnapshot, needle: &str) -> Vec<Point> {
    assert!(!needle.is_empty(), "locator must not be empty");
    let mut matches = Vec::new();
    for row in 0..screen.rows() {
        let line = row_text(screen, row);
        for (byte, _) in line.match_indices(needle) {
            let mut offset = 0;
            for (column, cell) in screen.row(row).unwrap().iter().enumerate() {
                let next = offset + cell.text().len();
                if byte >= offset && byte < next {
                    matches.push(Point {
                        x: u16::try_from(column).unwrap(),
                        y: u16::try_from(row).unwrap(),
                    });
                    break;
                }
                offset = next;
            }
        }
    }
    matches
}

pub fn find(screen: &TerminalSnapshot, needle: &str) -> Point {
    let matches = find_all(screen, needle);
    assert_eq!(
        matches.len(),
        1,
        "expected one physical-row match for {needle:?}, got {matches:?}\n{}",
        text(screen)
    );
    matches[0]
}

pub fn find_in(
    screen: &TerminalSnapshot,
    left: u16,
    top: u16,
    right: u16,
    bottom: u16,
    needle: &str,
) -> Point {
    let matches: Vec<_> = find_all(screen, needle)
        .into_iter()
        .filter(|point| point.x >= left && point.x <= right && point.y >= top && point.y <= bottom)
        .collect();
    assert_eq!(
        matches.len(),
        1,
        "expected one match for {needle:?} inside ({left},{top})-({right},{bottom}), got {matches:?}\n{}",
        text(screen)
    );
    matches[0]
}

pub fn cell_bg(screen: &TerminalSnapshot, point: Point) -> tui_test::SnapshotColor {
    screen
        .cell_at(u32::from(point.x), u32::from(point.y))
        .expect("cell within terminal")
        .bg()
}

pub fn crop(screen: &TerminalSnapshot, point: Point, width: u16, height: u16) -> Vec<SnapshotCell> {
    (point.y..point.y + height)
        .flat_map(|y| {
            (point.x..point.x + width).map(move |x| {
                screen
                    .cell_at(u32::from(x), u32::from(y))
                    .expect("crop within terminal")
                    .clone()
            })
        })
        .collect()
}

pub fn file_sha256(path: &Path) -> String {
    let output = if Path::new("/usr/bin/shasum").is_file() {
        std::process::Command::new("/usr/bin/shasum")
            .arg("-a")
            .arg("256")
            .arg(path)
            .output()
    } else {
        std::process::Command::new("/usr/bin/sha256sum")
            .arg(path)
            .output()
    }
    .expect("run sha256");
    assert!(
        output.status.success(),
        "sha256 failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout)
        .expect("sha256 output")
        .split_whitespace()
        .next()
        .expect("sha256 digest")
        .to_string()
}

fn evidence_directory(name: &str) -> PathBuf {
    static SEQ: AtomicU64 = AtomicU64::new(0);
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock")
        .as_nanos();
    let seq = SEQ.fetch_add(1, Ordering::Relaxed);
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("target/functional")
        .join(format!("{name}-{}-{nanos}-{seq}", std::process::id()))
}

fn child_args(launch: &Launch) -> Vec<String> {
    let mut args = launch.args.clone();
    if let Some(color) = launch.color.as_deref()
        && color != "nocolor"
    {
        args.push("--color".to_string());
        args.push(color.to_string());
    }
    if let Some(motion) = &launch.motion {
        args.push("--motion".to_string());
        args.push(motion.clone());
    }
    args
}

fn prepared_launch(launch: &Launch, home: &Path) -> LaunchOptions {
    let mut options = LaunchOptions::new(launch.bin.as_os_str())
        .with_args(child_args(launch))
        .with_cwd(home)
        .with_env_mode(LaunchEnvMode::Cleared)
        .with_env("HOME", home)
        .with_env("XDG_CONFIG_HOME", home.join("config"))
        .with_env("XDG_CACHE_HOME", home.join("cache"))
        .with_env("XDG_DATA_HOME", home.join("data"))
        .with_env("SHELL", "/bin/sh")
        .with_env("PATH", "/usr/bin:/bin")
        .with_env("TERM", &launch.term)
        .with_env("COLORTERM", "truecolor")
        .with_env("LANG", "C.UTF-8")
        .with_env("LC_ALL", "C.UTF-8")
        .with_env("TZ", "UTC")
        .with_env("JUNIE_TUI_CLIENT_PLATFORM", &launch.platform);
    if launch.clock {
        options = options.with_env("JUNIE_TUI_TEST_CLOCK", "1");
    }
    if !launch.history {
        options = options.with_env("HOLLA_NO_HISTORY", "1");
    }
    if launch.color.as_deref() == Some("nocolor") {
        options = options.with_env("NO_COLOR", "1");
    }
    for (key, value) in &launch.env {
        options = options.with_env(key, value);
    }
    options
}

fn write_binary_record(directory: &Path, bin: &Path) {
    let path = if bin.is_absolute() {
        bin.to_path_buf()
    } else {
        std::env::current_dir()
            .expect("current directory")
            .join(bin)
    };
    let path = path.canonicalize().unwrap_or(path);
    let record = serde_json::json!({
        "path": path,
        "sha256": file_sha256(&path),
    });
    std::fs::write(
        directory.join("binary.json"),
        serde_json::to_vec_pretty(&record).expect("binary record"),
    )
    .expect("write binary.json");
}
