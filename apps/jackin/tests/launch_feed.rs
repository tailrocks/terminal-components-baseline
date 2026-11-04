#![cfg(feature = "demo")]

mod common;

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

use common::*;
use jackin_console::app::{App, Route};
use jackin_console::backend::{Backend, Completion, Effect, FileSource, OnePassword, OpSession};
use jackin_console::domain::agent::Agent;
use jackin_console::domain::instance::{DaemonSnapshot, Instance, InstanceStatus};
use jackin_console::domain::launch::{LaunchContent, LaunchFailure};
use jackin_console::fixtures::{Scenario, app_with_world, world_for};
use jackin_console::launch::{
    EnvAnswer, EnvQuestion, FailedLaunch, LaunchEvent, LaunchFeed, LaunchTarget, ReadyInstance,
    Refusal, TypedKey,
};
use jackin_console::panes::{PaneBody, PaneRequest, PaneSpawn};
use jackin_console::sim::SimBackend;
use jackin_console::world::{Msg, World};
use junie_tui::core::event::Input;
use junie_tui::motion::Motion;
use junie_tui::runtime::Application;
use junie_tui::theme::{ColorLevel, Theme};
use junie_tui::widgets::steps::StepState;

const WORK: &str = "acct-claude-work";

fn started(world: World) -> App {
    app_with_world(
        Scenario::LaunchRunning,
        world,
        Motion::Reduced,
        0,
        Theme::for_level(ColorLevel::TrueColor),
    )
}

fn tick(app: &mut App, times: usize) {
    for _ in 0..times {
        Application::handle(app, Input::Tick);
    }
}

fn tick_until(app: &mut App, what: &str, done: impl Fn(&mut App) -> bool) {
    for _ in 0..600 {
        if done(app) {
            return;
        }
        tick(app, 1);
    }
    let shown = screen(app);
    panic!("never reached: {what}\n{shown}");
}

fn shows(text: &'static str) -> impl Fn(&mut App) -> bool {
    move |app| screen(app).contains(text)
}

fn facts(i: &Instance) -> String {
    format!(
        "{} | {} | {} | {:?} | {} | {} | {} | {}",
        i.id,
        i.container,
        i.workdir,
        i.status,
        i.created_secs,
        i.run_id,
        i.github.as_ref().map_or("-", |r| r.full_name.as_str()),
        i.branch.as_deref().unwrap_or("-"),
    )
}

fn the_sim_launch(world: World) -> App {
    let mut app = started(world);
    tick_until(&mut app, "the capsule", |a| a.route() == Route::Capsule);
    app
}

#[test]
fn the_sim_launch_ends_as_it_did() {
    let app = the_sim_launch(world_for(Scenario::LaunchRunning));
    let record = app
        .world
        .instance("jk-b5df")
        .expect("the launched instance");
    assert_eq!(
        facts(record),
        "jk-b5df | jackin-payments-platform | /workspace/payments-platform | Running | 1788401643 \
         | run-202609030914-b5df | chainargos/payments-platform | feature/settlement-backoff"
    );
    assert_eq!(record.workspace, Some(1));
    assert_eq!(record.role, "the-architect");
    assert_eq!(record.agent, Agent::ClaudeCode);
    assert_eq!(record.default_branch, "main");
    assert_eq!(record.account.as_deref(), Some(WORK));
    assert_eq!(
        record.accounts,
        [
            "acct-claude-personal",
            "acct-claude-work",
            "acct-codex-primary",
            "acct-grok-team",
            "acct-opencode-go",
        ]
    );

    let daemon = app.world.daemons.get("jk-b5df").expect("its daemon");
    assert_eq!(daemon.workspace, "payments-platform");
    assert_eq!(daemon.tabs.len(), 1);
    let [pane] = daemon.panes.as_slice() else {
        panic!("one pane expected");
    };
    assert_eq!(pane.agent, Some(Agent::ClaudeCode));
    assert_eq!(pane.account.as_deref(), Some(WORK));
}

#[test]
fn the_sim_retry_unlocks_the_session_and_resumes() {
    let mut world = world_for(Scenario::LaunchRunning);
    world.backend = SimBackend::hard_cases();
    world.op.set_session(OpSession::Locked);
    let mut app = started(world);
    tick_until(
        &mut app,
        "the credential dialog",
        shows("Credential source unavailable"),
    );
    assert_eq!(app.world.op.session(), OpSession::Locked);
    assert!(screen(&mut app).contains("1Password locked: unlock the app and retry"));

    press(&mut app, "enter");
    assert_eq!(app.world.op.session(), OpSession::SignedIn);
    assert_eq!(
        status(&app),
        "1Password unlocked · resolving the reference again"
    );
    tick_until(&mut app, "the capsule", |a| a.route() == Route::Capsule);
    assert_eq!(
        app.world.instances.last().map(|i| i.run_id.as_str()),
        Some("run-202609030914-b5df")
    );
}

#[test]
fn the_sim_cancel_records_a_failed_setup() {
    let mut app = started(world_for(Scenario::LaunchRunning));
    tick(&mut app, 10);
    press(&mut app, "x");
    press(&mut app, "right enter");
    let record = app.world.instances.last().expect("a record");
    assert_eq!(
        facts(record),
        "jk-b5df | jackin-payments-platform | /workspace/payments-platform | FailedSetup | 1788401640 \
         | run-202609030914-b5df | chainargos/payments-platform | feature/settlement-backoff"
    );
    assert_eq!(record.daemon, DaemonSnapshot::Unavailable);
    assert_eq!(app.route(), Route::Manager);
    assert!(!app.world.daemons.contains_key("jk-b5df"));
}

#[test]
fn the_sim_abort_records_a_failed_setup() {
    let mut app = started(world_for(Scenario::LaunchRunning));
    tick(&mut app, 10);
    press(&mut app, "ctrl+c");
    let record = app.world.instances.last().expect("a record");
    assert_eq!(
        facts(record),
        "jk-b5df | jackin-payments-platform | /workspace/payments-platform | FailedSetup | 1788401640 \
         | run-202609030914-b5df | chainargos/payments-platform | feature/settlement-backoff"
    );
    assert_eq!(app.route(), Route::Manager);
}

#[test]
fn the_sim_failure_records_a_failed_setup() {
    let mut app = started(world_for(Scenario::LaunchFailure));
    tick_until(
        &mut app,
        "the failure",
        shows("The Construct network could not be attached"),
    );
    let record = app.world.instances.last().expect("a record");
    assert_eq!(
        facts(record),
        "jk-b5df | jackin-payments-platform | /workspace/payments-platform | FailedSetup | 1788401642 \
         | run-202609030914-b5df | chainargos/payments-platform | feature/settlement-backoff"
    );
    press(&mut app, "esc");
    assert_eq!(app.route(), Route::Manager);
}

fn status(app: &App) -> String {
    app.shell.status.text().unwrap_or_default().to_owned()
}

// The backend's side of a launch: a feed the test scripts, in place of the
// simulator's. Everything else a backend answers goes to the simulator.

#[derive(Debug, Default)]
struct Calls {
    retries: usize,
    plain_text: usize,
    cancels: usize,
    initial_panes: Vec<String>,
    answers: Vec<Vec<EnvAnswer>>,
}

#[derive(Debug)]
struct ScriptedFeed {
    run_id: String,
    batches: VecDeque<Vec<LaunchEvent>>,
    held: bool,
    calls: Arc<Mutex<Calls>>,
    cancel_record: Option<Instance>,
    collector: Option<String>,
    typed: Option<TypedKey>,
}

impl LaunchFeed for ScriptedFeed {
    fn run_id(&self) -> &str {
        &self.run_id
    }

    fn telemetry(&self) -> Option<&str> {
        self.collector.as_deref()
    }

    fn poll(&mut self, _: &World) -> Vec<LaunchEvent> {
        if self.held {
            return vec![];
        }
        let batch = self.batches.pop_front().unwrap_or_default();
        self.held = batch.iter().any(|e| {
            matches!(
                e,
                LaunchEvent::CredentialError { .. } | LaunchEvent::NeedsInput(_)
            )
        });
        batch
    }

    fn answer_env(&mut self, answers: Vec<EnvAnswer>) {
        self.calls.lock().expect("calls").answers.push(answers);
        self.held = false;
    }

    fn retry_credentials(&mut self, _: &mut dyn OnePassword) {
        self.calls.lock().expect("calls").retries += 1;
        self.held = false;
    }

    fn use_plain_text(&mut self) -> Option<TypedKey> {
        self.calls.lock().expect("calls").plain_text += 1;
        self.held = false;
        self.typed.clone()
    }

    fn cancel(&mut self, _: &World) -> Option<Instance> {
        self.calls.lock().expect("calls").cancels += 1;
        self.cancel_record.take()
    }
}

#[derive(Debug)]
struct FeedBackend {
    inner: Arc<dyn Backend>,
    feed: Mutex<Option<ScriptedFeed>>,
    calls: Arc<Mutex<Calls>>,
    with_shell: bool,
}

impl Backend for FeedBackend {
    fn perform(&self, effect: &Effect, world: &World) -> Option<Completion> {
        self.inner.perform(effect, world)
    }

    fn settle(&self, effect: &Effect, world: &World) -> Option<Msg> {
        self.inner.settle(effect, world)
    }

    fn launch_content(&self) -> Arc<LaunchContent> {
        self.inner.launch_content()
    }

    fn launch(&self, _: &World, _: &LaunchTarget) -> Box<dyn LaunchFeed> {
        Box::new(self.feed.lock().expect("feed").take().expect("one launch"))
    }

    fn spawn_pane(&self, request: &PaneRequest, spawn: PaneSpawn) -> Box<dyn PaneBody> {
        self.inner.spawn_pane(request, spawn)
    }

    fn initial_panes(&self, instance: &Instance) -> Vec<PaneRequest> {
        self.calls
            .lock()
            .expect("calls")
            .initial_panes
            .push(instance.id.clone());
        let mut panes = self.inner.initial_panes(instance);
        if self.with_shell {
            panes.push(PaneRequest::new(&instance.id, None, None));
        }
        panes
    }

    fn file_source(&self, world: &World) -> Box<dyn FileSource> {
        self.inner.file_source(world)
    }
}

const SANDBOX: &str = "/sandbox/project";

/// What the backend says it launched: nothing the cockpit could have made up
/// (its container is not `jackin-<workspace>`, its working directory is not
/// `/workspace/<workspace>`, it has no repository although the workspace name
/// matches one in the world).
fn launched(id: &str, container: &str, status: InstanceStatus) -> Instance {
    Instance {
        id: id.into(),
        container: container.into(),
        workspace: Some(1),
        workdir: SANDBOX.into(),
        role: "the-architect".into(),
        agent: Agent::ClaudeCode,
        status,
        created_secs: 1_700_000_000,
        last_seen_secs: 1_700_000_005,
        run_id: "R1".into(),
        sessions: Ok(vec![]),
        daemon: DaemonSnapshot::Unavailable,
        branch: None,
        pr: None,
        default_branch: "trunk".into(),
        github: None,
        uncommitted: 0,
        unpushed: 0,
        account: Some(WORK.into()),
        accounts: vec![WORK.into()],
    }
}

fn ready(instance: Instance) -> LaunchEvent {
    LaunchEvent::Ready(Box::new(ReadyInstance {
        instance,
        run_id: "R1".into(),
    }))
}

fn failed(instance: Instance) -> LaunchEvent {
    failed_with(Some(instance))
}

fn failed_with(instance: Option<Instance>) -> LaunchEvent {
    LaunchEvent::Failed(Box::new(FailedLaunch {
        failure: LaunchFailure {
            stage: 3,
            summary: "The sandbox could not be created".into(),
            next_step: "Check the gateway, then launch again.".into(),
            detail: vec!["gateway: jackin-local".into()],
        },
        instance,
    }))
}

fn running(stage: usize) -> Vec<LaunchEvent> {
    vec![LaunchEvent::StageChanged(stage, StepState::Running)]
}

fn scripted(
    batches: Vec<Vec<LaunchEvent>>,
    cancel_record: Option<Instance>,
) -> (App, Arc<Mutex<Calls>>) {
    scripted_panes(batches, cancel_record, false)
}

fn scripted_panes(
    batches: Vec<Vec<LaunchEvent>>,
    cancel_record: Option<Instance>,
    with_shell: bool,
) -> (App, Arc<Mutex<Calls>>) {
    let calls = Arc::new(Mutex::new(Calls::default()));
    let feed = ScriptedFeed {
        run_id: "R-7".into(),
        batches: batches.into(),
        held: false,
        calls: calls.clone(),
        cancel_record,
        collector: None,
        typed: None,
    };
    (feed_app(feed, &calls, with_shell, Motion::Reduced), calls)
}

fn feed_app(
    feed: ScriptedFeed,
    calls: &Arc<Mutex<Calls>>,
    with_shell: bool,
    motion: Motion,
) -> App {
    let mut world = world_for(Scenario::LaunchRunning);
    world.backend = Arc::new(FeedBackend {
        inner: SimBackend::shared(),
        calls: calls.clone(),
        with_shell,
        feed: Mutex::new(Some(feed)),
    });
    world.op.set_session(OpSession::Locked);
    app_with_world(
        Scenario::LaunchRunning,
        world,
        motion,
        0,
        Theme::for_level(ColorLevel::TrueColor),
    )
}

#[test]
fn ready_event_inserts_the_backend_instance() {
    let mine = launched("jk-5b0x", "sbx-alpha", InstanceStatus::Running);
    let (mut app, _) = scripted(vec![running(0), vec![ready(mine.clone())]], None);
    let others = app.world.instances.len();
    tick_until(&mut app, "the capsule", |a| a.route() == Route::Capsule);

    assert_eq!(app.world.instances.len(), others + 1);
    let mut stored = app.world.instance("jk-5b0x").expect("the instance").clone();
    stored.daemon = mine.daemon.clone();
    stored.last_seen_secs = mine.last_seen_secs;
    assert_eq!(
        stored, mine,
        "the cockpit stores the backend's record as it is"
    );
    assert_eq!(app.world.running_count(), 3);

    let shown = screen(&mut app);
    assert!(shown.contains("sbx-alpha-5b0x"), "{shown}");
    assert!(!shown.contains("jackin-payments-pl"), "{shown}");

    let daemon = app
        .world
        .daemons
        .get("jk-5b0x")
        .expect("the attach builds it");
    assert_eq!(daemon.workspace, "payments-platform");
    let [pane] = daemon.panes.as_slice() else {
        panic!("one pane expected");
    };
    assert_eq!(pane.agent, Some(Agent::ClaudeCode));
    assert_eq!(pane.account.as_deref(), Some(WORK));
}

#[test]
fn daemon_is_built_by_attach() {
    let mine = launched("jk-5b0x", "sbx-alpha", InstanceStatus::Running);
    let (mut app, calls) = scripted_panes(vec![running(0), vec![ready(mine)]], None, true);
    tick_until(&mut app, "the capsule", |a| a.route() == Route::Capsule);

    assert_eq!(
        calls.lock().expect("calls").initial_panes,
        ["jk-5b0x"],
        "the attach asks the backend once"
    );
    let daemon = app
        .world
        .daemons
        .get("jk-5b0x")
        .expect("the attach builds it");
    assert_eq!(daemon.tabs.len(), 2, "one tab per request the backend made");
    let [agent, shell] = daemon.panes.as_slice() else {
        panic!("two panes expected");
    };
    assert_eq!(agent.agent, Some(Agent::ClaudeCode));
    assert_eq!(
        agent.account.as_deref(),
        Some(WORK),
        "the launch's account reaches the agent pane"
    );
    assert_eq!(shell.agent, None);
}

const PRIMARY: &str = "acct-codex-primary";

fn agent_pane_account(record: Instance) -> Option<String> {
    let (mut app, _) = scripted(vec![running(0), vec![ready(record)]], None);
    tick_until(&mut app, "the capsule", |a| a.route() == Route::Capsule);
    let daemon = app.world.daemons.get("jk-5b0x").expect("its daemon");
    let [pane] = daemon.panes.as_slice() else {
        panic!("one pane expected");
    };
    pane.account.clone()
}

#[test]
fn the_agent_pane_runs_with_the_account_of_the_record() {
    let mut mine = launched("jk-5b0x", "sbx-alpha", InstanceStatus::Running);
    mine.account = Some(PRIMARY.into());
    assert_eq!(
        agent_pane_account(mine).as_deref(),
        Some(PRIMARY),
        "the cockpit launched with {WORK}; the record says otherwise"
    );
}

#[test]
fn a_record_without_an_account_gets_a_pane_without_one() {
    let mut mine = launched("jk-5b0x", "sbx-alpha", InstanceStatus::Running);
    mine.account = None;
    assert_eq!(
        agent_pane_account(mine),
        None,
        "the cockpit must not fill in the account of its own launch"
    );
}

#[test]
fn the_cockpit_builds_no_daemon() {
    let gone = launched("jk-5b0x", "sbx-alpha", InstanceStatus::CleanExited);
    let (mut app, _) = scripted(vec![running(0), vec![ready(gone)]], None);
    tick(&mut app, 4);

    assert_eq!(status(&app), "Cannot attach: 5b0x is not running");
    assert_eq!(app.route(), Route::Cockpit);
    assert!(app.world.instance("jk-5b0x").is_some());
    assert!(
        !app.world.daemons.contains_key("jk-5b0x"),
        "only the attach builds a daemon, and it refused"
    );
}

#[test]
fn failed_event_builds_from_the_event() {
    let mine = launched("jk-fa11", "sbx-failed", InstanceStatus::FailedSetup);
    let (mut app, _) = scripted(vec![running(0), vec![failed(mine.clone())]], None);
    let others = app.world.instances.len();
    tick_until(
        &mut app,
        "the failure",
        shows("The sandbox could not be created"),
    );

    assert_eq!(app.world.instances.len(), others + 1);
    assert_eq!(app.world.instance("jk-fa11"), Some(&mine));
    let shown = screen(&mut app);
    assert!(
        shown.contains("R-7"),
        "the run id comes from the feed:\n{shown}"
    );
    press(&mut app, "esc");
    assert_eq!(app.route(), Route::Manager);
}

#[test]
fn a_failed_event_without_a_record_adds_no_instance() {
    let (mut app, _) = scripted(vec![running(0), vec![failed_with(None)]], None);
    let others = app.world.instances.len();
    tick_until(
        &mut app,
        "the failure",
        shows("The sandbox could not be created"),
    );

    assert_eq!(
        app.world.instances.len(),
        others,
        "the backend kept no record, so the roster gets no row"
    );
    press(&mut app, "esc");
    assert_eq!(app.route(), Route::Manager);
}

#[test]
fn run_id_comes_from_the_feed() {
    let (mut app, _) = scripted(vec![running(0)], None);
    tick(&mut app, 2);
    press(&mut app, "i");
    let shown = screen(&mut app);
    assert!(shown.contains("Debug info"), "{shown}");
    assert!(shown.contains("R-7"), "{shown}");
    assert!(!shown.contains("run-2026"), "{shown}");
}

#[test]
fn retry_credentials() {
    let (mut app, calls) = scripted(
        vec![
            running(2),
            vec![LaunchEvent::CredentialError {
                message: "The vault is sealed".into(),
            }],
            vec![LaunchEvent::Activity("Resolved by the backend".into())],
        ],
        None,
    );
    tick_until(
        &mut app,
        "the credential dialog",
        shows("Credential source unavailable"),
    );
    assert!(screen(&mut app).contains("The vault is sealed"));

    press(&mut app, "enter");
    assert_eq!(calls.lock().expect("calls").retries, 1);
    assert_eq!(calls.lock().expect("calls").plain_text, 0);
    assert_eq!(
        app.world.op.session(),
        OpSession::Locked,
        "the screen does not unlock the session; the backend does"
    );
    assert!(!screen(&mut app).contains("Resolved by the backend"));
    tick_until(&mut app, "the next event", shows("Resolved by the backend"));
}

#[test]
fn plain_text_resumes_the_feed() {
    let (mut app, calls) = scripted(
        vec![
            running(2),
            vec![LaunchEvent::CredentialError {
                message: "The vault is sealed".into(),
            }],
            vec![LaunchEvent::Activity("Resolved by the backend".into())],
        ],
        None,
    );
    tick_until(
        &mut app,
        "the credential dialog",
        shows("Credential source unavailable"),
    );
    press(&mut app, "right enter");
    assert_eq!(calls.lock().expect("calls").retries, 0);
    assert_eq!(calls.lock().expect("calls").plain_text, 1);
    assert_eq!(app.world.op.session(), OpSession::Locked);
    tick_until(&mut app, "the next event", shows("Resolved by the backend"));
}

#[test]
fn cancel_keeps_the_record_the_feed_returns() {
    let mine = launched("jk-ca9c", "sbx-cancelled", InstanceStatus::FailedSetup);
    let (mut app, calls) = scripted(vec![running(0)], Some(mine.clone()));
    tick(&mut app, 2);
    let others = app.world.instances.len();
    press(&mut app, "x");
    press(&mut app, "right enter");

    assert_eq!(calls.lock().expect("calls").cancels, 1);
    assert_eq!(app.world.instances.len(), others + 1);
    assert_eq!(app.world.instance("jk-ca9c"), Some(&mine));
    assert_eq!(app.route(), Route::Manager);
}

#[test]
fn ctrl_c_keeps_the_record_the_feed_returns() {
    let mine = launched("jk-ca9c", "sbx-cancelled", InstanceStatus::FailedSetup);
    let (mut app, calls) = scripted(vec![running(0)], Some(mine.clone()));
    tick(&mut app, 2);
    press(&mut app, "ctrl+c");

    assert_eq!(calls.lock().expect("calls").cancels, 1);
    assert_eq!(app.world.instance("jk-ca9c"), Some(&mine));
    assert_eq!(app.route(), Route::Manager);
}

#[test]
fn a_cancel_without_a_record_adds_none() {
    let (mut app, calls) = scripted(vec![running(0)], None);
    tick(&mut app, 2);
    let before = app.world.instances.clone();
    press(&mut app, "x");
    press(&mut app, "right enter");

    assert_eq!(calls.lock().expect("calls").cancels, 1);
    assert_eq!(
        app.world.instances, before,
        "nothing is made up for a launch the backend never recorded"
    );
    assert_eq!(app.route(), Route::Manager);
}

fn timing_on(shown: &str) -> Option<String> {
    shown.lines().find_map(|line| {
        let cells: Vec<char> = line.chars().collect();
        cells.windows(5).find_map(|w| {
            (w[0].is_ascii_digit()
                && w[1] == '.'
                && w[2].is_ascii_digit()
                && w[3] == ' '
                && w[4] == 's')
                .then(|| w.iter().collect())
        })
    })
}

#[test]
fn a_feed_that_gives_no_timing_draws_none() {
    let (mut app, _) = scripted(
        vec![
            running(0),
            vec![
                LaunchEvent::StageChanged(0, StepState::Done),
                LaunchEvent::StageChanged(1, StepState::Running),
            ],
        ],
        None,
    );
    tick_until(&mut app, "the second stage", shows("1 done"));
    tick(&mut app, 3);
    let shown = screen(&mut app);
    assert_eq!(
        timing_on(&shown),
        None,
        "no duration is made up for a finished stage:\n{shown}"
    );
}

#[test]
fn a_feed_that_names_no_collector_shows_no_telemetry() {
    let (mut app, _) = scripted(vec![running(0)], None);
    tick(&mut app, 2);
    press(&mut app, "t");
    press(&mut app, "i");
    let shown = screen(&mut app);
    assert!(shown.contains("Debug info"), "{shown}");
    assert!(shown.contains("R-7"), "{shown}");
    assert!(!shown.contains("Telemetry"), "{shown}");
    assert!(!shown.contains("otlp"), "{shown}");
}

#[test]
fn a_feed_that_shows_no_key_leaves_the_credentials_line_alone() {
    let (mut app, calls) = scripted(
        vec![
            running(2),
            vec![LaunchEvent::CredentialError {
                message: "The vault is sealed".into(),
            }],
            vec![LaunchEvent::Activity("Resolved by the backend".into())],
        ],
        None,
    );
    tick_until(
        &mut app,
        "the credential dialog",
        shows("Credential source unavailable"),
    );
    press(&mut app, "right enter");
    assert_eq!(calls.lock().expect("calls").plain_text, 1);
    let shown = screen(&mut app);
    assert!(!shown.contains("masked API key"), "{shown}");
    assert!(!shown.contains("••••"), "{shown}");
    assert!(!shown.contains("typed once"), "{shown}");
}

#[test]
fn a_reply_due_on_the_handoff_is_delivered() {
    let mine = launched("jk-5b0x", "sbx-alpha", InstanceStatus::Running);
    let calls = Arc::new(Mutex::new(Calls::default()));
    let feed = ScriptedFeed {
        run_id: "R-7".into(),
        batches: vec![running(0), vec![ready(mine)]].into(),
        held: false,
        calls: calls.clone(),
        cancel_record: None,
        collector: None,
        typed: None,
    };
    let mut app = feed_app(feed, &calls, false, Motion::Full);
    tick_until(&mut app, "the handoff", |a| a.route() == Route::Handoff);

    let mut snapshot = jackin_console::sim::snapshot(&app.world);
    snapshot.workspaces.truncate(1);
    app.world.schedule(
        0,
        Msg::Refreshed {
            result: Ok(Box::new(snapshot)),
        },
    );
    tick(&mut app, 1);
    assert_eq!(
        app.world.workspaces.len(),
        1,
        "the reply that fell due on the handoff was dropped"
    );
}

fn feed_with(batches: Vec<Vec<LaunchEvent>>) -> (ScriptedFeed, Arc<Mutex<Calls>>) {
    let calls = Arc::new(Mutex::new(Calls::default()));
    let feed = ScriptedFeed {
        run_id: "R-7".into(),
        batches: batches.into(),
        held: false,
        calls: calls.clone(),
        cancel_record: None,
        collector: None,
        typed: None,
    };
    (feed, calls)
}

#[test]
fn a_timing_from_the_feed_is_drawn_for_its_stage() {
    let (feed, calls) = feed_with(vec![
        running(0),
        vec![
            LaunchEvent::StageChanged(0, StepState::Done),
            LaunchEvent::StageTimed {
                stage: 0,
                elapsed_ms: 40_000,
            },
            LaunchEvent::StageChanged(1, StepState::Running),
        ],
    ]);
    let mut app = feed_app(feed, &calls, false, Motion::Reduced);
    tick_until(&mut app, "the timing", shows("40.0 s"));
}

#[test]
fn a_collector_from_the_feed_is_shown_in_the_debug_info() {
    let (mut feed, calls) = feed_with(vec![running(0)]);
    feed.collector = Some("otlp-test.invalid:1".into());
    let mut app = feed_app(feed, &calls, false, Motion::Reduced);
    tick(&mut app, 2);
    press(&mut app, "t");
    press(&mut app, "i");
    let shown = screen(&mut app);
    assert!(shown.contains("run R-7 -> otlp-test.invalid:1"), "{shown}");
}

#[test]
fn the_key_line_of_the_feed_is_what_the_credentials_line_shows() {
    let (mut feed, calls) = feed_with(vec![
        running(2),
        vec![LaunchEvent::CredentialError {
            message: "The vault is sealed".into(),
        }],
    ]);
    feed.typed = Some(TypedKey {
        origin: "typed by the operator".into(),
        validation: "kept by the backend".into(),
    });
    let mut app = feed_app(feed, &calls, false, Motion::Reduced);
    tick_until(
        &mut app,
        "the credential dialog",
        shows("Credential source unavailable"),
    );
    press(&mut app, "right enter");
    let shown = screen(&mut app);
    assert!(shown.contains("typed by the operator"), "{shown}");
    assert!(shown.contains("kept by the backend"), "{shown}");
}

fn refused() -> LaunchEvent {
    LaunchEvent::Refused(Box::new(Refusal {
        title: "Held back at the door".into(),
        found: "The gate would not open for this role.".into(),
        state: "no record was kept".into(),
        next_step: "Try the other door, or ask the gate keeper.".into(),
    }))
}

#[test]
fn a_refusal_from_the_feed_is_drawn_in_the_feeds_words() {
    let (mut app, calls) = scripted(vec![vec![refused()]], None);
    let others = app.world.instances.clone();
    tick_until(&mut app, "the refusal", shows("Held back at the door"));
    let shown = screen(&mut app);
    for text in [
        "The gate would not open for this role.",
        "no record was kept",
        "Try the other door, or ask the gate keeper.",
    ] {
        assert!(shown.contains(text), "{text:?} missing:\n{shown}");
    }
    assert_eq!(app.world.instances, others, "a refusal creates nothing");
    assert_eq!(calls.lock().expect("calls").cancels, 0);
    assert!(
        !shown.contains("Launch failed"),
        "a refusal is not a failed stage:\n{shown}"
    );

    press(&mut app, "esc");
    assert_eq!(app.route(), Route::Manager);
    assert_eq!(app.world.instances, others);
    assert_eq!(
        calls.lock().expect("calls").cancels,
        0,
        "there is nothing to cancel after a refusal"
    );
}

fn question(name: &str, prompt: &str, default: Option<&str>) -> EnvQuestion {
    EnvQuestion {
        name: name.into(),
        prompt: prompt.into(),
        default: default.map(Into::into),
    }
}

fn asks() -> LaunchEvent {
    LaunchEvent::NeedsInput(vec![
        question("FIRST_VALUE", "Name the first thing", Some("kept-default")),
        question("SECOND_VALUE", "Name the second thing", None),
    ])
}

fn answer(name: &str, value: &str) -> EnvAnswer {
    EnvAnswer {
        name: name.into(),
        value: value.into(),
    }
}

#[test]
fn the_feeds_questions_open_a_form_and_the_answers_go_back() {
    let mine = launched("jk-5b0x", "sbx-alpha", InstanceStatus::Running);
    let (mut app, calls) = scripted(vec![running(0), vec![asks()], vec![ready(mine)]], None);
    tick_until(&mut app, "the questions", shows("Name the first thing"));
    let shown = screen(&mut app);
    for text in [
        "Name the second thing",
        "kept-default",
        "FIRST_VALUE",
        "SECOND_VALUE",
    ] {
        assert!(shown.contains(text), "{text:?} missing:\n{shown}");
    }
    tick(&mut app, 5);
    assert_eq!(app.route(), Route::Cockpit, "the launch waits for the form");
    assert!(calls.lock().expect("calls").answers.is_empty());

    press(&mut app, "tab enter");
    for key in ["h", "e", "l", "l", "o"] {
        press(&mut app, key);
    }
    press(&mut app, "enter");
    click_text(&mut app, "Start launch");
    assert_eq!(
        calls.lock().expect("calls").answers,
        [[
            answer("FIRST_VALUE", "kept-default"),
            answer("SECOND_VALUE", "hello")
        ]],
        "one answer for each question, in the feed's order"
    );
    tick_until(&mut app, "the capsule", |a| a.route() == Route::Capsule);
    assert_eq!(calls.lock().expect("calls").cancels, 0);
}

#[test]
fn escape_on_the_questions_cancels_the_launch_and_creates_nothing() {
    let (mut app, calls) = scripted(vec![running(0), vec![asks()]], None);
    tick_until(&mut app, "the questions", shows("Name the first thing"));
    let others = app.world.instances.clone();
    press(&mut app, "esc");

    let calls = calls.lock().expect("calls");
    assert!(calls.answers.is_empty(), "nothing is sent on Escape");
    assert_eq!(calls.cancels, 1, "the backend is told the launch ended");
    assert_eq!(app.world.instances, others);
    assert_eq!(app.route(), Route::Manager);
}

#[test]
fn a_blocked_stage_keeps_the_cancel_it_offers() {
    let mine = launched("jk-b10c", "sbx-blocked", InstanceStatus::FailedSetup);
    let (mut app, calls) = scripted(
        vec![
            running(1),
            vec![LaunchEvent::StageChanged(1, StepState::Blocked)],
        ],
        Some(mine.clone()),
    );
    tick_until(&mut app, "the blocked stage", shows("blocked"));
    let shown = screen(&mut app);
    assert!(shown.contains("x Cancel launch…"), "no hint\n{shown}");

    let others = app.world.instances.len();
    press(&mut app, "x");
    assert!(screen(&mut app).contains("Cancel the launch?"));
    press(&mut app, "right enter");
    assert_eq!(
        calls.lock().expect("calls").cancels,
        1,
        "x reaches the feed's cancel while a stage is blocked"
    );
    assert_eq!(app.world.instances.len(), others + 1);
    assert_eq!(app.world.instance("jk-b10c"), Some(&mine));
    assert_eq!(app.route(), Route::Manager);
}
