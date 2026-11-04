//! Step BG6: the account form offers an environment-variable source under the
//! openshell surfaces. Each test states one case in terms of what the
//! operator sees or what the backend is asked. The cases are in
//! `docs/P9.3-red-green.md`, section BG6.
#![cfg(feature = "demo")]

mod common;

use std::sync::{Arc, Mutex};

use common::*;
use jackin_console::app::App;
use jackin_console::backend::{Backend, Completion, Effect, FileSource};
use jackin_console::domain::account::{Account, AccountForm, CredentialSource, DetectedKind};
use jackin_console::domain::agent::Provider;
use jackin_console::domain::instance::{DaemonSnapshot, Instance};
use jackin_console::domain::launch::LaunchContent;
use jackin_console::domain::reason::IssueReason;
use jackin_console::domain::vocabulary::Vocabulary;
use jackin_console::fixtures::{Scenario, app_with_world, world_for};
use jackin_console::launch::{LaunchFeed, LaunchTarget};
use jackin_console::panes::{PaneBody, PaneRequest, PaneSpawn};
use jackin_console::sim::SimBackend;
use jackin_console::world::{Msg, World};
use junie_tui::core::event::Input;
use junie_tui::motion::Motion;
use junie_tui::runtime::Application;
use junie_tui::theme::{ColorLevel, Theme};

/// The simulator, with every effect recorded.
#[derive(Debug)]
struct Recording {
    inner: Arc<dyn Backend>,
    seen: Mutex<Vec<Effect>>,
}

impl Recording {
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

    fn saved(&self) -> Vec<Account> {
        self.seen()
            .iter()
            .filter_map(|e| match e {
                Effect::SaveAccounts { registry, .. } => Some(registry.accounts.clone()),
                _ => None,
            })
            .next_back()
            .unwrap_or_default()
    }
}

impl Backend for Recording {
    fn perform(&self, effect: &Effect, world: &World) -> Option<Completion> {
        self.seen.lock().expect("seen").push(effect.clone());
        self.inner.perform(effect, world)
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

fn live_world() -> World {
    let mut world = world_for(Scenario::AccountsMixed);
    world.surfaces.account_form = AccountForm::openshell();
    world.surfaces.vocabulary = Vocabulary::openshell();
    world
}

fn recorded(mut world: World) -> (App, Arc<Recording>) {
    let backend = Arc::new(Recording {
        inner: SimBackend::shared(),
        seen: Mutex::new(vec![]),
    });
    world.backend = backend.clone();
    let app = app_with_world(
        Scenario::AccountsMixed,
        world,
        Motion::Reduced,
        10_000,
        Theme::for_level(ColorLevel::TrueColor),
    );
    (app, backend)
}

fn ticks(app: &mut App, n: usize) {
    for _ in 0..n {
        Application::handle(app, Input::Tick);
    }
}

fn status(app: &App) -> String {
    app.shell.status.text().unwrap_or_default().to_owned()
}

fn word(name: &str) -> String {
    Vocabulary::openshell()
        .fields()
        .into_iter()
        .find(|(field, _)| *field == name)
        .unwrap_or_else(|| panic!("the vocabulary has no word {name}"))
        .1
        .to_owned()
}

fn row_of<'a>(shown: &'a str, text: &str) -> &'a str {
    shown
        .lines()
        .find(|l| l.contains(text))
        .unwrap_or_else(|| panic!("{text} not on screen:\n{shown}"))
}

/// The new-account form of the openshell surfaces, with the environment
/// variable source chosen and the cursor in its name field.
fn env_form(app: &mut App) {
    press(app, "c a");
    click_text(app, "Environment variable");
    press(app, "tab");
}

fn type_name(app: &mut App, name: &str) {
    press(app, "enter");
    for c in name.chars() {
        let key = if c == ' ' {
            "space".into()
        } else {
            c.to_string()
        };
        press(app, &key);
    }
    press(app, "tab");
}

fn name_the_account(app: &mut App, name: &str) {
    click_text(app, "Display name");
    press(app, "enter");
    for c in name.chars() {
        press(app, &c.to_string());
    }
    press(app, "tab");
}

#[test]
fn the_openshell_form_offers_an_environment_variable_source() {
    let mut app = app_with_world(
        Scenario::AccountsMixed,
        live_world(),
        Motion::Paused,
        10_000,
        Theme::for_level(ColorLevel::TrueColor),
    );
    press(&mut app, "c a");
    let shown = screen(&mut app);
    assert!(
        row_of(&shown, "Environment variable").contains("( ) Environment variable"),
        "{shown}"
    );
    let rows: Vec<&str> = shown.lines().collect();
    let at = |text: &str| rows.iter().position(|r| r.contains(text)).expect(text);
    assert!(
        at("Plain-text API key") < at("Environment variable"),
        "the new source follows the plain-text key:\n{shown}"
    );
}

#[test]
fn the_demo_form_keeps_its_three_sources() {
    let mut app = app(Scenario::AccountsMixed);
    press(&mut app, "c a");
    let shown = screen(&mut app);
    assert!(!shown.contains("Environment variable"), "{shown}");
    for label in [
        "1Password item / field  (recommended)",
        "Local agent folder",
        "Plain-text API key",
    ] {
        assert!(shown.contains(label), "{label}:\n{shown}");
    }
    let form = AccountForm::default();
    assert_eq!(form.sources.len(), 3);
}

#[test]
fn choosing_it_asks_for_a_name_and_hides_the_other_sources() {
    let mut app = app_with_world(
        Scenario::AccountsMixed,
        live_world(),
        Motion::Paused,
        10_000,
        Theme::for_level(ColorLevel::TrueColor),
    );
    press(&mut app, "c a");
    let before = screen(&mut app);
    assert!(
        !before.contains(&word("source.env_placeholder")),
        "{before}"
    );
    click_text(&mut app, "Environment variable");
    let shown = screen(&mut app);
    assert!(
        row_of(&shown, "Environment variable").contains("(●) Environment variable"),
        "{shown}"
    );
    assert!(shown.contains(&word("source.env_placeholder")), "{shown}");
    assert!(!shown.contains("Browse…"), "{shown}");
    assert!(!shown.contains("paste the key"), "{shown}");
    assert!(!shown.contains("Choose…"), "{shown}");
}

#[test]
fn validate_asks_the_backend_to_check_the_name_and_sends_no_value() {
    let (mut app, backend) = recorded(live_world());
    env_form(&mut app);
    type_name(&mut app, " ZAI_API_KEY ");
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
    assert_eq!(
        *source,
        CredentialSource::HostEnv {
            var: "ZAI_API_KEY".into(),
            detected: DetectedKind::Unknown,
        }
    );
    assert!(key.is_none(), "a name is checked, never a typed value");
}

#[test]
fn an_empty_name_is_refused_before_the_backend_is_asked() {
    let (mut app, backend) = recorded(live_world());
    env_form(&mut app);
    click_text(&mut app, "Validate");
    let shown = screen(&mut app);
    assert!(shown.contains(&word("source.env_empty")), "{shown}");
    assert!(backend.checks().is_empty(), "{:?}", backend.seen());
    assert!(shown.contains("New account"), "the form stays open");
}

fn checked_row(name: &str) -> String {
    let (mut app, _) = recorded(live_world());
    env_form(&mut app);
    type_name(&mut app, name);
    click_text(&mut app, "Validate");
    ticks(&mut app, 4);
    screen(&mut app)
}

#[test]
fn the_check_words_a_set_missing_empty_and_invalid_variable() {
    let set = checked_row("ZAI_API_KEY");
    assert!(
        set.contains("✓ material   ZAI_API_KEY is set"),
        "a set variable passes:\n{set}"
    );
    let missing = checked_row("NOT_SET_ANYWHERE");
    assert!(
        missing.contains("! material   NOT_SET_ANYWHERE is not set"),
        "{missing}"
    );
    let empty = checked_row("JACKIN_EMPTY_KEY");
    assert!(
        empty.contains("! material   JACKIN_EMPTY_KEY is empty"),
        "{empty}"
    );
}

const TYPED: &str = "sk-ant-secret-1234";

#[test]
fn an_invalid_name_is_worded_without_the_name() {
    let words = Vocabulary::openshell();
    let reason = IssueReason::EnvInvalid { name: TYPED.into() };
    assert!(
        !words.reason(&reason).contains(TYPED),
        "{}",
        words.reason(&reason)
    );
    assert!(
        !words.check(&reason).contains(TYPED),
        "{}",
        words.check(&reason)
    );
    assert_eq!(
        words.reason(&reason),
        "Not a valid variable name · letters, digits and _ only"
    );
}

#[test]
fn a_pasted_key_is_not_echoed_by_the_check() {
    let invalid = checked_row(TYPED);
    assert!(
        invalid.contains(&format!(
            "! material   {}",
            word("reason.env_invalid.check")
        )),
        "{invalid}"
    );
    assert_eq!(
        invalid.matches(TYPED).count(),
        1,
        "only the field the operator typed in shows it:\n{invalid}"
    );
}

#[test]
fn saving_stores_the_name_as_a_host_variable_account() {
    let (mut app, backend) = recorded(live_world());
    env_form(&mut app);
    type_name(&mut app, "ANTHROPIC_API_KEY");
    name_the_account(&mut app, "Envy");
    click_text(&mut app, "Save");
    ticks(&mut app, 6);

    assert_eq!(status(&app), "Saved Claude · Envy", "{}", screen(&mut app));
    let saved = backend.saved();
    let account = saved
        .iter()
        .find(|a| a.display_name == "Envy")
        .unwrap_or_else(|| panic!("no account Envy was saved: {:?}", backend.seen()));
    assert_eq!(
        account.source,
        CredentialSource::HostEnv {
            var: "ANTHROPIC_API_KEY".into(),
            detected: DetectedKind::Unknown,
        }
    );
    assert!(
        !backend
            .seen()
            .iter()
            .any(|e| matches!(e, Effect::ProbeFolder { .. })),
        "a variable has no folder to probe"
    );
}

#[test]
fn a_variable_another_account_of_the_provider_reads_is_a_duplicate() {
    let mut world = live_world();
    world.accounts.insert(Account::registered(
        "reg-env",
        "Taken",
        Provider::Anthropic,
        CredentialSource::HostEnv {
            var: "ANTHROPIC_API_KEY".into(),
            detected: DetectedKind::Unknown,
        },
    ));
    let (mut app, backend) = recorded(world);
    env_form(&mut app);
    type_name(&mut app, "ANTHROPIC_API_KEY");
    name_the_account(&mut app, "Envy");
    click_text(&mut app, "Save");
    ticks(&mut app, 6);

    let shown = screen(&mut app);
    assert!(
        shown.contains("Already registered: this source is used by"),
        "{shown}"
    );
    assert!(
        backend.saved().iter().all(|a| a.display_name != "Envy"),
        "nothing is stored"
    );
}

#[test]
fn editing_a_host_variable_account_opens_on_its_source_and_name() {
    let mut world = live_world();
    world.accounts.insert(Account::registered(
        "reg-env",
        "Taken",
        Provider::Anthropic,
        CredentialSource::HostEnv {
            var: "ANTHROPIC_API_KEY".into(),
            detected: DetectedKind::Unknown,
        },
    ));
    let (mut app, _) = recorded(world);
    press(&mut app, "c");
    click_text(&mut app, "Taken");
    press(&mut app, "e");
    let shown = screen(&mut app);
    assert!(shown.contains("Edit account"), "{shown}");
    assert!(
        row_of(&shown, "Environment variable").contains("(●) Environment variable"),
        "{shown}"
    );
    assert!(shown.contains("ANTHROPIC_API_KEY"), "{shown}");
}

fn roster_card(vocabulary: Vocabulary, daemon: DaemonSnapshot) -> String {
    let mut world = world_for(Scenario::Returning);
    world.surfaces.vocabulary = vocabulary;
    let id = world.running()[0].id.clone();
    world.daemons.remove(&id);
    world.instance_mut(&id).expect("instance").daemon = daemon;
    let mut app = app_with(Scenario::Returning, world);
    screen_at(&mut app, 170, 40)
}

#[test]
fn a_running_sandbox_without_tabs_is_not_called_unreachable() {
    let live = roster_card(Vocabulary::openshell(), DaemonSnapshot::NoTabs);
    assert!(live.contains("· no Capsule tabs open"), "{live}");
    assert!(!live.contains("· gateway unreachable"), "{live}");
}

#[test]
fn an_unreachable_gateway_still_reads_unreachable_on_the_card() {
    let live = roster_card(Vocabulary::openshell(), DaemonSnapshot::Unavailable);
    assert!(live.contains("· gateway unreachable"), "{live}");
    assert!(!live.contains("no Capsule tabs open"), "{live}");
}

#[test]
fn the_demo_card_reads_as_it_did_for_a_terminal_without_tabs() {
    let demo = roster_card(Vocabulary::demo(), DaemonSnapshot::NoTabs);
    assert!(demo.contains("· daemon unavailable"), "{demo}");
}
