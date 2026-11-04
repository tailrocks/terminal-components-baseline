//! Step BG6: the words of the environment-variable source come from the
//! world's `Vocabulary`, not from the screen code. Section BG6 of
//! `docs/P9.3-red-green.md`.
#![cfg(feature = "demo")]

mod common;

use common::*;
use jackin_console::domain::account::AccountForm;
use jackin_console::domain::vocabulary::Vocabulary;
use jackin_console::fixtures::{Scenario, world_for};

fn canary_world() -> jackin_console::world::World {
    let mut world = world_for(Scenario::AccountsMixed);
    world.surfaces.account_form = AccountForm::openshell();
    world.surfaces.vocabulary = Vocabulary::openshell();
    let words = &mut world.surfaces.vocabulary.source;
    words.env_label = "CANARY label".into();
    words.env_placeholder = "CANARY placeholder".into();
    words.env_help = "CANARY help".into();
    words.env_empty = "CANARY empty".into();
    world
}

#[test]
fn the_form_words_the_source_from_the_vocabulary() {
    let mut app = app_with(Scenario::AccountsMixed, canary_world());
    press(&mut app, "c a");
    click_text(&mut app, "Environment variable");
    let shown = screen(&mut app);
    assert!(shown.contains("CANARY label"), "{shown}");
    assert!(shown.contains("CANARY placeholder"), "{shown}");
    press(&mut app, "tab");
    assert!(screen(&mut app).contains("CANARY help"));
    click_text(&mut app, "Validate");
    let refused = screen(&mut app);
    assert!(refused.contains("CANARY empty"), "{refused}");
}

#[test]
fn the_demo_and_openshell_share_the_source_words() {
    assert_eq!(Vocabulary::demo().source, Vocabulary::openshell().source);
    let names: Vec<&str> = Vocabulary::openshell()
        .fields()
        .into_iter()
        .map(|(name, _)| name)
        .filter(|name| name.starts_with("source."))
        .collect();
    assert_eq!(
        names,
        [
            "source.env_label",
            "source.env_placeholder",
            "source.env_help",
            "source.env_empty",
        ]
    );
}
