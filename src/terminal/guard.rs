//! A filter over the bytes a program writes, ahead of the terminal parser.
//!
//! The parser keeps every byte of an open string sequence and grows one cell
//! for every combining mark it is given, so a program that never ends a string
//! or never stops sending marks costs the host memory or time without bound.
//! The guard bounds both, in a streaming way: its state survives across
//! `feed` calls, and a sequence split over several calls is judged as a whole.
//! It also keeps two kinds of character out of every cell: DEL, which the
//! renderer cannot draw, and the bidirectional controls, which reorder the
//! text around them.

use unicode_width::UnicodeWidthChar;

/// The most bytes of one string sequence (OSC, DCS, APC, PM, SOS) that is
/// passed on. A 100,000 byte clipboard write is 133,336 bytes of base64, and
/// the rest is room for the sequence's own prefix.
const MAX_STRING_BYTES: usize = 144 * 1024;

/// The most characters that may join the cell before them: combining marks,
/// variation selectors, joiners, and the characters a joiner pulls in.
const MAX_JOINERS: u16 = 16;

const ESC: u8 = 0x1b;
const BEL: u8 = 0x07;
const CAN: u8 = 0x18;
const SUB: u8 = 0x1a;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum State {
    Ground,
    /// After an `ESC` that is not yet passed on: it goes with the byte that
    /// says what it starts.
    Esc,
    Csi,
    /// Inside a string sequence. `Held` keeps its bytes until it ends, so a
    /// sequence that grows past the cap is dropped whole and the parser never
    /// sees a prefix of it. `Dropping` discards bytes until the end.
    Held {
        osc: bool,
        esc: bool,
    },
    Dropping {
        osc: bool,
        esc: bool,
    },
}

#[derive(Debug)]
pub(super) struct FeedGuard {
    state: State,
    held: Vec<u8>,
    /// The bytes of a character that a feed ended in the middle of.
    partial: Vec<u8>,
    joiners: u16,
    after_joiner: bool,
    out: Vec<u8>,
}

impl FeedGuard {
    pub(super) fn new() -> Self {
        Self {
            state: State::Ground,
            held: Vec::new(),
            partial: Vec::new(),
            joiners: 0,
            after_joiner: false,
            out: Vec::new(),
        }
    }

    /// The bytes to give the parser for `bytes`.
    pub(super) fn filter(&mut self, bytes: &[u8]) -> &[u8] {
        self.out.clear();
        for &byte in bytes {
            self.step(byte);
        }
        &self.out
    }

    fn step(&mut self, byte: u8) {
        loop {
            match self.state {
                State::Ground => return self.ground(byte),
                State::Esc => {
                    if self.escape(byte) {
                        return;
                    }
                }
                State::Csi => {
                    self.out.push(byte);
                    self.state = match byte {
                        0x40..=0x7e | CAN | SUB => State::Ground,
                        ESC => State::Esc,
                        _ => State::Csi,
                    };
                    return;
                }
                State::Held { osc, esc } => {
                    if self.held_string(byte, osc, esc) {
                        return;
                    }
                }
                State::Dropping { osc, esc } => {
                    if self.dropping(byte, osc, esc) {
                        return;
                    }
                }
            }
        }
    }

    /// Whether `byte` is used up. `false` means the state changed and the byte
    /// is to be read again in the new one.
    fn escape(&mut self, byte: u8) -> bool {
        match byte {
            b']' | b'P' | b'_' | b'^' | b'X' => {
                self.held.clear();
                self.held.extend_from_slice(&[ESC, byte]);
                self.state = State::Held {
                    osc: byte == b']',
                    esc: false,
                };
            }
            b'[' => {
                self.out.extend_from_slice(&[ESC, byte]);
                self.state = State::Csi;
            }
            ESC => self.out.push(ESC),
            _ => {
                self.out.extend_from_slice(&[ESC, byte]);
                self.state = State::Ground;
            }
        }
        true
    }

    fn held_string(&mut self, byte: u8, osc: bool, esc: bool) -> bool {
        if esc {
            if byte == b'\\' {
                self.held.push(byte);
                self.release();
                return true;
            }
            self.held.pop();
            self.release();
            self.state = State::Esc;
            return false;
        }
        match byte {
            BEL if osc => {
                self.held.push(byte);
                self.release();
            }
            CAN | SUB => {
                self.held.push(byte);
                self.release();
            }
            ESC => {
                self.held.push(byte);
                self.state = State::Held { osc, esc: true };
            }
            _ => {
                if self.held.len() >= MAX_STRING_BYTES {
                    self.held.clear();
                    self.held.shrink_to(0);
                    self.state = State::Dropping { osc, esc: false };
                } else {
                    self.held.push(byte);
                }
            }
        }
        true
    }

    fn dropping(&mut self, byte: u8, osc: bool, esc: bool) -> bool {
        if esc {
            if byte == b'\\' {
                self.state = State::Ground;
                return true;
            }
            self.state = State::Esc;
            return false;
        }
        match byte {
            BEL if osc => self.state = State::Ground,
            CAN | SUB => self.state = State::Ground,
            ESC => self.state = State::Dropping { osc, esc: true },
            _ => {}
        }
        true
    }

    fn release(&mut self) {
        self.out.append(&mut self.held);
        self.state = State::Ground;
    }

    fn ground(&mut self, byte: u8) {
        if !self.partial.is_empty() || byte >= 0x80 {
            return self.multibyte(byte);
        }
        match byte {
            ESC => self.state = State::Esc,
            0x7f => {}
            0x20..=0x7e => {
                self.base();
                self.out.push(byte);
            }
            _ => self.out.push(byte),
        }
    }

    fn base(&mut self) {
        self.joiners = 0;
        self.after_joiner = false;
    }

    fn multibyte(&mut self, byte: u8) {
        let want = match self.partial.first().copied().unwrap_or(byte) {
            0xc2..=0xdf => 2,
            0xe0..=0xef => 3,
            0xf0..=0xf4 => 4,
            _ => 1,
        };
        let continues = (0x80..=0xbf).contains(&byte) && !self.partial.is_empty();
        if !self.partial.is_empty() && !continues {
            self.out.append(&mut self.partial);
            return self.ground(byte);
        }
        self.partial.push(byte);
        if self.partial.len() < want {
            return;
        }
        let bytes = std::mem::take(&mut self.partial);
        match std::str::from_utf8(&bytes)
            .ok()
            .and_then(|s| s.chars().next())
        {
            Some(c) => self.character(c, &bytes),
            None => self.out.extend_from_slice(&bytes),
        }
    }

    fn character(&mut self, c: char, bytes: &[u8]) {
        if matches!(c, '\u{202a}'..='\u{202e}' | '\u{2066}'..='\u{2069}') {
            return;
        }
        let joins = self.after_joiner || c.width() == Some(0);
        if joins {
            if self.joiners >= MAX_JOINERS {
                return;
            }
            self.joiners += 1;
            self.after_joiner = c == '\u{200d}';
        } else {
            self.base();
        }
        self.out.extend_from_slice(bytes);
    }
}
