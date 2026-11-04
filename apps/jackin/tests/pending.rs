#![cfg(feature = "demo")]

mod common;

use std::sync::{Arc, Mutex};

use common::*;
use jackin_console::app::App;
use jackin_console::backend::{
    Backend, ChangeSet, ChangedFile, CheckRow, Completion, DiffLine, DiffStatus, Effect,
    FileSource, FolderProbe, Hunk, OpAccountRow, OpAccountState, OpError, OpListing, OpScope,
    ValidationOutcome,
};
use jackin_console::domain::account::{
    AccountIdentity, CredentialSource, DetectedKind, IssueCode, Recoverability, RecoverableIssue,
};
use jackin_console::domain::agent::Provider;
use jackin_console::domain::instance::Instance;
use jackin_console::domain::launch::LaunchContent;
use jackin_console::fixtures::{Scenario, app_with_world, world_for};
use jackin_console::launch::{LaunchFeed, LaunchTarget};
use jackin_console::panes::{PaneBody, PaneRequest, PaneSpawn};
use jackin_console::sim::{SimBackend, provider};
use jackin_console::world::{Msg, World};
use junie_tui::core::event::Input;
use junie_tui::motion::Motion;
use junie_tui::runtime::Application;
use junie_tui::theme::{ColorLevel, Theme};

/// The simulator, except that the four requests this file is about get no
/// answer until the test delivers one, the way a slow provider, a slow disk
/// or a slow `op` would leave them.
#[derive(Debug)]
struct Held {
    inner: Arc<dyn Backend>,
    seen: Mutex<Vec<Effect>>,
}

impl Held {
    fn new() -> Arc<Self> {
        Arc::new(Self {
            inner: SimBackend::shared(),
            seen: Mutex::new(vec![]),
        })
    }

    fn seen(&self) -> Vec<Effect> {
        let mut all = self.seen.lock().expect("seen").clone();
        all.retain(|e| *e != Effect::Refresh);
        all
    }

    fn checks(&self) -> Vec<Effect> {
        self.seen()
            .into_iter()
            .filter(|e| matches!(e, Effect::CheckSource { .. }))
            .collect()
    }

    fn saves(&self) -> usize {
        self.seen()
            .iter()
            .filter(|e| matches!(e, Effect::SaveAccounts { .. }))
            .count()
    }

    fn last_ticket(&self, want: fn(&Effect) -> Option<u64>) -> u64 {
        self.seen().iter().rev().find_map(want).unwrap_or_else(|| {
            panic!(
                "the request was not sent; the backend saw {:?}",
                self.seen()
            )
        })
    }

    fn check_ticket(&self) -> u64 {
        self.last_ticket(|e| match e {
            Effect::CheckSource { ticket, .. } => Some(*ticket),
            _ => None,
        })
    }

    fn probe_ticket(&self) -> u64 {
        self.last_ticket(|e| match e {
            Effect::ProbeFolder { ticket, .. } => Some(*ticket),
            _ => None,
        })
    }

    fn read_ticket(&self) -> u64 {
        self.last_ticket(|e| match e {
            Effect::ReadChanges { ticket, .. } => Some(*ticket),
            _ => None,
        })
    }

    fn op_ticket(&self) -> u64 {
        self.last_ticket(|e| match e {
            Effect::LoadOp { ticket, .. } => Some(*ticket),
            _ => None,
        })
    }

    fn op_scopes(&self) -> Vec<OpScope> {
        self.seen()
            .iter()
            .filter_map(|e| match e {
                Effect::LoadOp { scope, .. } => Some(scope.clone()),
                _ => None,
            })
            .collect()
    }
}

impl Backend for Held {
    fn perform(&self, effect: &Effect, world: &World) -> Option<Completion> {
        self.seen.lock().expect("seen").push(effect.clone());
        match effect {
            Effect::CheckSource { .. }
            | Effect::ProbeFolder { .. }
            | Effect::ReadChanges { .. }
            | Effect::LoadOp { .. } => None,
            _ => self.inner.perform(effect, world),
        }
    }

    fn settle(&self, effect: &Effect, world: &World) -> Option<Msg> {
        self.inner.settle(effect, world)
    }

    fn launch_content(&self) -> Arc<LaunchContent> {
        self.inner.launch_content()
    }

    fn launch(&self, world: &World, target: &LaunchTarget) -> Box<dyn LaunchFeed> {
        self.inner.launch(world, target)
    }

    fn spawn_pane(&self, request: &PaneRequest, spawn: PaneSpawn) -> Box<dyn PaneBody> {
        self.inner.spawn_pane(request, spawn)
    }

    fn initial_panes(&self, instance: &Instance) -> Vec<PaneRequest> {
        self.inner.initial_panes(instance)
    }

    fn file_source(&self, world: &World) -> Box<dyn FileSource> {
        self.inner.file_source(world)
    }
}

fn held(scenario: Scenario) -> (App, Arc<Held>) {
    let backend = Held::new();
    let mut world = world_for(scenario);
    world.backend = backend.clone();
    (timed(scenario, world), backend)
}

fn timed(scenario: Scenario, world: World) -> App {
    app_with_world(
        scenario,
        world,
        Motion::Reduced,
        10_000,
        Theme::for_level(ColorLevel::TrueColor),
    )
}

fn ticks(app: &mut App, n: usize) {
    for _ in 0..n {
        Application::handle(app, Input::Tick);
    }
}

fn status(app: &App) -> String {
    app.shell.status.text().unwrap_or_default().to_owned()
}

/// "Spare", provider Claude, the plain-text source and a key the simulator
/// takes as valid. The cursor ends on the key field's tail.
const PLAIN_KEY: &str = "c a enter S p a r e tab tab tab down down tab enter s k - a n t - v a l i d - a b c d e f 1 2 3 4 tab";

const SPARE_CANARY: &str = "reply from the backend";

/// What the simulator would answer for the typed key, with one row changed so
/// that a form showing it can only have taken it from the reply.
fn key_outcome(app: &App) -> ValidationOutcome {
    let mut outcome = provider::validate(
        Provider::Anthropic,
        &CredentialSource::PlainApiKey {
            fingerprint: "a605d51c".into(),
            tail: "1234".into(),
        },
        Some("sk-ant-valid-abcdef1234"),
        app.world.op.as_ref(),
        app.world.now_secs(),
    );
    outcome.material = CheckRow::Ok(SPARE_CANARY.into());
    outcome
}

fn checked(ticket: u64, outcome: ValidationOutcome) -> Msg {
    Msg::SourceChecked {
        ticket,
        result: Ok(Box::new(outcome)),
    }
}

#[test]
fn validate_leaves_the_form_pending_until_the_backend_answers() {
    let (mut app, backend) = held(Scenario::AccountsMixed);
    press(&mut app, PLAIN_KEY);
    click_text(&mut app, "Validate");

    let checks = backend.checks();
    let [
        Effect::CheckSource {
            provider,
            source,
            key,
            ..
        },
    ] = checks.as_slice()
    else {
        panic!("one CheckSource expected, got {:?}", backend.seen());
    };
    assert_eq!(*provider, Provider::Anthropic);
    assert!(matches!(source, CredentialSource::PlainApiKey { tail, .. } if tail == "1234"));
    assert!(key.is_some());

    let pending = screen(&mut app);
    for label in ["material", "identity", "quota"] {
        assert!(
            pending.contains(&format!("… {label}")) && pending.contains("checking"),
            "the {label} row waits:\n{pending}"
        );
    }
    assert!(!pending.contains("✓ material"), "{pending}");
    assert_eq!(status(&app), "Validating the source…");
    assert!(pending.contains("New account"), "the form stays open");

    click_text(&mut app, "Validate");
    assert_eq!(
        backend.checks().len(),
        1,
        "a pending check is not asked twice"
    );

    let ticket = backend.check_ticket();
    let outcome = key_outcome(&app);
    app.deliver(&checked(ticket, outcome));
    let done = screen(&mut app);
    assert!(
        done.contains(&format!("✓ material   {SPARE_CANARY}")),
        "{done}"
    );
    assert!(!done.contains("checking"), "{done}");
    assert_eq!(
        status(&app),
        "Validated · Save stores the reference, never the secret"
    );
}

#[test]
fn a_failed_check_shows_the_reply_error_in_the_form() {
    let (mut app, backend) = held(Scenario::AccountsMixed);
    press(&mut app, PLAIN_KEY);
    click_text(&mut app, "Validate");
    let ticket = backend.check_ticket();
    app.deliver(&Msg::SourceChecked {
        ticket,
        result: Err("the provider did not answer".into()),
    });

    let shown = screen(&mut app);
    assert!(shown.contains("the provider did not answer"), "{shown}");
    assert!(!shown.contains("checking"), "{shown}");
    assert!(shown.contains("New account"), "{shown}");
    click_text(&mut app, "Validate");
    assert_eq!(
        backend.checks().len(),
        2,
        "the button works again after a failed check"
    );
}

#[test]
fn editing_the_form_drops_the_pending_check() {
    let (mut app, backend) = held(Scenario::AccountsMixed);
    press(&mut app, PLAIN_KEY);
    click_text(&mut app, "Validate");
    let ticket = backend.check_ticket();

    click_text(&mut app, "Local agent folder");
    let edited = screen(&mut app);
    assert!(!edited.contains("checking"), "{edited}");

    let outcome = key_outcome(&app);
    app.deliver(&checked(ticket, outcome));
    let late = screen(&mut app);
    assert!(
        !late.contains(SPARE_CANARY),
        "a reply to a check the form gave up is not shown:\n{late}"
    );
}

#[test]
fn a_reply_to_an_earlier_check_is_not_the_forms() {
    let (mut app, backend) = held(Scenario::AccountsMixed);
    press(&mut app, PLAIN_KEY);
    click_text(&mut app, "Validate");
    let first = backend.check_ticket();
    click_text(&mut app, "Local agent folder");
    click_text(&mut app, "Plain-text API key");
    press(&mut app, "tab tab tab tab tab");
    click_text(&mut app, "Validate");
    let second = backend.check_ticket();
    assert_ne!(first, second);

    let outcome = key_outcome(&app);
    app.deliver(&checked(first, outcome.clone()));
    let waiting = screen(&mut app);
    assert!(
        waiting.contains("… material   checking") && !waiting.contains(SPARE_CANARY),
        "the first reply does not end the second check:\n{waiting}"
    );

    app.deliver(&checked(second, outcome));
    assert!(screen(&mut app).contains(SPARE_CANARY));
}

#[test]
fn a_reply_after_the_form_closed_does_nothing() {
    let (mut app, backend) = held(Scenario::AccountsMixed);
    press(&mut app, PLAIN_KEY);
    click_text(&mut app, "Validate");
    let ticket = backend.check_ticket();
    press(&mut app, "esc");

    let outcome = key_outcome(&app);
    app.deliver(&checked(ticket, outcome));
    press(&mut app, "a");
    let fresh = screen(&mut app);
    assert!(fresh.contains("New account"), "{fresh}");
    assert!(
        !fresh.contains(SPARE_CANARY) && !fresh.contains("checking"),
        "a new form starts clean:\n{fresh}"
    );
    assert_eq!(backend.saves(), 0);
}

#[test]
fn save_waits_for_the_check_and_then_saves() {
    let (mut app, backend) = held(Scenario::AccountsMixed);
    press(&mut app, PLAIN_KEY);
    click_text(&mut app, "Save");

    assert_eq!(backend.checks().len(), 1);
    assert_eq!(backend.saves(), 0, "nothing is stored before the check");
    let waiting = screen(&mut app);
    assert!(waiting.contains("New account"), "{waiting}");
    assert!(waiting.contains("checking"), "{waiting}");

    let ticket = backend.check_ticket();
    let outcome = key_outcome(&app);
    app.deliver(&checked(ticket, outcome));
    ticks(&mut app, 4);
    assert_eq!(backend.saves(), 1);
    assert_eq!(status(&app), "Saved Claude · Spare");
    assert!(!screen(&mut app).contains("New account"));
}

#[test]
fn save_during_a_validate_saves_when_the_check_returns() {
    let (mut app, backend) = held(Scenario::AccountsMixed);
    press(&mut app, PLAIN_KEY);
    click_text(&mut app, "Validate");
    click_text(&mut app, "Save");
    assert_eq!(backend.checks().len(), 1, "one check serves both");
    assert_eq!(backend.saves(), 0);

    let ticket = backend.check_ticket();
    let outcome = key_outcome(&app);
    app.deliver(&checked(ticket, outcome));
    ticks(&mut app, 4);
    assert_eq!(backend.saves(), 1);
    assert_eq!(status(&app), "Saved Claude · Spare");
}

#[test]
fn a_rejected_check_keeps_the_form_open_and_stores_nothing() {
    let (mut app, backend) = held(Scenario::AccountsMixed);
    press(&mut app, PLAIN_KEY);
    click_text(&mut app, "Save");
    let ticket = backend.check_ticket();
    let mut rejected = key_outcome(&app);
    rejected.level = None;
    rejected.issue = Some(RecoverableIssue::new(
        IssueCode::ApiKeyEmpty,
        "API key required: the field is empty",
        Recoverability::ActionRequired,
    ));
    rejected.material = CheckRow::Failed("Field is empty".into());
    app.deliver(&checked(ticket, rejected));
    ticks(&mut app, 4);

    let shown = screen(&mut app);
    assert!(shown.contains("New account"), "{shown}");
    assert!(!shown.contains("checking"), "{shown}");
    assert!(shown.contains("! material   Field is empty"), "{shown}");
    assert!(
        shown.contains("API key required: the field is empty"),
        "{shown}"
    );
    assert_eq!(backend.saves(), 0);
}

/// "Spare", provider Claude, the local folder source and `~/.claude-spare`.
const FOLDER: &str =
    "c a enter S p a r e tab tab tab down tab enter ~ / . c l a u d e - s p a r e tab";

fn folder_outcome(app: &App) -> ValidationOutcome {
    let mut outcome = provider::validate(
        Provider::Anthropic,
        &CredentialSource::LocalFolder {
            path: "~/.claude-work".into(),
            detected: DetectedKind::Unknown,
        },
        None,
        app.world.op.as_ref(),
        app.world.now_secs(),
    );
    assert!(outcome.level.is_some(), "the simulator accepts this folder");
    outcome.identity = AccountIdentity::default();
    outcome
}

#[test]
fn saving_a_folder_asks_the_backend_what_is_in_it() {
    let (mut app, backend) = held(Scenario::AccountsMixed);
    press(&mut app, FOLDER);
    click_text(&mut app, "Save");

    let ticket = backend.check_ticket();
    assert!(
        backend
            .seen()
            .iter()
            .all(|e| !matches!(e, Effect::ProbeFolder { .. })),
        "the folder is probed after the check, not with it"
    );
    let outcome = folder_outcome(&app);
    app.deliver(&checked(ticket, outcome));

    let probes: Vec<_> = backend
        .seen()
        .into_iter()
        .filter(|e| matches!(e, Effect::ProbeFolder { .. }))
        .collect();
    let [Effect::ProbeFolder { path, .. }] = probes.as_slice() else {
        panic!("one ProbeFolder expected, got {:?}", backend.seen());
    };
    assert_eq!(path, "~/.claude-spare");
    assert_eq!(backend.saves(), 0, "nothing is stored before the probe");
    assert!(screen(&mut app).contains("New account"));

    let ticket = backend.probe_ticket();
    app.deliver(&Msg::FolderProbed {
        ticket,
        result: Ok(FolderProbe::Found(DetectedKind::CodexAuthJson)),
    });
    ticks(&mut app, 4);

    let Some(Effect::SaveAccounts { registry, .. }) = backend
        .seen()
        .into_iter()
        .find(|e| matches!(e, Effect::SaveAccounts { .. }))
    else {
        panic!("the account is stored once the folder is probed");
    };
    let spare = registry
        .accounts
        .iter()
        .find(|a| a.display_name == "Spare")
        .expect("the new account");
    assert!(
        matches!(
            &spare.source,
            CredentialSource::LocalFolder { path, detected }
                if path == "~/.claude-spare" && *detected == DetectedKind::CodexAuthJson
        ),
        "the kind comes from the probe reply: {:?}",
        spare.source
    );
}

#[test]
fn a_folder_probe_that_fails_keeps_the_form_open() {
    let (mut app, backend) = held(Scenario::AccountsMixed);
    press(&mut app, FOLDER);
    click_text(&mut app, "Save");
    let ticket = backend.check_ticket();
    let outcome = folder_outcome(&app);
    app.deliver(&checked(ticket, outcome));
    let ticket = backend.probe_ticket();
    app.deliver(&Msg::FolderProbed {
        ticket,
        result: Err("the disk is not answering".into()),
    });

    let shown = screen(&mut app);
    assert!(shown.contains("the disk is not answering"), "{shown}");
    assert!(shown.contains("New account"), "{shown}");
    assert_eq!(backend.saves(), 0);
}

fn view_changes(app: &mut App) {
    press(app, "f10");
    click_text(app, "View");
    click_text(app, "Inspect changes");
}

fn canary_changes() -> ChangeSet {
    ChangeSet {
        files: vec![ChangedFile {
            path: "src/canary.rs".into(),
            status: DiffStatus::Modified,
            hunks: vec![Hunk {
                old_start: 1,
                new_start: 1,
                lines: vec![DiffLine::add("let canary = 1;")],
            }],
        }],
        unpushed: 0,
    }
}

#[test]
fn inspect_changes_waits_for_the_backend() {
    let (mut app, backend) = held(Scenario::CapsuleMulti);
    view_changes(&mut app);

    let reads: Vec<_> = backend
        .seen()
        .into_iter()
        .filter(|e| matches!(e, Effect::ReadChanges { .. }))
        .collect();
    let [Effect::ReadChanges { instance, .. }] = reads.as_slice() else {
        panic!("one ReadChanges expected, got {:?}", backend.seen());
    };
    assert!(instance.starts_with("jk-"), "{instance}");
    assert_eq!(status(&app), "Reading changes…");
    assert!(!screen(&mut app).contains("Inspect changes ·"));

    view_changes(&mut app);
    let asked = backend
        .seen()
        .iter()
        .filter(|e| matches!(e, Effect::ReadChanges { .. }))
        .count();
    assert_eq!(asked, 1, "a pending read is not asked twice");

    let ticket = backend.read_ticket();
    app.deliver(&Msg::ChangesRead {
        ticket,
        result: Ok(canary_changes()),
    });
    let shown = screen(&mut app);
    assert!(shown.contains("Inspect changes ·"), "{shown}");
    assert!(shown.contains("canary.rs"), "{shown}");
}

#[test]
fn an_empty_changes_reply_says_nothing_changed() {
    let (mut app, backend) = held(Scenario::CapsuleMulti);
    view_changes(&mut app);
    let ticket = backend.read_ticket();
    app.deliver(&Msg::ChangesRead {
        ticket,
        result: Ok(ChangeSet {
            files: vec![],
            unpushed: 0,
        }),
    });
    assert_eq!(status(&app), "Nothing changed in this instance");
    assert!(!screen(&mut app).contains("Inspect changes ·"));
}

#[test]
fn a_failed_changes_read_shows_the_reply_error() {
    let (mut app, backend) = held(Scenario::CapsuleMulti);
    view_changes(&mut app);
    let ticket = backend.read_ticket();
    app.deliver(&Msg::ChangesRead {
        ticket,
        result: Err("git is not answering".into()),
    });
    assert_eq!(status(&app), "git is not answering");
    assert!(!screen(&mut app).contains("Inspect changes ·"));
    view_changes(&mut app);
    assert_eq!(
        backend
            .seen()
            .iter()
            .filter(|e| matches!(e, Effect::ReadChanges { .. }))
            .count(),
        2,
        "the next try asks again"
    );
}

fn open_picker(app: &mut App) {
    press(app, "a");
    click_text(app, "Choose…");
}

fn accounts(rows: &[(&str, &str, OpAccountState)]) -> OpListing {
    OpListing::Accounts(
        rows.iter()
            .map(|(id, email, state)| OpAccountRow {
                id: (*id).into(),
                email: (*email).into(),
                state: *state,
            })
            .collect(),
    )
}

#[test]
fn the_picker_loads_through_the_backend() {
    let (mut app, backend) = held(Scenario::AccountsMixed);
    open_picker(&mut app);

    assert_eq!(backend.op_scopes(), [OpScope::Accounts]);
    ticks(&mut app, 12);
    let waiting = screen(&mut app);
    assert!(
        waiting.contains("loading accounts…"),
        "the picker waits for the reply, not for the clock:\n{waiting}"
    );

    let ticket = backend.op_ticket();
    app.deliver(&Msg::OpLoaded {
        ticket,
        result: Ok(accounts(&[(
            "canary.1password.com",
            "canary@example.test",
            OpAccountState::Available,
        )])),
    });
    let listed = screen(&mut app);
    assert!(listed.contains("canary.1password.com"), "{listed}");
    assert!(!listed.contains("loading accounts"), "{listed}");
    assert!(!listed.contains("chainargos"), "{listed}");
}

#[test]
fn choosing_an_account_asks_for_its_vaults() {
    let (mut app, backend) = held(Scenario::AccountsMixed);
    open_picker(&mut app);
    let ticket = backend.op_ticket();
    app.deliver(&Msg::OpLoaded {
        ticket,
        result: Ok(accounts(&[(
            "canary.1password.com",
            "canary@example.test",
            OpAccountState::Available,
        )])),
    });
    press(&mut app, "enter");

    assert_eq!(
        backend.op_scopes(),
        [
            OpScope::Accounts,
            OpScope::Vaults {
                account: "canary.1password.com".into()
            }
        ]
    );
    let waiting = screen(&mut app);
    assert!(waiting.contains("loading vaults…"), "{waiting}");
    press(&mut app, "down");
    assert_eq!(
        backend.op_scopes().len(),
        2,
        "keys do not start another load while one is out"
    );
}

#[test]
fn a_locked_listing_offers_a_retry_that_asks_again() {
    let (mut app, backend) = held(Scenario::AccountsMixed);
    open_picker(&mut app);
    let ticket = backend.op_ticket();
    app.deliver(&Msg::OpLoaded {
        ticket,
        result: Err(OpError::Locked),
    });
    let locked = screen(&mut app);
    assert!(locked.contains("1Password is locked"), "{locked}");

    press(&mut app, "r");
    assert_eq!(backend.op_scopes(), [OpScope::Accounts, OpScope::Accounts]);
    assert!(screen(&mut app).contains("loading accounts…"));
}

#[test]
fn a_listing_the_picker_has_left_behind_is_not_shown() {
    let (mut app, backend) = held(Scenario::AccountsMixed);
    open_picker(&mut app);
    let first = backend.op_ticket();
    app.deliver(&Msg::OpLoaded {
        ticket: first,
        result: Ok(accounts(&[(
            "canary.1password.com",
            "canary@example.test",
            OpAccountState::Available,
        )])),
    });
    press(&mut app, "enter");
    let vaults = backend.op_ticket();
    press(&mut app, "esc");
    let accounts_again = backend.op_ticket();
    assert_ne!(vaults, accounts_again);

    app.deliver(&Msg::OpLoaded {
        ticket: vaults,
        result: Ok(OpListing::Vaults(vec![])),
    });
    let shown = screen(&mut app);
    assert!(
        shown.contains("loading accounts…"),
        "the old reply must not end the new load:\n{shown}"
    );
}

#[test]
fn the_sim_still_lists_the_picker_after_260_ms() {
    let mut app = timed(Scenario::AccountsMixed, world_for(Scenario::AccountsMixed));
    open_picker(&mut app);
    let start = app.world.clock.now_ms;
    let mut elapsed = 0;
    while screen(&mut app).contains("loading accounts…") && elapsed < 2_000 {
        ticks(&mut app, 1);
        elapsed = app.world.clock.now_ms - start;
    }
    assert_eq!(
        elapsed, 320,
        "260 ms of latency, delivered on the tick after"
    );
    let listed = screen(&mut app);
    assert!(listed.contains("chainargos.1password.com"), "{listed}");
    assert!(listed.contains("acme.1password.com"), "{listed}");
}

#[test]
fn the_sim_checks_a_typed_key_on_the_spot() {
    let mut app = timed(Scenario::AccountsMixed, world_for(Scenario::AccountsMixed));
    press(&mut app, PLAIN_KEY);
    click_text(&mut app, "Validate");
    let done = screen(&mut app);
    assert!(done.contains("✓ material   Key material present"), "{done}");
    assert!(!done.contains("checking"), "{done}");
    assert_eq!(
        status(&app),
        "Validated · Save stores the reference, never the secret"
    );
}

#[test]
fn the_sim_saves_a_folder_on_the_spot_with_the_kind_it_probes() {
    let mut app = timed(Scenario::AccountsMixed, world_for(Scenario::AccountsMixed));
    press(
        &mut app,
        "c a enter S p a r e tab tab tab down tab enter ~ / . c l a u d e - w o r k tab",
    );
    click_text(&mut app, "Save");
    assert_eq!(status(&app), "Saved Claude · Spare");
    let spare = app
        .world
        .accounts
        .accounts
        .iter()
        .find(|a| a.display_name == "Spare")
        .expect("the saved account");
    assert!(
        matches!(
            &spare.source,
            CredentialSource::LocalFolder { detected, .. }
                if *detected == DetectedKind::ClaudeApiKeyEnv
        ),
        "{:?}",
        spare.source
    );
}

#[test]
fn the_sim_answers_a_changes_read_on_the_spot() {
    let mut app = timed(Scenario::CapsuleMulti, world_for(Scenario::CapsuleMulti));
    view_changes(&mut app);
    let shown = screen(&mut app);
    assert!(shown.contains("Inspect changes ·"), "{shown}");
}

fn until_loaded(app: &mut App) {
    for _ in 0..30 {
        if !screen(app).contains("loading ") {
            return;
        }
        ticks(app, 1);
    }
    panic!("the picker never loaded:\n{}", screen(app));
}

#[test]
fn the_sim_holds_a_1password_check_for_900_ms() {
    let mut app = timed(Scenario::AccountsMixed, world_for(Scenario::AccountsMixed));
    open_picker(&mut app);
    until_loaded(&mut app);
    press(&mut app, "enter");
    until_loaded(&mut app);
    press(&mut app, "enter");
    until_loaded(&mut app);
    press(&mut app, "W o r k");
    press(&mut app, "enter");
    until_loaded(&mut app);
    press(&mut app, "enter");
    let chosen = screen(&mut app);
    assert!(chosen.contains("Anthropic · Work"), "{chosen}");

    click_text(&mut app, "Validate");
    let start = app.world.clock.now_ms;
    let pending = screen(&mut app);
    assert!(pending.contains("… material   checking"), "{pending}");
    ticks(&mut app, 5);
    assert!(
        screen(&mut app).contains("… material   checking"),
        "still waiting after 400 ms"
    );
    while screen(&mut app).contains("checking") && app.world.clock.now_ms - start < 3_000 {
        ticks(&mut app, 1);
    }
    let elapsed = app.world.clock.now_ms - start;
    assert!(
        (900..980).contains(&elapsed),
        "900 ms, delivered on the tick after: {elapsed}"
    );
    let done = screen(&mut app);
    assert!(
        done.contains("✓ material   1Password reference resolved"),
        "{done}"
    );
}
