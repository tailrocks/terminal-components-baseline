use std::num::NonZeroU16;

use ratatui::buffer::Buffer;
use ratatui::crossterm::event::{KeyCode, KeyModifiers};
use ratatui::layout::{Position, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::widgets::Widget;

use crate::core::event::Key;

pub const SCROLLBACK_ROWS: usize = 1_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Modes {
    pub app_cursor: bool,
    pub bracketed_paste: bool,
    pub alt_screen: bool,
    pub hide_cursor: bool,
}

pub struct TerminalState {
    parser: vt100::Parser,
}

impl std::fmt::Debug for TerminalState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let (rows, cols) = self.size();
        f.debug_struct("TerminalState")
            .field("rows", &rows)
            .field("cols", &cols)
            .finish_non_exhaustive()
    }
}

fn dimension(value: u16) -> NonZeroU16 {
    NonZeroU16::new(value).unwrap_or(NonZeroU16::MIN)
}

impl TerminalState {
    pub fn new(rows: u16, cols: u16) -> Self {
        Self {
            parser: vt100::Parser::new(dimension(rows), dimension(cols), SCROLLBACK_ROWS),
        }
    }

    pub fn feed(&mut self, bytes: &[u8]) {
        self.parser.process(bytes);
    }

    pub fn resize(&mut self, rows: u16, cols: u16) {
        self.parser.set_size(dimension(rows), dimension(cols));
    }

    pub fn reset(&mut self) {
        let (rows, cols) = self.size();
        *self = Self::new(rows, cols);
    }

    pub fn size(&self) -> (u16, u16) {
        let (rows, cols) = self.parser.screen().size();
        (rows.get(), cols.get())
    }

    pub fn modes(&self) -> Modes {
        let screen = self.parser.screen();
        Modes {
            app_cursor: screen.application_cursor(),
            bracketed_paste: screen.bracketed_paste(),
            alt_screen: screen.alternate_screen(),
            hide_cursor: screen.hide_cursor(),
        }
    }

    pub fn cursor(&self, area: Rect) -> Option<Position> {
        let screen = self.parser.screen();
        if screen.hide_cursor() {
            return None;
        }
        let (row, col) = screen.cursor_position();
        (row < area.height && col < area.width).then(|| Position::new(area.x + col, area.y + row))
    }
}

#[derive(Debug, Clone, Copy)]
pub struct TerminalView<'a> {
    state: &'a TerminalState,
    focused: bool,
    color: bool,
}

impl<'a> TerminalView<'a> {
    pub fn new(state: &'a TerminalState) -> Self {
        Self {
            state,
            focused: false,
            color: true,
        }
    }

    pub fn focused(mut self, focused: bool) -> Self {
        self.focused = focused;
        self
    }

    pub fn color(mut self, color: bool) -> Self {
        self.color = color;
        self
    }
}

fn color(value: vt100::Color) -> Color {
    match value {
        vt100::Color::Default => Color::Reset,
        vt100::Color::Idx(index) => Color::Indexed(index),
        vt100::Color::Rgb(r, g, b) => Color::Rgb(r, g, b),
    }
}

fn style(cell: &vt100::Cell, with_color: bool) -> Style {
    let mut style = Style::default();
    if with_color {
        style = style.fg(color(cell.fgcolor())).bg(color(cell.bgcolor()));
    }
    for (on, modifier) in [
        (cell.bold(), Modifier::BOLD),
        (cell.dim(), Modifier::DIM),
        (cell.italic(), Modifier::ITALIC),
        (cell.underline(), Modifier::UNDERLINED),
        (cell.inverse(), Modifier::REVERSED),
    ] {
        if on {
            style = style.add_modifier(modifier);
        }
    }
    style
}

impl Widget for TerminalView<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let screen = self.state.parser.screen();
        let (rows, cols) = screen.size();
        for row in 0..area.height.min(rows.get()) {
            for col in 0..area.width.min(cols.get()) {
                let Some(cell) = screen.cell(row, col) else {
                    continue;
                };
                if cell.is_wide_continuation() {
                    continue;
                }
                let target = &mut buf[(area.x + col, area.y + row)];
                let text = cell.contents();
                target.set_symbol(if text.is_empty() { " " } else { text });
                target.set_style(style(cell, self.color));
            }
        }
        if self.focused
            && let Some(at) = self.state.cursor(area)
        {
            let target = &mut buf[(at.x, at.y)];
            target.set_style(target.style().add_modifier(Modifier::REVERSED));
        }
    }
}

pub fn encode_key(key: &Key, modes: &Modes) -> Option<Vec<u8>> {
    let ctrl = key.mods.contains(KeyModifiers::CONTROL);
    let alt = key.mods.contains(KeyModifiers::ALT);
    let cursor = |letter: u8| {
        if modes.app_cursor {
            vec![0x1b, b'O', letter]
        } else {
            vec![0x1b, b'[', letter]
        }
    };
    let mut bytes = match key.code {
        KeyCode::Char(c) if ctrl && c.is_ascii_alphabetic() => {
            vec![c.to_ascii_lowercase() as u8 - b'a' + 1]
        }
        KeyCode::Char(' ') if ctrl => vec![0],
        KeyCode::Char(c) => c.to_string().into_bytes(),
        KeyCode::Enter => vec![b'\r'],
        KeyCode::Backspace => vec![0x7f],
        KeyCode::Tab => vec![b'\t'],
        KeyCode::BackTab => b"\x1b[Z".to_vec(),
        KeyCode::Esc => vec![0x1b],
        KeyCode::Up => cursor(b'A'),
        KeyCode::Down => cursor(b'B'),
        KeyCode::Right => cursor(b'C'),
        KeyCode::Left => cursor(b'D'),
        KeyCode::Home => cursor(b'H'),
        KeyCode::End => cursor(b'F'),
        KeyCode::PageUp => b"\x1b[5~".to_vec(),
        KeyCode::PageDown => b"\x1b[6~".to_vec(),
        KeyCode::Delete => b"\x1b[3~".to_vec(),
        KeyCode::Insert => b"\x1b[2~".to_vec(),
        KeyCode::F(n @ 1..=4) => vec![0x1b, b'O', b'P' + (n - 1)],
        KeyCode::F(n @ 5..=12) => {
            const CODES: [u8; 8] = [15, 17, 18, 19, 20, 21, 23, 24];
            format!("\x1b[{}~", CODES[usize::from(n - 5)]).into_bytes()
        }
        _ => return None,
    };
    if alt {
        bytes.insert(0, 0x1b);
    }
    Some(bytes)
}
