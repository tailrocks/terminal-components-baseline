use ratatui::buffer::Buffer;
use ratatui::crossterm::event::{KeyCode, KeyModifiers};
use ratatui::layout::{Position, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::widgets::Widget;
use termpane::{DamageGrid, PassthroughEvent, ScrollOp};

use crate::core::event::Key;

pub const SCROLLBACK_ROWS: usize = 1_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Modes {
    pub app_cursor: bool,
    pub bracketed_paste: bool,
    pub alt_screen: bool,
    pub hide_cursor: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TermEvent {
    Reply(Vec<u8>),
    Title(String),
    ClipboardWrite(String),
    Bell,
    Notification(String),
    Cwd(String),
    ScrollbackCleared,
}

impl TermEvent {
    fn from_passthrough(event: PassthroughEvent) -> Option<Self> {
        match event {
            PassthroughEvent::Reply(bytes) => Some(Self::Reply(bytes)),
            PassthroughEvent::TitleChanged(title) => Some(Self::Title(title)),
            PassthroughEvent::ClipboardWrite(payload) => Some(Self::ClipboardWrite(payload)),
            PassthroughEvent::Bell => Some(Self::Bell),
            PassthroughEvent::Notification(text) => Some(Self::Notification(text)),
            PassthroughEvent::CwdChanged(uri) => Some(Self::Cwd(uri)),
            PassthroughEvent::ScrollbackClear => Some(Self::ScrollbackCleared),
            PassthroughEvent::IconNameChanged(_)
            | PassthroughEvent::ApplicationCursorKeys(_)
            | PassthroughEvent::FocusEvents(_)
            | PassthroughEvent::BracketedPaste(_)
            | PassthroughEvent::Hyperlink { .. }
            | PassthroughEvent::UnhandledCsi(_)
            | PassthroughEvent::DroppedCsi(_) => None,
        }
    }
}

pub struct TerminalState {
    grid: DamageGrid,
    limit: usize,
    scrolled_off: u64,
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

impl TerminalState {
    pub fn new(rows: u16, cols: u16) -> Self {
        Self::with_scrollback(rows, cols, SCROLLBACK_ROWS)
    }

    pub fn with_scrollback(rows: u16, cols: u16, limit: usize) -> Self {
        Self {
            grid: DamageGrid::new(rows.max(1), cols.max(1), limit),
            limit,
            scrolled_off: 0,
        }
    }

    pub fn feed(&mut self, bytes: &[u8]) -> Vec<TermEvent> {
        let before = self.grid.scrollback_len();
        self.grid.process(bytes);
        self.scrolled_off += self.pushed_since(before);
        self.grid
            .drain_passthrough()
            .into_iter()
            .filter_map(TermEvent::from_passthrough)
            .collect()
    }

    fn pushed_since(&mut self, before: usize) -> u64 {
        let ops = self.grid.drain_scroll_ops();
        let len = self.grid.scrollback_len();
        if self.limit == 0 {
            0
        } else if len < self.limit {
            (len - before.min(len)) as u64
        } else if self.grid.alternate_screen() {
            0
        } else {
            ops.iter()
                .map(|op| match op {
                    ScrollOp::Up { top: 0, rows, .. } => u64::from(*rows),
                    _ => 0,
                })
                .sum()
        }
    }

    pub fn resize(&mut self, rows: u16, cols: u16) {
        self.grid.set_size(rows, cols);
    }

    pub fn reset(&mut self) {
        let (rows, cols) = self.size();
        *self = Self::with_scrollback(rows, cols, self.limit);
    }

    pub fn size(&self) -> (u16, u16) {
        self.grid.size()
    }

    pub fn modes(&self) -> Modes {
        Modes {
            app_cursor: self.grid.application_cursor(),
            bracketed_paste: self.grid.bracketed_paste(),
            alt_screen: self.grid.alternate_screen(),
            hide_cursor: self.grid.hide_cursor(),
        }
    }

    pub fn scrollback_len(&self) -> usize {
        self.grid.scrollback_len()
    }

    pub fn scrolled_off(&self) -> u64 {
        self.scrolled_off
    }

    pub fn scrollback(&self) -> usize {
        self.grid.scrollback()
    }

    pub fn set_scrollback(&mut self, rows: usize) {
        self.grid.set_scrollback(rows);
    }

    pub fn view(&self, offset: usize) -> TermView<'_> {
        let offset = if self.grid.alternate_screen() {
            0
        } else {
            offset
        };
        let (rows, _) = self.grid.size();
        TermView {
            grid: self.grid.scrollback_view(offset, rows),
        }
    }

    pub fn cursor(&self, area: Rect) -> Option<Position> {
        if self.grid.hide_cursor() || self.grid.scrollback() > 0 {
            return None;
        }
        let (row, col) = self.view(0).cursor();
        (row < area.height && col < area.width).then(|| Position::new(area.x + col, area.y + row))
    }
}

#[derive(Debug)]
pub struct TermView<'a> {
    grid: termpane::GridView<'a>,
}

impl<'a> TermView<'a> {
    pub fn rows(&self) -> u16 {
        self.grid.rows
    }

    pub fn cols(&self) -> u16 {
        self.grid.cols
    }

    pub fn cell(&self, row: u16, col: u16) -> Option<TermCell<'a>> {
        self.grid.cell(row, col).map(|cell| TermCell { cell })
    }

    pub fn row_wrap(&self, row: u16) -> Option<RowWrap> {
        self.grid.row_wrap(row).map(|wrap| match wrap {
            termpane::RowWrap::Hard => RowWrap::Hard,
            termpane::RowWrap::Soft => RowWrap::Soft,
        })
    }

    pub fn cursor(&self) -> (u16, u16) {
        let (row, col) = self.grid.cursor;
        (row, col.min(self.grid.cols.saturating_sub(1)))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RowWrap {
    Hard,
    Soft,
}

#[derive(Debug, Clone, Copy)]
pub struct TermCell<'a> {
    cell: &'a termpane::Cell,
}

impl TermCell<'_> {
    pub fn text(&self) -> &str {
        self.cell.contents()
    }

    pub fn is_wide_continuation(&self) -> bool {
        self.cell.is_wide_continuation
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

fn color(value: termpane::Color) -> Color {
    match value {
        termpane::Color::Default => Color::Reset,
        termpane::Color::Idx(index) => Color::Indexed(index),
        termpane::Color::Rgb(r, g, b) => Color::Rgb(r, g, b),
    }
}

fn style(cell: &termpane::Cell, with_color: bool) -> Style {
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
        let view = self.state.view(self.state.scrollback());
        for row in 0..area.height.min(view.rows()) {
            for col in 0..area.width.min(view.cols()) {
                let Some(cell) = view.cell(row, col) else {
                    continue;
                };
                if cell.is_wide_continuation() {
                    continue;
                }
                let target = &mut buf[(area.x + col, area.y + row)];
                let text = cell.text();
                target.set_symbol(if text.is_empty() { " " } else { text });
                target.set_style(style(cell.cell, self.color));
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
