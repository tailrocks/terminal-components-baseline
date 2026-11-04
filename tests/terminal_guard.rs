#![cfg(feature = "terminal")]

//! What untrusted program output may cost the host, and what it may leave in
//! a cell. Everything goes through `TerminalState::feed`.

use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc;
use std::time::Duration;

use junie_tui::terminal::{TermEvent, TerminalState, TerminalView};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::widgets::Widget;

struct Counting;

static LIVE: AtomicUsize = AtomicUsize::new(0);
static PEAK: AtomicUsize = AtomicUsize::new(0);

fn grow(by: usize) {
    let live = LIVE.fetch_add(by, Ordering::SeqCst) + by;
    PEAK.fetch_max(live, Ordering::SeqCst);
}

unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        grow(layout.size());
        unsafe { System.alloc(layout) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        LIVE.fetch_sub(layout.size(), Ordering::SeqCst);
        unsafe { System.dealloc(ptr, layout) }
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new: usize) -> *mut u8 {
        LIVE.fetch_sub(layout.size(), Ordering::SeqCst);
        grow(new);
        unsafe { System.realloc(ptr, layout, new) }
    }
}

#[global_allocator]
static ALLOC: Counting = Counting;

fn row(state: &TerminalState, row: u16) -> String {
    let view = state.view(0);
    (0..view.cols())
        .filter_map(|col| view.cell(row, col))
        .filter(|cell| !cell.is_wide_continuation())
        .map(|cell| cell.text().to_owned())
        .collect::<String>()
        .trim_end()
        .to_owned()
}

fn cell_text(state: &TerminalState, col: u16) -> String {
    state
        .view(0)
        .cell(0, col)
        .map(|c| c.text().to_owned())
        .unwrap_or_default()
}

fn osc52(payload: &str) -> Vec<u8> {
    format!("\x1b]52;c;{payload}\x07").into_bytes()
}

fn base64(bytes: &[u8]) -> String {
    const ABC: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::new();
    for chunk in bytes.chunks(3) {
        let n = chunk
            .iter()
            .enumerate()
            .fold(0u32, |n, (i, b)| n | u32::from(*b) << (16 - 8 * i));
        for i in 0..4 {
            out.push(if i <= chunk.len() {
                ABC[(n >> (18 - 6 * i) & 63) as usize] as char
            } else {
                '='
            });
        }
    }
    out
}

#[test]
fn a_mebibyte_of_combining_marks_feeds_in_well_under_a_second() {
    let mut bytes = b"a".to_vec();
    for _ in 0..(1 << 20) / 2 {
        bytes.extend_from_slice("\u{301}".as_bytes());
    }
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let mut state = TerminalState::new(24, 80);
        let started = std::time::Instant::now();
        for chunk in bytes.chunks(65_536) {
            state.feed(chunk);
        }
        let _ = tx.send((started.elapsed(), cell_text(&state, 0)));
    });
    let (took, cell) = rx
        .recv_timeout(Duration::from_secs(5))
        .expect("1 MiB of combining marks must not freeze the feed");
    assert!(took < Duration::from_secs(1), "{took:?}");
    assert_eq!(
        cell.chars().count(),
        1 + 16,
        "the cell keeps a bounded cluster"
    );
}

#[test]
fn ordinary_clusters_and_text_are_unchanged() {
    let mut state = TerminalState::new(4, 40);
    state.feed("e\u{301}x \u{1F468}\u{200D}\u{1F469}\u{200D}\u{1F467} ok".as_bytes());
    assert_eq!(
        row(&state, 0),
        "e\u{301}x \u{1F468}\u{200D}\u{1F469}\u{200D}\u{1F467} ok"
    );

    let mut split = TerminalState::new(4, 40);
    let text = "e\u{301}x \u{1F468}\u{200D}\u{1F469}\u{200D}\u{1F467} ok".as_bytes();
    for byte in text {
        split.feed(std::slice::from_ref(byte));
    }
    assert_eq!(
        row(&split, 0),
        row(&state, 0),
        "one byte per feed gives the same screen"
    );
}

#[test]
fn marks_are_counted_across_feeds_and_through_escape_sequences() {
    let mut state = TerminalState::new(4, 40);
    state.feed(b"a");
    for _ in 0..40 {
        state.feed("\u{301}".as_bytes());
        state.feed(b"\x1b[0m");
    }
    assert_eq!(cell_text(&state, 0).chars().count(), 1 + 16);

    state.feed(b"b");
    state.feed("\u{301}".as_bytes());
    assert_eq!(
        cell_text(&state, 1).chars().count(),
        2,
        "a new base starts again"
    );
}

#[test]
fn an_unterminated_string_cannot_grow_the_parser_without_bound() {
    for introducer in [
        &b"\x1b]52;c;"[..],
        b"\x1b]0;",
        b"\x1bP",
        b"\x1b_G",
        b"\x1b^",
        b"\x1bX",
    ] {
        let mut state = TerminalState::new(24, 80);
        let chunk = vec![b'A'; 1 << 16];
        state.feed(introducer);
        let base = LIVE.load(Ordering::SeqCst);
        PEAK.store(base, Ordering::SeqCst);
        for _ in 0..(32 << 20) / chunk.len() {
            state.feed(&chunk);
        }
        let grown = PEAK.load(Ordering::SeqCst).saturating_sub(base);
        assert!(
            grown < 2 << 20,
            "{:?}: 32 MiB of an open string grew the heap by {grown} bytes",
            String::from_utf8_lossy(introducer)
        );
        state.feed(b"\x1b\\ok");
        assert_eq!(
            row(&state, 0),
            "ok",
            "the string ended and text prints again"
        );
    }
}

#[test]
fn a_dropped_string_does_not_dispatch_its_prefix() {
    let mut state = TerminalState::new(24, 80);
    let mut events = state.feed(&osc52("SGVsbG8="));
    events.extend(state.feed(b"\x1b]52;c;"));
    for _ in 0..40 {
        events.extend(state.feed(&vec![b'A'; 1 << 16]));
    }
    events.extend(state.feed(b"\x07after"));
    assert_eq!(
        events,
        vec![TermEvent::ClipboardWrite("c;SGVsbG8=".to_owned())],
        "only the short sequence dispatches"
    );
    assert_eq!(row(&state, 0), "after");
}

#[test]
fn normal_strings_still_work_up_to_the_clipboard_limit() {
    let mut state = TerminalState::new(24, 80);
    let events = state.feed(b"\x1b]0;a title\x07\x1b]0;second\x1b\\x");
    assert_eq!(
        events,
        vec![
            TermEvent::Title("a title".to_owned()),
            TermEvent::Title("second".to_owned())
        ]
    );
    assert_eq!(row(&state, 0), "x");

    let payload = base64(&vec![b'z'; 100_000]);
    let events = state.feed(&osc52(&payload));
    assert_eq!(
        events,
        vec![TermEvent::ClipboardWrite(format!("c;{payload}"))],
        "a 100,000 byte clipboard write fits"
    );

    let split = state.feed(b"\x1b]0;in two");
    assert!(split.is_empty());
    assert_eq!(
        state.feed(b" parts\x07"),
        vec![TermEvent::Title("in two parts".to_owned())]
    );
}

fn draw(state: &TerminalState) -> Buffer {
    let (rows, cols) = state.size();
    let area = Rect::new(0, 0, cols, rows);
    let mut buf = Buffer::empty(area);
    TerminalView::new(state).render(area, &mut buf);
    buf
}

#[test]
fn a_delete_byte_never_reaches_a_cell() {
    let mut state = TerminalState::new(2, 10);
    state.feed(b"A\x7fB");
    assert_eq!(row(&state, 0), "AB");
    let buf = draw(&state);
    assert_eq!(buf[(0, 0)].symbol(), "A");
    assert_eq!(buf[(1, 0)].symbol(), "B");
}

#[test]
fn bidi_controls_never_reach_a_cell() {
    let mut state = TerminalState::new(2, 20);
    let mut text = String::from("A");
    for c in ('\u{202A}'..='\u{202E}').chain('\u{2066}'..='\u{2069}') {
        text.push(c);
    }
    text.push('B');
    state.feed(text.as_bytes());
    assert_eq!(row(&state, 0), "AB");
}

#[test]
fn escape_heavy_output_gives_the_screen_of_the_bare_parser_however_it_is_split() {
    let stream = "\x1b[31mred\x1b[0m \x1b(B\x1b]0;t\x07\x1b]2;u\x1b\\ \x1bP1$r\x1b\\e\u{301}\u{e9}\
                  \x1b[2;1Hnext \x1b[1;5Hmid\x1b\x1b[3;1Hlast\x1b]8;;http://x\x07link\x1b]8;;\x07 \u{fc}\
                  \x1b[4;1H\u{1F468}\u{200D}\u{1F469}\u{200D}\u{1F467}\x1b^pm\x1b\\\x1b_apc\x07\x18z"
        .as_bytes();
    let mut bare = termpane::DamageGrid::new(5, 30, 10);
    bare.process(stream);
    let bare_rows: Vec<String> = (0..5)
        .map(|r| {
            (0..30)
                .filter_map(|c| bare.cell(r, c))
                .filter(|cell| !cell.is_wide_continuation)
                .map(|cell| cell.contents().to_owned())
                .collect::<String>()
                .trim_end()
                .to_owned()
        })
        .collect();
    assert!(bare_rows[0].starts_with("red mid"), "{bare_rows:?}");

    for size in [stream.len(), 1, 2, 3, 5, 7] {
        let mut state = TerminalState::new(5, 30);
        for chunk in stream.chunks(size) {
            state.feed(chunk);
        }
        let rows: Vec<String> = (0..5).map(|r| row(&state, r)).collect();
        assert_eq!(rows, bare_rows, "chunks of {size}");
    }
}
