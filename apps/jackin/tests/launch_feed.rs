#![cfg(feature = "demo")]

mod common;

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

use common::*;
use jackin_console::app::{App, Route};
use jackin_console::backend::{
    Backend, ChangeSet, Completion, Effect, FileSource, FolderProbe, OnePassword, OpSession,
    ValidationOutcome,
};
use jackin_console::domain::account::CredentialSource;
use jackin_console::domain::agent::{Agent, Provider};
use jackin_console::domain::instance::{DaemonSnapshot, Instance, InstanceStatus};
use jackin_console::domain::launch::{LaunchContent, LaunchFailure};
use jackin_console::fixtures::{Scenario, app_with_world, world_for};
use jackin_console::launch::{FailedLaunch, LaunchEvent, LaunchFeed, LaunchTarget, ReadyInstance};
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
}

#[derive(Debug)]
struct ScriptedFeed {
    run_id: String,
    batches: VecDeque<Vec<LaunchEvent>>,
    held: bool,
    calls: Arc<Mutex<Calls>>,
    cancel_record: Option<Instance>,
}

impl LaunchFeed for ScriptedFeed {
    fn run_id(&self) -> &str {
        &self.run_id
    }

    fn poll(&mut self, _: &World) -> Vec<LaunchEvent> {
        if self.held {
            return vec![];
        }
        let batch = self.batches.pop_front().unwrap_or_default();
        self.held = batch
            .iter()
            .any(|e| matches!(e, LaunchEvent::CredentialError { .. }));
        batch
    }

    fn retry_credentials(&mut self, _: &mut dyn OnePassword) {
        self.calls.lock().expect("calls").retries += 1;
        self.held = false;
    }

    fn use_plain_text(&mut self) {
        self.calls.lock().expect("calls").plain_text += 1;
        self.held = false;
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
    fn validate(
        &self,
        provider: Provider,
        source: &CredentialSource,
        plain_value: Option<&str>,
        op: &dyn OnePassword,
        now: i64,
    ) -> ValidationOutcome {
        self.inner.validate(provider, source, plain_value, op, now)
    }

    fn probe_folder(&self, path: &str) -> FolderProbe {
        self.inner.probe_folder(path)
    }

    fn perform(&self, effect: &Effect, world: &World) -> Option<Completion> {
        self.inner.perform(effect, world)
    }

    fn settle(&self, effect: &Effect, world: &World) -> Option<Msg> {
        self.inner.settle(effect, world)
    }

    fn changes(
        &self,
        id: &str,
        touched: &[String],
        uncommitted: usize,
        unpushed: usize,
    ) -> ChangeSet {
        self.inner.changes(id, touched, uncommitted, unpushed)
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
    let mut world = world_for(Scenario::LaunchRunning);
    world.backend = Arc::new(FeedBackend {
        inner: SimBackend::shared(),
        calls: calls.clone(),
        with_shell,
        feed: Mutex::new(Some(ScriptedFeed {
            run_id: "R-7".into(),
            batches: batches.into(),
            held: false,
            calls: calls.clone(),
            cancel_record,
        })),
    });
    world.op.set_session(OpSession::Locked);
    (started(world), calls)
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
