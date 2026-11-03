#![cfg(feature = "fixtures")]

mod common;

use common::*;
use jackin_console::app::App;
use jackin_console::domain::fixed::Fixed;
use jackin_console::domain::vocabulary::Vocabulary;
use jackin_console::domain::workspace::{is_credential_reference, looks_secret};
use jackin_console::fixtures::world_for;
use jackin_console::scenario::{Motion, Scenario};
use junie_tui::core::event::Input;
use junie_tui::runtime::Application;
use junie_tui::theme::{ColorLevel, Theme};
use junie_tui::ui::text::width;

fn fixed() -> App {
    let scenario = Scenario::OpenshellFixed;
    App::for_scenario(
        scenario,
        world_for(scenario),
        Motion::Reduced,
        10_000,
        Theme::for_level(ColorLevel::TrueColor),
    )
}

fn footer(screen: &str) -> String {
    screen.lines().last().unwrap_or_default().to_owned()
}

fn tick(app: &mut App, times: usize) -> String {
    for _ in 0..times {
        Application::handle(app, Input::Tick);
    }
    screen(app)
}

fn paste(app: &mut App, text: &str) {
    Application::handle(app, Input::Paste(text.into()));
    screen(app);
}

fn settings(app: &mut App, tab: &str) {
    press(app, "s");
    let tabs = ["General", "Mounts", "Environments", "Agents", "Trust"];
    let at = tabs.iter().position(|t| *t == tab).expect("a tab");
    for _ in 0..at {
        press(app, "]");
    }
    press(app, "enter");
}

#[test]
fn every_fixed_text_fits_the_surface_that_shows_it() {
    let f = Fixed::openshell();
    let reasons = [
        &f.keep_awake,
        &f.git_pull,
        &f.dirty_exit,
        &f.mounts,
        &f.workdir,
        &f.enable_workspace,
        &f.commits,
        &f.trust,
        &f.env_sources,
        &f.role_env,
        &f.agent_launchable,
        &f.agent_other,
    ];
    for reason in reasons {
        assert!(reason.within_limits(), "{}", reason.text);
    }
    let form_lines = f
        .agent_notes
        .iter()
        .chain(&f.env_secret_note)
        .map(String::as_str);
    for line in form_lines {
        assert!(width(line) <= 62, "{line}");
    }
    assert!(width(&f.env_reference) <= 62, "{}", f.env_reference);
    assert!(width(&f.last_role) <= 60);
    let words = Vocabulary::openshell();
    assert!(width(&words.trailer_sample) <= 60);
    assert!(
        width(&f.env_secret_note.join(" ")) <= 64,
        "the two lines are one sentence pair"
    );
}

#[test]
fn a_credential_reference_is_refused_in_any_letter_case_after_leading_space() {
    let refused = [
        "op://v/i/f",
        " op://v/i/f",
        "\t Op://x",
        "OP://V/I/F",
        "\u{feff}op://x",
        "openshell:resolve:env:X",
        " OPENSHELL:RESOLVE:x",
    ];
    for value in refused {
        assert!(is_credential_reference(value), "{value:?}");
    }
    let kept = [
        "ghp_abc",
        "sk-live-1",
        "-----BEGIN PRIVATE KEY-----",
        "\"op://x\"",
        "see op://x",
        "https://op.example",
        "o",
        "é",
        "",
        "openshell:resolve",
        "op:/",
    ];
    for value in kept {
        assert!(!is_credential_reference(value), "{value:?}");
    }
}

#[test]
fn only_a_name_with_a_secret_word_gets_the_note() {
    for name in [
        "DEPLOY_TOKEN",
        "db_password",
        "Client_Secret",
        "SSH_KEY",
        "APIKEY",
    ] {
        assert!(looks_secret(name), "{name}");
    }
    for name in ["EDITOR", "CARGO_NET_GIT_FETCH_WITH_CLI", "HOME_DIR", "TOKE"] {
        assert!(!looks_secret(name), "{name}");
    }
}

#[test]
fn a_trust_source_stays_until_the_next_key_and_a_refusal_expires() {
    let mut app = fixed();
    settings(&mut app, "Trust");
    press(&mut app, "o");
    let shown = tick(&mut app, 400);
    assert!(
        footer(&shown).contains("Source · github.com/chainargos/roles"),
        "{shown}"
    );
    press(&mut app, "down");
    assert!(!footer(&screen(&mut app)).contains("Source ·"));

    press(&mut app, "space");
    let refused = screen(&mut app);
    let reason = Fixed::openshell().trust.text;
    assert!(footer(&refused).contains(&reason), "{refused}");
    let later = tick(&mut app, 400);
    assert!(!footer(&later).contains(&reason), "{later}");
}

#[test]
fn the_demo_settings_keep_every_control() {
    let mut app = app(Scenario::Returning);
    settings(&mut app, "General");
    let general = screen(&mut app);
    assert!(general.contains("Two independent flags; both apply to every Workspace."));
    assert!(footer(&general).contains("Space Toggle"));
    press(&mut app, "space");
    let toggled = screen(&mut app);
    assert!(toggled.contains("• 1 change"), "{toggled}");
    assert!(toggled.contains("[ ] Add Co-authored-by trailer"));

    press(&mut app, "esc");
    press(&mut app, "]");
    press(&mut app, "]");
    press(&mut app, "]");
    press(&mut app, "]");
    press(&mut app, "enter");
    let trust = screen(&mut app);
    assert!(trust.contains("An untrusted source blocks + Load role"));
    assert!(footer(&trust).contains("Space Toggle trust"));
    press(&mut app, "space");
    let trusted = screen(&mut app);
    assert!(trusted.contains("• 2 changes"), "{trusted}");
    assert!(footer(&trusted).contains("trusted · save to apply"));

    press(&mut app, "esc");
    press(&mut app, "[");
    press(&mut app, "enter");
    let agents = screen(&mut app);
    assert!(agents.contains("sync mirrors the host login"));
    assert!(footer(&agents).contains("Space Cycle mode"));
    press(&mut app, "space");
    let cycled = screen(&mut app);
    assert!(cycled.contains("• 3 changes"), "{cycled}");
}

#[test]
fn the_demo_env_form_keeps_its_sources_and_saves_what_it_is_given() {
    let mut app = app(Scenario::Returning);
    settings(&mut app, "Environments");
    press(&mut app, "p");
    let flow = screen(&mut app);
    assert!(flow.contains("1Password"), "{flow}");
    press(&mut app, "esc");
    press(&mut app, "a");
    let form = screen(&mut app);
    assert!(form.contains("1Password item / field  (recommended for secrets)"));
    assert!(form.contains("Host environment variable"));
    assert!(!form.contains("Plain values only"));
    press(&mut app, "tab");
    press(&mut app, "down");
    press(&mut app, "space");
    let chosen = screen(&mut app);
    assert!(chosen.contains("(●) 1Password item / field"), "{chosen}");
    assert!(!footer(&chosen).contains("Plain values only"));
}

#[test]
fn the_demo_env_form_keeps_an_empty_value_open_with_its_message() {
    let mut app = app(Scenario::Returning);
    settings(&mut app, "Environments");
    press(&mut app, "a");
    paste(&mut app, "NEWVAR");
    press(&mut app, "enter");
    click_text(&mut app, "Save");
    let form = screen(&mut app);
    assert!(form.contains("New global environment key"), "{form}");
    assert!(form.contains("Value cannot be empty"), "{form}");
    assert!(form.contains("NEWVAR"), "the typed key stays:\n{form}");
}

#[test]
fn the_demo_env_form_saves_a_plain_value_that_starts_with_op() {
    let mut app = app(Scenario::Returning);
    settings(&mut app, "Environments");
    press(&mut app, "a");
    paste(&mut app, "NEWVAR");
    press(&mut app, "tab tab");
    paste(&mut app, "op://vault/item/field");
    press(&mut app, "enter");
    click_text(&mut app, "Save");
    let table = screen(&mut app);
    assert!(!table.contains("New global environment key"), "{table}");
    assert!(
        footer(&table).contains("NEWVAR · added · save to apply"),
        "{table}"
    );
}
