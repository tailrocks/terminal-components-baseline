#![cfg(feature = "demo")]

mod common;

use common::*;
use jackin_console::app::{App, Route};
use jackin_console::domain::vocabulary::Vocabulary;
use jackin_console::domain::workspace::WizardDefaults;
use jackin_console::fixtures::world_for;
use jackin_console::scenario::Scenario;

fn live() -> App {
    let mut world = world_for(Scenario::Returning);
    world.surfaces.vocabulary = Vocabulary::openshell();
    world.surfaces.wizard = Some(WizardDefaults {
        destination: "/sandbox/project".into(),
        destination_label: "Project folder".into(),
    });
    app_with(world)
}

#[test]
fn the_demo_wizard_keeps_its_steps_and_defaults() {
    let mut app = app(Scenario::Returning);
    press(&mut app, "n space");
    let step2 = screen(&mut app);
    assert!(step2.contains("Mount destination"), "{step2}");
    assert!(step2.contains("(●) Same path   /Users/alexey/src/payments-platform"));
    assert!(step2.contains("( ) Edit destination…"));
    press(&mut app, "enter");
    let step4 = screen(&mut app);
    assert!(step4.contains("~/src/payments-platform/crates"), "{step4}");
    press(&mut app, "enter");
    let step5 = screen(&mut app);
    assert!(step5.contains("step 5 of 5 · Name"));
    assert!(step5.contains("▎ payments-platform"), "{step5}");
}

#[test]
fn the_wizard_data_offers_one_destination() {
    let mut app = live();
    press(&mut app, "n space");
    let step2 = screen(&mut app);
    assert!(step2.contains("Project destination"), "{step2}");
    assert!(step2.contains("(●) Project folder /sandbox/project"));
    assert!(!step2.contains("Edit destination…") && !step2.contains("Same path"));
}

#[test]
fn the_wizard_data_skips_the_edit_step_and_lists_one_working_dir() {
    let mut app = live();
    press(&mut app, "n space enter");
    let step4 = screen(&mut app);
    assert!(step4.contains("step 4 of 5 · Working dir"), "{step4}");
    assert!(step4.contains("▪ /sandbox/project"));
    assert!(step4.contains("destination"));
    assert!(!step4.contains("crates") && !step4.contains("docs"));
}

#[test]
fn the_wizard_data_names_the_workspace_from_the_source_folder() {
    let mut app = live();
    press(&mut app, "n space enter enter");
    let step5 = screen(&mut app);
    assert!(step5.contains("step 5 of 5 · Name"), "{step5}");
    assert!(step5.contains("▎ payments-platform"), "{step5}");
    assert!(!step5.contains("▎ project"));
}

#[test]
fn the_wizard_data_names_a_second_folder_by_its_own_basename() {
    let mut app = live();
    press(&mut app, "n down space enter enter");
    let step5 = screen(&mut app);
    assert!(step5.contains("step 5 of 5 · Name"), "{step5}");
    assert!(step5.contains("▎ crates"), "{step5}");
}

#[test]
fn the_wizard_data_creates_the_workspace_at_the_destination() {
    let mut app = live();
    press(&mut app, "n down space enter enter enter");
    assert_eq!(app.route(), Route::Editor);
    assert!(screen(&mut app).contains("crates"));
    click_text(&mut app, "Project");
    let project = screen(&mut app);
    assert!(project.contains("Project"), "{project}");
    assert!(project.contains("/sandbox/project"), "{project}");
    assert!(!project.contains("Working directory"), "{project}");
}
