use jackin_console::backend::{KeyRefusal, PlainKey};

#[test]
fn a_key_is_trimmed_and_kept_whole() {
    let key = PlainKey::new("  sk-abc \t\r\n").expect("a key");
    assert_eq!(key.expose(), "sk-abc");
    assert_eq!(format!("{key:?}"), "PlainKey(…)");
}

#[test]
fn blanks_are_no_key() {
    for blank in ["", "   ", "\t\r\n", "\u{a0}\u{2003}"] {
        assert_eq!(
            PlainKey::new(blank).unwrap_err(),
            KeyRefusal::Empty,
            "{blank:?}"
        );
    }
}

#[test]
fn a_key_that_hides_text_is_refused() {
    for c in [
        '\u{1b}',
        '\u{7}',
        '\r',
        '\n',
        '\t',
        '\u{85}',
        '\u{202e}',
        '\u{2066}',
        '\u{200b}',
        '\u{feff}',
        '\u{e0041}',
    ] {
        assert_eq!(
            PlainKey::new(&format!("sk-{c}abc")).unwrap_err(),
            KeyRefusal::Unprintable,
            "U+{:04X}",
            c as u32
        );
    }
}
