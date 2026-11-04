//! A scripted link for a terminal pane: it plays a byte stream and the link
//! events around it on the console's clock, so the terminal pane has a program
//! to show without any gateway.

use std::collections::VecDeque;
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

use crate::panes::{
    EndKind, Lease, LinkEvent, LinkSource, PaneCommand, PaneLink, PaneRequest, PaneSize, Sent,
};

/// Link events, each due this many milliseconds after the first time the pane
/// drained the link.
#[derive(Debug, Clone)]
pub struct Timed {
    pub after_ms: i64,
    pub event: LinkEvent,
}

#[derive(Debug)]
pub struct SimLink {
    plan: Mutex<Plan>,
}

#[derive(Debug)]
struct Plan {
    started: Option<i64>,
    steps: VecDeque<Timed>,
}

impl SimLink {
    pub fn new(steps: Vec<Timed>) -> Self {
        Self {
            plan: Mutex::new(Plan {
                started: None,
                steps: steps.into(),
            }),
        }
    }
}

impl PaneLink for SimLink {
    fn send(&self, _: PaneCommand) -> Sent {
        Sent::Queued
    }

    fn set_size(&self, _: PaneSize) {}

    fn drain(&self, now_ms: i64, output_budget: usize) -> Vec<LinkEvent> {
        let mut plan = self.plan.lock().expect("plan");
        let started = *plan.started.get_or_insert(now_ms);
        let mut out = Vec::new();
        let mut bytes = 0;
        while let Some(step) = plan.steps.front() {
            if started + step.after_ms > now_ms {
                break;
            }
            if let LinkEvent::Output(chunk) = &step.event {
                if bytes > 0 && bytes + chunk.len() > output_budget {
                    break;
                }
                bytes += chunk.len();
            }
            if let Some(step) = plan.steps.pop_front() {
                out.push(step.event);
            }
        }
        out
    }
}

/// What the program on the other end of a pane does.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Play {
    /// Attaches, shows the stream and stays.
    Stays(Stream),
    /// Attaches as a second client: the stream, read only.
    Watches(Stream),
    /// Never finishes attaching.
    Hangs,
    /// Attaches, shows the stream, and is detached a second later; the next
    /// attach stays.
    Detaches(Stream),
    /// Attaches, shows the stream, and exits with code 0 a second later; a
    /// restart attaches and stays.
    Exits(Stream),
    /// The attach cannot start.
    Refuses(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stream {
    Codex,
    Shell,
    Pager,
}

impl Stream {
    pub fn bytes(self) -> Vec<u8> {
        match self {
            Stream::Codex => codex_screen(),
            Stream::Shell => shell_screen(),
            Stream::Pager => pager_screen(),
        }
    }
}

/// Opens the scripted link of one pane. It counts its opens, so a play can
/// behave differently the second time.
#[derive(Debug)]
pub struct SimSource {
    play: Play,
    opens: AtomicUsize,
}

impl SimSource {
    pub fn new(play: Play) -> Self {
        Self {
            play,
            opens: AtomicUsize::new(0),
        }
    }
}

fn at(after_ms: i64, event: LinkEvent) -> Timed {
    Timed { after_ms, event }
}

fn attach(stream: Stream) -> Vec<Timed> {
    vec![
        at(0, LinkEvent::Attached),
        at(0, LinkEvent::Output(stream.bytes())),
    ]
}

impl LinkSource for SimSource {
    fn open(&self, _: &PaneRequest, _: PaneSize) -> Result<Box<dyn PaneLink>, String> {
        let again = self.opens.fetch_add(1, Ordering::Relaxed) > 0;
        let steps = match &self.play {
            Play::Refuses(reason) => return Err(reason.clone()),
            Play::Hangs => vec![],
            Play::Stays(stream) => attach(*stream),
            Play::Watches(stream) => vec![
                at(0, LinkEvent::Attached),
                at(0, LinkEvent::Lease(Lease::ReadOnly)),
                at(
                    0,
                    LinkEvent::Notice("another client controls this pane".into()),
                ),
                at(0, LinkEvent::Output(stream.bytes())),
            ],
            Play::Detaches(stream) => {
                let mut steps = attach(*stream);
                if !again {
                    steps.push(at(1_000, LinkEvent::Ended(EndKind::Detached)));
                }
                steps
            }
            Play::Exits(stream) => {
                let mut steps = attach(*stream);
                steps.push(at(1_000, LinkEvent::Exited(Some(0))));
                steps
            }
        };
        Ok(Box::new(SimLink::new(steps)))
    }

    fn restart(&self, request: &PaneRequest, size: PaneSize) -> Result<Box<dyn PaneLink>, String> {
        match &self.play {
            Play::Exits(stream) => Ok(Box::new(SimLink::new(attach(*stream)))),
            _ => self.open(request, size),
        }
    }
}

fn codex_screen() -> Vec<u8> {
    [
        "\x1b]2;payments-platform · gpt-5-codex\x07",
        "\x1b[2J\x1b[H",
        "\x1b[1;38;5;75m>_ Codex\x1b[0m \x1b[2m(v0.9.2)\x1b[0m\r\n",
        "\x1b[2mmodel:\x1b[0m     gpt-5-codex\r\n",
        "\x1b[2mdirectory:\x1b[0m /sandbox/project\r\n\r\n",
        "\x1b[1;36m›\x1b[0m fix the flaky retry test in settlement\r\n\r\n",
        "\x1b[38;2;120;200;120m●\x1b[0m Read \x1b[4msrc/settlement/retry.rs\x1b[0m\r\n",
        "  \x1b[2m日本語のコメントは残します\x1b[0m\r\n",
        "\x1b[38;2;230;180;60m●\x1b[0m Edit \x1b[1m2 files\x1b[0m\r\n",
        "  \x1b[31m- backoff(500)\x1b[0m\r\n",
        "  \x1b[32m+ backoff(attempt * 500)\x1b[0m\r\n\r\n",
        "\x1b[1;36m›\x1b[0m ",
    ]
    .concat()
    .into_bytes()
}

fn shell_screen() -> Vec<u8> {
    [
        "\x1b[2J\x1b[H",
        "\x1b[32mdev@sandbox\x1b[0m:\x1b[34m/sandbox/project\x1b[0m$ ls\r\n",
        "\x1b[1;34mscripts\x1b[0m  \x1b[1;34msrc\x1b[0m  Cargo.toml  README.md\r\n",
        "\x1b[32mdev@sandbox\x1b[0m:\x1b[34m/sandbox/project\x1b[0m$ ",
    ]
    .concat()
    .into_bytes()
}

fn pager_screen() -> Vec<u8> {
    [
        "\x1b[?1049h\x1b[2J\x1b[H\x1b[?25l",
        "\x1b[7m top · 3 tasks · load 0.42 \x1b[0m\r\n",
        "  PID  USER  CMD\r\n",
        " 1204  dev   \x1b[32mcodex\x1b[0m\r\n",
        " 1311  dev   cargo test\r\n",
        "\x1b[24;1H\x1b[2m q quit\x1b[0m",
    ]
    .concat()
    .into_bytes()
}
