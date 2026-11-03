mod common;

use common::*;
use junie_tui::core::event::Outcome;
use junie_tui::core::refusal::{clear_refusal, raise, take_refusal};
use junie_tui::theme::Tone;
use ratatui::crossterm::event::KeyCode;

#[test]
fn the_shell_clears_an_untaken_refusal_at_the_end_of_each_input() {
    for (name, input) in every_input() {
        clear_refusal();
        let mut host = Host::new(Act::Raise(reason()));
        host.input(input);
        assert_eq!(take_refusal(), None, "{name}");
    }
}

#[test]
fn the_shell_clears_a_stale_refusal_before_each_input() {
    for (name, input) in every_input() {
        clear_refusal();
        raise(&reason());
        let mut host = Host::new(Act::Nothing);
        host.input(input);
        assert_eq!(host.seen_at_entry, Some(None), "{name}");
    }
}

#[test]
fn a_status_posted_by_a_consumed_key_is_a_change() {
    let mut host = Host::new(Act::Status("saved", Tone::Secondary));
    assert_eq!(host.press(KeyCode::Char('x')), Outcome::Changed);
    assert_eq!(host.status(), Some(("saved".to_owned(), Tone::Secondary)));
}

#[test]
fn a_key_that_posts_nothing_is_not_a_change() {
    let mut host = Host::new(Act::Nothing);
    assert_eq!(host.press(KeyCode::Char('x')), Outcome::Ignored);
}

#[test]
fn an_error_cleared_by_the_input_is_a_change() {
    let mut host = Host::new(Act::Nothing);
    host.set_status("write failed", Tone::Error);
    assert_eq!(host.press(KeyCode::Char('x')), Outcome::Changed);
    assert_eq!(host.status(), None);
}

#[test]
fn an_identical_status_posted_again_is_not_a_change() {
    let mut host = Host::new(Act::Status("saved", Tone::Secondary));
    host.set_status("saved", Tone::Secondary);
    assert_eq!(host.press(KeyCode::Char('x')), Outcome::Consumed);
}

#[test]
fn the_same_text_in_another_tone_is_a_change() {
    let mut host = Host::new(Act::Status("saved", Tone::Warning));
    host.set_status("saved", Tone::Secondary);
    assert_eq!(host.press(KeyCode::Char('x')), Outcome::Changed);
    assert_eq!(host.status(), Some(("saved".to_owned(), Tone::Warning)));
}
