mod common;

use common::*;
use junie_tui::clock::Moment;
use junie_tui::core::event::{External, Input, Mouse, MouseKind, Outcome};
use junie_tui::core::focus::{Focus, FocusRing};
use junie_tui::shell::{Cx, Request, StatusLine, background};
use junie_tui::theme::Tone;
use ratatui::crossterm::event::KeyCode;
use ratatui::layout::Position;

fn mouse(kind: MouseKind) -> Input {
    Input::Mouse(Mouse::new(kind, Position::new(1, 1)))
}

#[test]
fn a_sticky_status_is_neutral_and_does_not_expire() {
    let mut line = StatusLine::<()>::new();
    line.sticky("/Users/me/project");
    assert!(line.is_sticky());
    assert_eq!(line.get(), Some(("/Users/me/project", Tone::Secondary)));
    assert!(!line.expire(Moment::from_millis(60_000)));
    assert_eq!(line.text(), Some("/Users/me/project"));

    line.set("saved", Tone::Secondary);
    assert!(!line.is_sticky());
    assert!(line.expire(Moment::from_millis(65_000)));
    assert_eq!(line.text(), None);
}

#[test]
fn a_sticky_status_clears_on_the_next_key_paste_or_press() {
    let clearing = [
        ("key", Input::Key(key(KeyCode::Char('x')))),
        ("paste", Input::Paste("text".into())),
        ("press", mouse(MouseKind::Down)),
        ("secondary press", mouse(MouseKind::Secondary)),
        ("middle press", mouse(MouseKind::Middle)),
    ];
    for (name, input) in clearing {
        let mut line = StatusLine::<()>::new();
        line.sticky("path");
        assert!(line.on_input(&input), "{name}");
        assert_eq!(line.text(), None, "{name}");
        assert!(!line.is_sticky(), "{name}");
    }
}

#[test]
fn a_sticky_status_stays_through_ticks_resizes_and_pointer_moves() {
    let staying = [
        ("tick", Input::Tick),
        ("resize", Input::Resize(100, 30)),
        ("external", Input::External(External::new(1_u8))),
        ("move", mouse(MouseKind::Move)),
        ("release", mouse(MouseKind::Up)),
    ];
    for (name, input) in staying {
        let mut line = StatusLine::<()>::new();
        line.sticky("path");
        assert!(!line.on_input(&input), "{name}");
        assert_eq!(line.text(), Some("path"), "{name}");
    }
}

#[test]
fn a_sticky_status_posted_by_a_key_survives_that_key() {
    let mut host = Host::new(Act::Sticky("path"));
    assert_eq!(host.press(KeyCode::Char('x')), Outcome::Changed);
    assert_eq!(host.status(), Some(("path".to_owned(), Tone::Secondary)));
    host.act = Act::Nothing;
    assert_eq!(host.press(KeyCode::Char('y')), Outcome::Changed);
    assert_eq!(host.status(), None);
}

#[test]
fn a_page_in_the_background_may_keep_a_sticky_status() {
    let mut focus = Focus::default();
    let ring = FocusRing::default();
    let mut cx: Cx<'_, (), (), ()> = Cx::new(&mut focus, &ring);
    cx.sticky("path");
    let requests: Vec<_> = background(cx.take()).collect();
    assert!(matches!(requests.as_slice(), [Request::Sticky(text)] if text == "path"));
}
