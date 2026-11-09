use junie_tui::terminal::is_spoofing_format;

pub fn shown(raw: &str, max_chars: usize) -> String {
    let mut out = String::new();
    let mut chars = 0;
    let mut gap = false;
    let mut visible = false;
    for c in raw.chars() {
        if c.is_whitespace() {
            gap = chars > 0;
            continue;
        }
        if is_hidden(c) {
            continue;
        }
        let needed = if gap { 2 } else { 1 };
        if chars + needed > max_chars {
            break;
        }
        if gap {
            out.push(' ');
            gap = false;
        }
        visible |= !is_joiner(c);
        out.push(c);
        chars += needed;
    }
    if visible { out } else { String::new() }
}

pub fn escaped(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    for c in raw.chars() {
        if is_hidden(c) {
            out.extend(c.escape_debug());
        } else {
            out.push(c);
        }
    }
    out
}

pub fn is_hidden(c: char) -> bool {
    c.is_control() || is_spoofing_format(c) || is_invisible(c)
}

fn is_invisible(c: char) -> bool {
    matches!(
        c,
        '\u{00ad}'
            | '\u{034f}'
            | '\u{115f}'..='\u{1160}'
            | '\u{17b4}'..='\u{17b5}'
            | '\u{180b}'..='\u{180f}'
            | '\u{200b}'
            | '\u{2060}'..='\u{2064}'
            | '\u{206a}'..='\u{206f}'
            | '\u{2800}'
            | '\u{3164}'
            | '\u{feff}'
            | '\u{ffa0}'
            | '\u{fff9}'..='\u{fffb}'
    )
}

fn is_joiner(c: char) -> bool {
    matches!(c, '\u{200c}' | '\u{200d}' | '\u{fe00}'..='\u{fe0f}' | '\u{e0100}'..='\u{e01ef}')
}
