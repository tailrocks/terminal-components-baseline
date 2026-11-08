//! What a program or a backend may put in the title and the status line of a
//! pane: text a reader can see, in a bounded amount.

mod pane_support;

use jackin_console::panes::{LinkEvent, PaneBody, PaneEffect, sanitize_title};
use pane_support::*;

const INVISIBLE: [char; 25] = [
    '\u{00ad}', '\u{034f}', '\u{115f}', '\u{1160}', '\u{17b4}', '\u{17b5}', '\u{180b}', '\u{180e}',
    '\u{200b}', '\u{2060}', '\u{2061}', '\u{2062}', '\u{2063}', '\u{2064}', '\u{206a}', '\u{206f}',
    '\u{2800}', '\u{3164}', '\u{feff}', '\u{ffa0}', '\u{fff9}', '\u{fffb}', '\u{180f}', '\u{206b}',
    '\u{206e}',
];

fn notices(text: String) -> Vec<String> {
    let bench = Bench::new();
    let mut pane = pane(&bench);
    pane.sync_size(inner(80, 24));
    bench.push(0, LinkEvent::Attached);
    bench.push(0, LinkEvent::Notice(text));
    pane.tick(0);
    pane.take_effects(&words())
        .into_iter()
        .filter_map(|effect| match effect {
            PaneEffect::Status { text, .. } => Some(text),
            _ => None,
        })
        .collect()
}

#[test]
fn a_title_and_a_notice_lose_every_invisible_character() {
    for c in INVISIBLE {
        assert_eq!(
            sanitize_title(&format!("a{c}b")),
            "ab",
            "U+{:04X} in a title",
            c as u32
        );
        assert_eq!(
            notices(format!("a{c}b")),
            ["ab"],
            "U+{:04X} in a notice",
            c as u32
        );
    }
}

#[test]
fn a_line_separator_in_a_title_or_a_notice_is_a_space() {
    for c in ['\u{2028}', '\u{2029}', '\u{00a0}'] {
        assert_eq!(sanitize_title(&format!("a{c}b")), "a b");
        assert_eq!(notices(format!("a{c}b")), ["a b"]);
    }
}

#[test]
fn text_of_only_invisible_characters_shows_nothing() {
    let blank: String = INVISIBLE.iter().collect::<String>() + " \u{200d}\u{fe0f}\u{200c}";
    assert_eq!(sanitize_title(&blank), "");
    assert_eq!(notices(blank.clone()), Vec::<String>::new());

    let bench = Bench::new();
    let mut pane = pane(&bench);
    pane.sync_size(inner(80, 24));
    bench.push(0, LinkEvent::Attached);
    bench.output(0, format!("\x1b]0;{blank}\x07").as_bytes());
    pane.tick(0);
    assert_eq!(pane.title(), None);
}

#[test]
fn joiners_between_visible_characters_stay() {
    let family = "\u{1f468}\u{200d}\u{1f469}\u{200d}\u{1f467}";
    assert_eq!(sanitize_title(family), family);
    assert_eq!(notices(family.to_owned()), [family]);
}

#[test]
fn a_title_is_cut_by_characters_not_by_grapheme() {
    let piled = format!("a{}", "\u{301}".repeat(5_000));
    assert_eq!(sanitize_title(&piled).chars().count(), 64);

    let bench = Bench::new();
    let mut pane = pane(&bench);
    pane.sync_size(inner(80, 24));
    bench.push(0, LinkEvent::Attached);
    bench.output(0, format!("\x1b]0;{piled}\x07").as_bytes());
    pane.tick(0);
    assert_eq!(pane.title().map(|t| t.chars().count()), Some(64));
}

#[test]
fn a_notice_is_clipped() {
    let shown = notices("x".repeat(200_000));
    assert_eq!(shown.len(), 1);
    assert_eq!(shown[0].chars().count(), 240);
}
