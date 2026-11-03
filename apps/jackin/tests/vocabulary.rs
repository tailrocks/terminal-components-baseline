#![cfg(feature = "fixtures")]

mod common;

use common::*;
use jackin_console::domain::vocabulary::Vocabulary;
use jackin_console::fixtures::world_for;
use jackin_console::scenario::Scenario;

fn pair(scenario: Scenario, keys: &str) -> (String, String) {
    let mut demo = app(scenario);
    press(&mut demo, keys);
    let mut world = world_for(scenario);
    world.surfaces.vocabulary = Vocabulary::openshell();
    let mut live = app_with(world);
    press(&mut live, keys);
    (screen(&mut demo), screen(&mut live))
}

#[test]
fn the_openshell_words_avoid_the_false_strings() {
    for (name, text) in Vocabulary::openshell().fields() {
        for banned in ["on the host", "Opened", "Revealed", "Capsule"] {
            assert!(!text.contains(banned), "{name} says {banned}: {text}");
        }
    }
}

#[test]
fn the_openshell_vocabulary_renames_the_demo_words() {
    const SAME: [&str; 3] = [
        "status.preserved_dirty",
        "status.preserved_unpushed",
        "status.superseded",
    ];
    let demo = Vocabulary::default();
    let openshell = Vocabulary::openshell();
    let (demo, live) = (demo.fields(), openshell.fields());
    assert_eq!(
        demo.iter().map(|f| f.0).collect::<Vec<_>>(),
        live.iter().map(|f| f.0).collect::<Vec<_>>(),
    );
    for ((name, was), (_, now)) in demo.iter().zip(&live) {
        if SAME.contains(name) {
            assert_eq!(was, now, "{name}");
        } else {
            assert_ne!(was, now, "{name}");
        }
    }
}

#[test]
fn the_manager_card_names_the_project() {
    let (demo, live) = pair(Scenario::Returning, "down");
    assert!(demo.contains("Mounts") && !demo.contains("Project "));
    assert!(demo.contains("~/src/payments-platform · rw "));
    assert!(live.contains("Project ") && !live.contains("Mounts"));
    assert!(live.contains("~/src/payments-platform · uploaded"));
}

#[test]
fn the_status_strip_names_the_world() {
    let (demo, live) = pair(Scenario::Returning, "");
    assert!(demo.contains("inside the Construct"));
    assert!(live.contains("inside the sandbox") && !live.contains("Construct"));
}

#[test]
fn the_first_use_hint_follows_the_words() {
    let (demo, live) = pair(Scenario::FirstUse, "");
    assert!(demo.contains("is mounted at its own path inside the Construct"));
    assert!(live.contains("is uploaded to /sandbox/project"));
    assert!(!live.contains("mounted"));
}

#[test]
fn the_editor_tab_follows_the_words() {
    let (demo, live) = pair(Scenario::Returning, "down e");
    assert!(demo.contains("General    Mounts    Roles"));
    assert!(live.contains("General    Project    Roles") && !live.contains("Mounts"));
}

#[test]
fn the_github_status_follows_the_words() {
    let (demo, live) = pair(Scenario::Returning, "down o");
    assert!(demo.contains("Opened https://github.com/chainargos/payments-platform on the host"));
    assert!(live.contains("GitHub · https://github.com/chainargos/payments-platform"));
    assert!(!live.contains("Opened"));
}

#[test]
fn the_instance_status_text_follows_the_words() {
    let (demo, live) = pair(Scenario::Returning, "down right down");
    assert!(demo.contains("running · Container and Capsule daemon are live."));
    assert!(live.contains("running · The sandbox is running."));
    assert!(!live.contains("Capsule"));
}

#[test]
fn the_trailer_sample_follows_the_words() {
    let (demo, live) = pair(Scenario::Returning, "s");
    assert!(demo.contains("Signed-off-by: Alexey Zhokhov <alexey@chainargos.com>"));
    assert!(live.contains("None · jackin adds no trailers to commits"));
    assert!(!live.contains("Alexey"));
}

#[test]
fn the_trust_open_status_follows_the_words() {
    let (demo, live) = pair(Scenario::Returning, "s 5 tab o");
    assert!(demo.contains("Opened https://github.com/chainargos/roles in the browser"));
    assert!(live.contains("Source · github.com/chainargos/roles"));
    assert!(!live.contains("Opened"));
}

#[test]
fn the_env_form_wording_follows_the_words() {
    let (demo, live) = pair(Scenario::Returning, "s 3 tab a");
    assert!(
        demo.contains("Plain text  (masked)") && demo.contains("typed once · masked afterwards")
    );
    assert!(live.contains("(●) Plain text ") && live.contains("value is stored as plain text"));
    assert!(!live.contains("masked"));
}

#[test]
fn the_wizard_source_step_follows_the_words() {
    let (demo, live) = pair(Scenario::Returning, "n");
    assert!(demo.contains("[ ] Mount read-only"));
    assert!(live.contains("[ ] Read-only") && !live.contains("Mount read-only"));
}

#[test]
fn the_git_url_help_follows_the_words() {
    let (demo, live) = pair(Scenario::Returning, "n g");
    assert!(demo.contains("The repository is cloned into the"));
    assert!(live.contains("jackin uploads a folder from this machine."));
    assert!(live.contains("Clone the repository first, then choose its folder."));
}

#[test]
fn the_session_choice_reason_follows_the_words() {
    let mut demo = world_for(Scenario::Returning);
    let mut live = world_for(Scenario::Returning);
    live.surfaces.vocabulary = Vocabulary::openshell();
    let provider = jackin_console::domain::agent::Agent::Codex.provider();
    for world in [&mut demo, &mut live] {
        let session = world
            .accounts
            .by_provider(provider)
            .next()
            .map(|a| a.id.clone())
            .expect("a Codex account");
        let why = world.account_for(provider, None, None, Some(&session)).why;
        assert_eq!(why, world.surfaces.vocabulary.session_choice);
    }
    assert!(
        demo.surfaces
            .vocabulary
            .session_choice
            .ends_with("in Capsule")
    );
    assert!(
        live.surfaces
            .vocabulary
            .session_choice
            .ends_with("in the terminal")
    );
}

#[test]
fn the_mount_open_status_follows_the_words() {
    let (demo, live) = pair(Scenario::Returning, "down down down down e 2 down o");
    assert!(demo.contains("Opened ~/src/customer-portal on GitHub"));
    assert!(live.contains("GitHub · ~/src/customer-portal"));
    assert!(!live.contains("Opened"));
}

#[test]
fn the_editor_workdir_hint_follows_the_words() {
    let (demo, live) = pair(Scenario::Returning, "down e");
    assert!(demo.contains("Inside the Construct"));
    assert!(live.contains("Inside the sandbox") && !live.contains("Construct"));
}
