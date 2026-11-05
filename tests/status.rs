mod common;

use common::*;
use junie_tui::clock::{Moment, TestRequest};
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

#[test]
fn a_clock_step_request_does_not_clear_an_error_status_like_operator_input() {
    let mut host = Host::new(Act::Nothing);
    host.test_requests(true);
    host.set_status("save failed", Tone::Error);

    host.input(Input::Paste(TestRequest::advance(250)));
    assert_eq!(
        host.status(),
        Some(("save failed".to_owned(), Tone::Error)),
        "a step of the test clock is not the operator reading the error"
    );

    host.input(Input::Paste("typed".into()));
    assert_eq!(host.status(), None, "a real paste still clears it");
}

#[test]
fn a_clock_step_request_does_not_clear_a_sticky_status() {
    let mut host = Host::new(Act::Nothing);
    host.test_requests(true);
    host.sticky("/Users/me/project");

    host.input(Input::Paste(TestRequest::advance(250)));

    assert_eq!(
        host.status(),
        Some(("/Users/me/project".to_owned(), Tone::Secondary))
    );
}

mod payload {
    use std::time::Duration;

    use junie_tui::core::event::{Input, Outcome};
    use junie_tui::shell::{App, Shell, handle};
    use junie_tui::theme::{ColorLevel, Theme, Tone};
    use junie_tui::ui::ctx::RenderCtx;
    use junie_tui::widgets::too_small::TooSmall;
    use ratatui::buffer::Buffer;
    use ratatui::layout::Rect;

    pub struct Host {
        shell: Shell<u8>,
        pub next: Option<(&'static str, Tone, u8)>,
    }

    impl Host {
        pub fn new() -> Self {
            Self {
                shell: Shell::new(),
                next: None,
            }
        }

        pub fn show(&mut self, text: &str, tone: Tone, payload: u8) {
            self.shell.status.set_with(text, tone, payload);
        }

        pub fn input(&mut self, input: Input) -> Outcome {
            handle(self, input)
        }

        pub fn payload(&self) -> Option<u8> {
            self.shell.status.payload().copied()
        }
    }

    impl App for Host {
        type Status = u8;

        fn shell(&self) -> &Shell<u8> {
            &self.shell
        }

        fn shell_mut(&mut self) -> &mut Shell<u8> {
            &mut self.shell
        }

        fn theme(&self) -> Theme {
            Theme::for_level(ColorLevel::TrueColor)
        }

        fn too_small(&self) -> TooSmall<'_> {
            TooSmall::new("host")
        }

        fn now_ms(&self) -> u64 {
            0
        }

        fn tick(&self) -> u64 {
            0
        }

        fn tick_interval(&self) -> Duration {
            Duration::from_millis(100)
        }

        fn draw(&mut self, _area: Rect, _buf: &mut Buffer, _ctx: &mut RenderCtx) {}

        fn on_input(&mut self, _input: Input) -> Outcome {
            if let Some((text, tone, payload)) = self.next.take() {
                self.shell.status.set_with(text, tone, payload);
            }
            Outcome::Ignored
        }
    }
}

#[test]
fn an_input_that_changes_only_the_payload_of_the_status_redraws() {
    let mut host = payload::Host::new();
    host.show("Copied", Tone::Secondary, 1);

    host.next = Some(("Copied", Tone::Secondary, 2));
    assert_eq!(host.input(Input::Tick), Outcome::Changed);
    assert_eq!(host.payload(), Some(2));

    assert_eq!(host.input(Input::Tick), Outcome::Ignored);
}
