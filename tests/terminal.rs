#![cfg(feature = "terminal")]

use junie_tui::terminal::{RowWrap, TermEvent, TermView, TerminalState, TerminalView};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier};
use ratatui::widgets::Widget;

fn lines(from: u32, to: u32) -> Vec<u8> {
    (from..=to)
        .flat_map(|n| format!("L{n:02}\r\n").into_bytes())
        .collect()
}

fn view_rows(view: &TermView<'_>) -> Vec<String> {
    (0..view.rows())
        .map(|row| {
            (0..view.cols())
                .filter_map(|col| view.cell(row, col))
                .filter(|cell| !cell.is_wide_continuation())
                .map(|cell| match cell.text() {
                    "" => " ".to_owned(),
                    text => text.to_owned(),
                })
                .collect()
        })
        .collect()
}

fn draw(state: &TerminalState, focused: bool) -> Buffer {
    let (rows, cols) = state.size();
    let area = Rect::new(0, 0, cols, rows);
    let mut buf = Buffer::empty(area);
    TerminalView::new(state)
        .focused(focused)
        .render(area, &mut buf);
    buf
}

fn buf_rows(buf: &Buffer) -> Vec<String> {
    (0..buf.area.height)
        .map(|y| {
            (0..buf.area.width)
                .map(|x| buf[(x, y)].symbol().to_owned())
                .collect()
        })
        .collect()
}

fn numbered(first: u32, last: u32) -> Vec<String> {
    (first..=last).map(|n| format!("L{n:02}")).collect()
}

fn history_state() -> TerminalState {
    let mut state = TerminalState::new(6, 3);
    state.feed(&lines(1, 29));
    state.feed(b"L30");
    state
}

#[test]
fn scrolled_view_reads_scrollback() {
    let mut state = history_state();
    assert_eq!(state.scrollback_len(), 24);
    assert_eq!(buf_rows(&draw(&state, false)), numbered(25, 30));
    for (offset, first) in [(3, 22), (6, 19), (10, 15), (99, 1)] {
        state.set_scrollback(offset);
        assert_eq!(
            buf_rows(&draw(&state, false)),
            numbered(first, first + 5),
            "offset {offset}"
        );
        assert_eq!(
            view_rows(&state.view(offset)),
            numbered(first, first + 5),
            "view at offset {offset}"
        );
    }
    assert_eq!(state.scrollback(), 24);
}

#[test]
fn view_forces_offset_zero_on_the_alternate_screen() {
    let mut state = history_state();
    let events = state.feed(b"\x1b[?1049h\x1b[H\x1b[2JA1\r\nA2\r\nA3");
    assert_eq!(events, vec![]);
    let live = vec!["A1 ", "A2 ", "A3 ", "   ", "   ", "   "];
    for offset in [0, 3, 6, 99] {
        assert_eq!(view_rows(&state.view(offset)), live, "view at {offset}");
    }
    state.set_scrollback(6);
    assert_eq!(buf_rows(&draw(&state, false)), live);

    state.feed(b"\x1b[?1049l");
    assert_eq!(view_rows(&state.view(3)), numbered(22, 27));
}

#[test]
fn with_scrollback_limit_is_honoured() {
    let mut state = TerminalState::with_scrollback(6, 3, 20);
    for n in 1..=10 {
        state.feed(&lines(n, n));
    }
    assert_eq!(state.scrollback_len(), 5);
    assert_eq!(state.scrolled_off(), 5);

    for n in 11..=60 {
        state.feed(&lines(n, n));
    }
    assert_eq!(state.scrollback_len(), 20);
    assert_eq!(state.scrolled_off(), 55);

    let mut once = TerminalState::with_scrollback(6, 3, 20);
    once.feed(&lines(1, 60));
    assert_eq!(once.scrollback_len(), 20);
    assert_eq!(once.scrolled_off(), 55);

    state.reset();
    assert_eq!(state.size(), (6, 3));
    assert_eq!((state.scrollback_len(), state.scrolled_off()), (0, 0));
    state.feed(&lines(1, 60));
    assert_eq!(state.scrollback_len(), 20);

    let mut none = TerminalState::with_scrollback(6, 3, 0);
    none.feed(&lines(1, 60));
    assert_eq!((none.scrollback_len(), none.scrolled_off()), (0, 0));
}

#[test]
fn scrolled_off_is_exact_after_many_feeds() {
    let mut state = TerminalState::with_scrollback(6, 3, 20);
    for _ in 0..100 {
        state.feed(b"abc\r\n");
    }
    assert_eq!(state.scrollback_len(), 20);
    assert_eq!(state.scrolled_off(), 95);
}

#[test]
fn scrolled_off_counts_only_rows_that_reach_history() {
    let mut state = TerminalState::with_scrollback(6, 3, 50);
    state.feed(b"\x1b[M\x1b[3M");
    assert_eq!((state.scrollback_len(), state.scrolled_off()), (0, 0));

    state.feed(b"\x1b[?1049h\r\n\r\n\r\n\r\n\r\n\r\n\r\n\r\n\x1b[?1049l");
    assert_eq!((state.scrollback_len(), state.scrolled_off()), (0, 0));

    state.feed(b"\x1b[H\x1b[2Jone\r\ntwo\r\nthree\r\nfour\r\nfive\r\nsix\r\nseven\r\neight");
    let pushed = state.scrollback_len();
    assert!(pushed > 0);
    assert_eq!(state.scrolled_off(), pushed as u64);

    state.feed(b"\x1b[2J\x1b[H");
    assert_eq!(state.scrollback_len(), pushed);
    assert_eq!(state.scrolled_off(), pushed as u64);
}

#[test]
fn wide_cell_is_drawn_once() {
    let mut state = TerminalState::new(2, 6);
    state.feed("\x1b[41m日\x1b[0mx".as_bytes());
    let buf = draw(&state, false);
    assert_eq!(buf[(0, 0)].symbol(), "日");
    assert_eq!(buf[(0, 0)].bg, Color::Indexed(1));
    assert_eq!(buf[(1, 0)].symbol(), " ");
    assert_eq!(buf[(1, 0)].bg, Color::Reset);
    assert_eq!(buf[(2, 0)].symbol(), "x");
    assert_eq!(buf[(2, 0)].bg, Color::Reset);
}

#[test]
fn phantom_cursor_column_stays_visible() {
    let mut state = TerminalState::new(3, 5);
    state.feed(b"abcde");
    assert_eq!(state.view(0).cursor(), (0, 4));
    let buf = draw(&state, true);
    let reversed: Vec<(u16, u16)> = (0..3)
        .flat_map(|y| (0..5).map(move |x| (x, y)))
        .filter(|&at| buf[at].modifier.contains(Modifier::REVERSED))
        .collect();
    assert_eq!(reversed, vec![(4, 0)]);

    let unfocused = draw(&state, false);
    assert!(!unfocused[(4, 0)].modifier.contains(Modifier::REVERSED));

    state.feed(b"\x1b[?25l");
    let hidden = draw(&state, true);
    assert!(
        (0..5).all(|x| !hidden[(x, 0)].modifier.contains(Modifier::REVERSED)),
        "a hidden cursor draws nothing"
    );
}

#[test]
fn terminal_state_is_send() {
    fn assert_send<T: Send>() {}
    assert_send::<TerminalState>();
}

#[test]
fn replies_for_queries() {
    let mut state = TerminalState::new(6, 20);
    let reply = |events: Vec<TermEvent>| match events.as_slice() {
        [TermEvent::Reply(bytes)] => bytes.clone(),
        other => panic!("expected one reply, got {other:?}"),
    };

    let device = reply(state.feed(b"\x1b[c"));
    assert!(
        device.starts_with(b"\x1b[?") && device.ends_with(b"c"),
        "{device:?}"
    );

    assert_eq!(reply(state.feed(b"\x1b[3;5H\x1b[6n")), b"\x1b[3;5R");

    let colour = reply(state.feed(b"\x1b]11;?\x07"));
    assert!(colour.starts_with(b"\x1b]11;"), "{colour:?}");

    assert_eq!(reply(state.feed(b"\x1b[?2004$p")), b"\x1b[?2004;2$y");

    assert_eq!(state.feed(b"plain text"), vec![]);
}

#[test]
fn passthrough_events_map() {
    let mut state = TerminalState::new(6, 20);
    let cases: [(&[u8], TermEvent); 8] = [
        (b"\x1b]0;first\x07", TermEvent::Title("first".into())),
        (b"\x1b]2;second\x07", TermEvent::Title("second".into())),
        (
            b"\x1b]52;c;SGVsbG8=\x07",
            TermEvent::ClipboardWrite("c;SGVsbG8=".into()),
        ),
        (b"\x1b]52;c;?\x07", TermEvent::ClipboardWrite("c;?".into())),
        (b"\x07", TermEvent::Bell),
        (
            b"\x1b]9;build done\x07",
            TermEvent::Notification("build done".into()),
        ),
        (
            b"\x1b]7;file:///tmp\x07",
            TermEvent::Cwd("file:///tmp".into()),
        ),
        (b"\x1b[3J", TermEvent::ScrollbackCleared),
    ];
    for (bytes, event) in cases {
        assert_eq!(state.feed(bytes), vec![event], "{bytes:?}");
        assert_eq!(
            state.feed(b"x"),
            vec![],
            "a second feed returns only new events"
        );
    }

    assert_eq!(
        state.feed(b"\x1b[?2004h\x1b[?1h\x1b[?1004h\x1b]8;;https://example.com\x07"),
        vec![]
    );
    assert_eq!(
        state.feed(b"\x07\x1b]0;a\x07\x07"),
        vec![
            TermEvent::Bell,
            TermEvent::Title("a".into()),
            TermEvent::Bell
        ]
    );
}

#[test]
fn split_utf8_across_feeds() {
    let mut state = TerminalState::new(2, 4);
    state.feed(&[0xE2, 0x94]);
    assert_eq!(buf_rows(&draw(&state, false))[0], "    ");
    state.feed(&[0x82, b'x']);
    assert_eq!(buf_rows(&draw(&state, false))[0], "│x  ");
}

#[test]
fn resize_contract() {
    let mut state = TerminalState::new(4, 10);
    state.feed(b"\x1b[?1049hhi");
    state.resize(2, 5);
    assert_eq!(state.size(), (2, 5));
    assert!(state.modes().alt_screen);
    let area = Rect::new(3, 1, 5, 2);
    let at = state.cursor(area).expect("cursor inside the area");
    assert!(area.contains(at), "{at:?}");
    state.feed(b"\x1b[1;1HZZ");
    assert_eq!(buf_rows(&draw(&state, false))[0], "ZZ   ");

    state.resize(6, 20);
    assert_eq!(state.size(), (6, 20));
    assert!(state.modes().alt_screen);
    state.feed(b"\x1b[6;20Hq");
    let buf = draw(&state, false);
    assert_eq!(buf[(19, 5)].symbol(), "q");

    state.resize(0, 0);
    assert_eq!(state.size(), (1, 1));
    state.feed(b"still alive\r\n");
    assert_eq!(TerminalState::new(0, 0).size(), (1, 1));
    TerminalState::new(0, 0).feed(b"no panic\r\n");
}

#[test]
fn view_reports_soft_wraps() {
    let mut state = TerminalState::new(5, 10);
    state.feed(b"ABCDEFGHIJKLMNOPQRSTUVWXY\r\nab\r\ncd");
    let view = state.view(0);
    let wraps: Vec<_> = (0..5).map(|row| view.row_wrap(row)).collect();
    assert_eq!(
        wraps,
        [
            Some(RowWrap::Hard),
            Some(RowWrap::Soft),
            Some(RowWrap::Soft),
            Some(RowWrap::Hard),
            Some(RowWrap::Hard),
        ]
    );
    assert_eq!(view.row_wrap(5), None);
    assert_eq!(view_rows(&view)[2], "UVWXY     ");
}
