mod common;

use common::*;
use junie_tui::clock::TestRequest;
use junie_tui::core::event::{Input, Outcome};
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

#[test]
fn the_same_tone_with_other_text_is_a_change() {
    let mut host = Host::new(Act::Status("second", Tone::Secondary));
    host.set_status("first", Tone::Secondary);
    assert_eq!(host.press(KeyCode::Char('x')), Outcome::Changed);
    assert_eq!(host.status(), Some(("second".to_owned(), Tone::Secondary)));
}

#[test]
fn a_fence_is_acknowledged_once_and_never_reaches_the_app() {
    junie_tui::shell::clipboard::take_output();
    let mut host = Host::new(Act::Nothing);
    host.test_requests(true);

    let out = host.input(Input::Paste(TestRequest::FENCE.to_owned()));

    assert_eq!(out, Outcome::Ignored);
    assert_eq!(host.inputs, 0, "the app never sees a fence");
    assert_eq!(
        junie_tui::shell::clipboard::take_output(),
        [TestRequest::FENCE_ACK],
        "one acknowledgement per fence"
    );
}

#[test]
fn a_clock_step_reaches_the_app_as_a_step_and_not_as_a_paste() {
    junie_tui::shell::clipboard::take_output();
    let mut host = Host::new(Act::Nothing);
    host.test_requests(true);

    host.input(Input::Paste(TestRequest::advance(250)));
    host.input(Input::Paste(TestRequest::advance(40)));

    assert_eq!(host.stepped, [250, 40]);
    assert_eq!(host.inputs, 0);
}

#[test]
fn test_requests_are_plain_pastes_unless_the_shell_accepts_them() {
    junie_tui::shell::clipboard::take_output();
    let mut host = Host::new(Act::Nothing);
    host.test_requests(false);

    host.input(Input::Paste(TestRequest::FENCE.to_owned()));
    host.input(Input::Paste(TestRequest::advance(250)));

    assert_eq!(host.inputs, 2, "both reach the app as text");
    assert!(host.stepped.is_empty());
    assert_eq!(
        junie_tui::shell::clipboard::take_output(),
        Vec::<String>::new()
    );
}

#[test]
fn a_stop_handle_ends_the_loop_from_another_thread() {
    use junie_tui::runtime::Application;

    let host = Host::new(Act::Nothing);
    assert!(!host.should_quit());

    let stop = host.stop_handle();
    std::thread::spawn(move || stop.stop())
        .join()
        .expect("the stopping thread");

    assert!(
        host.should_quit(),
        "the loop reads should_quit after a stop"
    );
}
