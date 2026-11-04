//! A pane that shows a program running on the other side of a `PaneLink`.

use std::collections::VecDeque;
use std::sync::Arc;

use junie_tui::core::event::{Key, Outcome};
use junie_tui::core::hit::Axis;
use junie_tui::core::id::WidgetId;
use junie_tui::terminal::encode_key;
use junie_tui::terminal::{ForwardPolicy, Paste, TermEvent, TerminalState, TerminalViewport};
use junie_tui::theme::Tone;
use junie_tui::ui::text::{clip, plural};
use junie_tui::widgets::viewport_ops::ViewportOps;
use ratatui::crossterm::event::{KeyCode, KeyModifiers};
use ratatui::layout::Rect;

use super::clipboard::{ClipContext, ClipDecision, ClipRefusal, decide};
use super::link::{
    DetachCause, EndKind, Lease, LinkEvent, LinkFault, LinkSource, LinkStatus, PaneCommand,
    PaneLink, PaneSize, Sent,
};
use super::policy::{PanePolicy, PaneTiming};
use super::{
    MIN_PANE_COLS, MIN_PANE_ROWS, PaneBody, PaneEffect, PaneFace, PaneId, PaneKey, PaneKindTag,
    PaneNote, PaneRequest, PasteOutcome, SCROLLBACK,
};
use crate::domain::instance::AgentState;
use crate::domain::vocabulary::{PaneWords, fill_in};

const FOCUS_IN: &[u8] = b"\x1b[I";
const FOCUS_OUT: &[u8] = b"\x1b[O";
const TITLE_COLUMNS: usize = 64;
const PENDING_EFFECTS: usize = 16;
const BIDI: [char; 9] = [
    '\u{202a}', '\u{202b}', '\u{202c}', '\u{202d}', '\u{202e}', '\u{2066}', '\u{2067}', '\u{2068}',
    '\u{2069}',
];

/// The title a program set, made safe to show: controls and bidirectional
/// controls are dropped, runs of white space become one space, and the result
/// is cut at 64 columns.
pub fn sanitize_title(raw: &str) -> String {
    let spaced: String = raw
        .chars()
        .map(|c| if c.is_whitespace() { ' ' } else { c })
        .filter(|c| !c.is_control() && !BIDI.contains(c))
        .collect();
    let collapsed = spaced.split_whitespace().collect::<Vec<_>>().join(" ");
    clip(&collapsed, TITLE_COLUMNS)
}

/// The window after an attach in which the replay of the screen arrives. The
/// pane shows a placeholder and answers no query until it closes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Replay {
    Idle,
    Open { since: i64, quiet_from: i64 },
    Closed,
}

impl Replay {
    fn is_open(self) -> bool {
        matches!(self, Replay::Open { .. })
    }

    fn open(&mut self, now: i64) {
        *self = Replay::Open {
            since: now,
            quiet_from: now,
        };
    }

    fn output(&mut self, now: i64) {
        if let Replay::Open { quiet_from, .. } = self {
            *quiet_from = now;
        }
    }

    /// Closes the window when the time has come; returns whether it did.
    fn tick(&mut self, now: i64, timing: &PaneTiming) -> bool {
        let Replay::Open { since, quiet_from } = *self else {
            return false;
        };
        let capped = now >= since + timing.replay_cap_ms;
        let quiet = now >= since + timing.nudge_ms && now >= quiet_from + timing.quiet_ms;
        if capped || quiet {
            *self = Replay::Closed;
        }
        capped || quiet
    }
}

/// How a pane asks its source for a link: the same program again, or a new
/// run of one that exited.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
enum Attempt {
    #[default]
    Again,
    Restart,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Say {
    Detached(DetachCause),
    Failed(LinkFault),
    Notice(String),
    Blocked(ClipRefusal),
    Copied { text: String },
}

#[derive(Debug)]
pub struct TerminalPane {
    request: PaneRequest,
    source: Arc<dyn LinkSource>,
    link: Option<Box<dyn PaneLink>>,
    status: LinkStatus,
    lease: Lease,
    view: TerminalViewport,
    area: Rect,
    wanted: Option<PaneSize>,
    forwarded: Option<PaneSize>,
    too_small: bool,
    title: Option<String>,
    policy: Arc<PanePolicy>,
    timing: PaneTiming,
    forward: ForwardPolicy,
    focused: bool,
    attention: bool,
    last_gesture_ms: Option<i64>,
    last_refusal_ms: Option<i64>,
    replay: Replay,
    attempt: Attempt,
    attached_once: bool,
    has_output: bool,
    dropped_replies: usize,
    said: VecDeque<Say>,
    now_ms: i64,
}

impl TerminalPane {
    pub fn new(
        id: PaneId,
        request: PaneRequest,
        source: Arc<dyn LinkSource>,
        timing: PaneTiming,
        now_ms: i64,
    ) -> Self {
        let policy = Arc::new(PanePolicy::default());
        let state = TerminalState::with_scrollback(24, 80, SCROLLBACK);
        let view = TerminalViewport::new(WidgetId::of("capsule.pane").child(id as usize), state);
        Self {
            request,
            source,
            link: None,
            status: LinkStatus::Pending,
            lease: Lease::Owner,
            view,
            area: Rect::ZERO,
            wanted: None,
            forwarded: None,
            too_small: false,
            title: None,
            forward: ForwardPolicy::new(policy.paste_max_bytes),
            policy,
            timing,
            focused: false,
            attention: false,
            last_gesture_ms: None,
            last_refusal_ms: None,
            replay: Replay::Idle,
            attempt: Attempt::Again,
            attached_once: false,
            has_output: false,
            dropped_replies: 0,
            said: VecDeque::new(),
            now_ms,
        }
    }

    pub fn status(&self) -> &LinkStatus {
        &self.status
    }

    /// The screen of the program: the viewport that owns its grid.
    pub fn grid(&self) -> &TerminalViewport {
        &self.view
    }

    /// Replies a full queue made the pane drop.
    pub fn dropped_replies(&self) -> usize {
        self.dropped_replies
    }

    fn agent(&self) -> bool {
        self.request.agent().is_some()
    }

    fn label(&self, words: &PaneWords) -> String {
        self.request
            .agent()
            .map_or_else(|| words.shell_label.clone(), |a| a.label().to_owned())
    }

    fn live(&self) -> bool {
        self.link.is_some()
            && matches!(self.status, LinkStatus::Attaching | LinkStatus::Attached(_))
    }

    fn say(&mut self, say: Say) {
        if self.said.len() == PENDING_EFFECTS {
            self.said.pop_front();
        }
        self.said.push_back(say);
    }

    fn send(&mut self, command: PaneCommand) -> Sent {
        match &self.link {
            Some(link) => link.send(command),
            None => Sent::Closed,
        }
    }

    fn send_input(&mut self, bytes: Vec<u8>, now_ms: i64) -> Sent {
        if !self.live() {
            return Sent::Closed;
        }
        let sent = self.send(PaneCommand::Input(bytes));
        if sent == Sent::Queued {
            self.last_gesture_ms = Some(now_ms);
        }
        sent
    }

    /// Whether the output now is the replay of the screen, not the program
    /// speaking: what the operator did before the attach says nothing about it.
    fn replaying(&self) -> bool {
        self.replay.is_open() || self.status == LinkStatus::Attaching
    }

    fn fresh_grid(&mut self, size: PaneSize) {
        let state = TerminalState::with_scrollback(size.rows, size.cols, SCROLLBACK);
        let mut view = TerminalViewport::new(self.view.id, state);
        view.set_area(self.area);
        self.view = view;
        self.title = None;
        self.has_output = false;
    }

    fn open_link(&mut self, attempt: Attempt) {
        let Some(size) = self.wanted else { return };
        self.link = None;
        self.fresh_grid(size);
        self.attached_once = false;
        self.lease = Lease::Owner;
        self.replay = Replay::Idle;
        self.last_gesture_ms = None;
        let opened = match attempt {
            Attempt::Again => self.source.open(&self.request, size),
            Attempt::Restart => self.source.restart(&self.request, size),
        };
        match opened {
            Ok(link) => {
                self.link = Some(link);
                self.forwarded = Some(size);
                self.status = LinkStatus::Attaching;
            }
            Err(fault) => {
                self.forwarded = None;
                self.status = LinkStatus::Failed(fault.clone());
                self.say(Say::Failed(fault));
            }
        }
    }

    /// Builds a new link and a new screen on this pane, at the size the pane
    /// has now. A pane below the floor waits for room and opens at the first
    /// size that fits.
    fn relink(&mut self, attempt: Attempt) {
        if self.too_small {
            self.link = None;
            self.status = LinkStatus::Pending;
            self.attempt = attempt;
        } else {
            self.open_link(attempt);
        }
    }

    fn pump(&mut self, now_ms: i64) -> bool {
        let Some(link) = &self.link else {
            return false;
        };
        let events = link.drain(now_ms, self.policy.feed_budget_bytes);
        let mut changed = !events.is_empty();
        for event in events {
            self.handle(event, now_ms);
        }
        if self.link.is_some() && self.replay.tick(now_ms, &self.timing) {
            self.status = LinkStatus::Attached(self.lease);
            changed = true;
        }
        changed
    }

    fn handle(&mut self, event: LinkEvent, now_ms: i64) {
        match event {
            LinkEvent::Attached => {
                self.attached_once = true;
                self.replay.open(now_ms);
            }
            LinkEvent::Lease(lease) => {
                self.lease = lease;
                if matches!(self.status, LinkStatus::Attached(_)) {
                    self.status = LinkStatus::Attached(lease);
                }
            }
            LinkEvent::Output(bytes) => {
                self.has_output = true;
                self.replay.output(now_ms);
                let events = self.view.feed(&bytes);
                for event in events {
                    self.term_event(event, now_ms);
                }
            }
            LinkEvent::Notice(text) => {
                let clean: String = text.chars().filter(|c| !c.is_control()).collect();
                if !clean.is_empty() {
                    self.say(Say::Notice(clean));
                }
            }
            LinkEvent::Exited(code) => {
                self.link = None;
                self.replay = Replay::Closed;
                self.status = LinkStatus::Exited(code);
            }
            LinkEvent::Ended(kind) => self.ended(kind),
        }
    }

    fn ended(&mut self, kind: EndKind) {
        self.link = None;
        self.replay = Replay::Closed;
        self.status = match kind {
            EndKind::Detached => LinkStatus::Detached(DetachCause::Operator),
            EndKind::StreamClosed | EndKind::Failed(_) if self.attached_once => {
                LinkStatus::Detached(DetachCause::GatewayLost)
            }
            EndKind::StreamClosed => LinkStatus::Failed(LinkFault::new("")),
            EndKind::Failed(fault) | EndKind::Refused(fault) => LinkStatus::Failed(fault),
        };
        match &self.status {
            LinkStatus::Detached(cause) => self.say(Say::Detached(*cause)),
            LinkStatus::Failed(fault) => {
                let fault = fault.clone();
                self.say(Say::Failed(fault));
            }
            _ => {}
        }
    }

    fn term_event(&mut self, event: TermEvent, now_ms: i64) {
        match event {
            TermEvent::Reply(bytes) => {
                if self.replay.is_open() {
                    return;
                }
                if self.send(PaneCommand::Reply(bytes)) == Sent::Full {
                    self.dropped_replies += 1;
                }
            }
            TermEvent::Title(raw) => {
                let title = sanitize_title(&raw);
                self.title = (!title.is_empty()).then_some(title);
            }
            TermEvent::ClipboardWrite(payload) => {
                if !self.replaying() {
                    self.clipboard(&payload, now_ms);
                }
            }
            TermEvent::Bell | TermEvent::Notification(_) => {
                if !self.focused && !self.replaying() {
                    self.attention = true;
                }
            }
            TermEvent::Cwd(_) | TermEvent::ScrollbackCleared => {}
        }
    }

    fn clipboard(&mut self, payload: &str, now_ms: i64) {
        let ctx = ClipContext {
            agent: self.agent(),
            focused: self.focused,
            last_gesture_ms: self.last_gesture_ms,
            now_ms,
        };
        match decide(&self.policy, &ctx, payload) {
            ClipDecision::Allow(text) => self.say(Say::Copied { text }),
            ClipDecision::Drop => {}
            ClipDecision::Refuse(refusal) => {
                let every = i64::from(self.policy.refusal_status_every_ms);
                let due = self
                    .last_refusal_ms
                    .is_none_or(|at| now_ms.saturating_sub(at) >= every);
                if due {
                    self.last_refusal_ms = Some(now_ms);
                    self.say(Say::Blocked(refusal));
                }
            }
        }
    }

    fn note_text(&self, words: &PaneWords) -> Option<(String, Tone)> {
        let agent = self.label(words);
        match &self.status {
            LinkStatus::Failed(fault) => {
                return Some((words.failure(&agent, fault), Tone::Error));
            }
            LinkStatus::Exited(code) => {
                let tone = if *code == Some(0) {
                    Tone::Muted
                } else {
                    Tone::Warning
                };
                let code =
                    code.map_or_else(|| words.exited_unknown_code.clone(), |c| c.to_string());
                return Some((
                    fill_in(&words.exited, &[("agent", &agent), ("code", &code)]),
                    tone,
                ));
            }
            LinkStatus::Detached(DetachCause::Operator) => {
                return Some((
                    fill_in(&words.detached, &[("agent", &agent)]),
                    Tone::Warning,
                ));
            }
            LinkStatus::Detached(DetachCause::GatewayLost) => {
                return Some((words.connection_lost.clone(), Tone::Warning));
            }
            _ => {}
        }
        if self.too_small {
            return Some((words.too_small.clone(), Tone::Warning));
        }
        matches!(self.status, LinkStatus::Attached(Lease::ReadOnly))
            .then(|| (words.viewing.clone(), Tone::Muted))
    }
}

impl PaneBody for TerminalPane {
    fn kind(&self) -> PaneKindTag {
        if self.agent() {
            PaneKindTag::Agent
        } else {
            PaneKindTag::Shell
        }
    }

    fn state(&self) -> AgentState {
        match &self.status {
            _ if self.attention => AgentState::Blocked,
            LinkStatus::Exited(Some(0)) => AgentState::Done,
            LinkStatus::Exited(Some(_)) | LinkStatus::Failed(_) => AgentState::Blocked,
            _ => AgentState::Unknown,
        }
    }

    fn title(&self) -> Option<&str> {
        self.title.as_deref()
    }

    fn note(&self, words: &PaneWords) -> Option<PaneNote> {
        self.note_text(words)
            .map(|(text, tone)| PaneNote { text, tone })
    }

    fn suffix(&self, words: &PaneWords) -> Option<String> {
        matches!(self.status, LinkStatus::Attached(Lease::ReadOnly))
            .then(|| words.viewing_suffix.clone())
    }

    fn face(&self, words: &PaneWords) -> PaneFace {
        let agent = self.label(words);
        match &self.status {
            LinkStatus::Pending => PaneFace::Empty {
                title: words.waiting_title.clone(),
                hint: Some(fill_in(&words.waiting_hint, &[("agent", &agent)])),
            },
            LinkStatus::Attaching => PaneFace::Empty {
                title: fill_in(&words.attaching_title, &[("agent", &agent)]),
                hint: Some(words.attaching_hint.clone()),
            },
            LinkStatus::Detached(_) => PaneFace::Dimmed,
            LinkStatus::Failed(_) if !self.has_output => PaneFace::Empty {
                title: fill_in(&words.unattached_title, &[("agent", &agent)]),
                hint: None,
            },
            LinkStatus::Attached(_) | LinkStatus::Exited(_) | LinkStatus::Failed(_) => {
                PaneFace::Grid
            }
        }
    }

    fn viewport(&self) -> &dyn ViewportOps {
        &self.view
    }

    fn accepts_input(&self) -> bool {
        self.live()
    }

    fn viewport_mut(&mut self) -> &mut dyn ViewportOps {
        &mut self.view
    }

    fn key(&mut self, key: &Key, now_ms: i64) -> PaneKey {
        self.now_ms = now_ms;
        if key.code == KeyCode::Enter && key.mods.difference(KeyModifiers::SHIFT).is_empty() {
            match self.status {
                LinkStatus::Detached(_) | LinkStatus::Failed(_) => {
                    self.relink(Attempt::Again);
                    return PaneKey::Consumed;
                }
                LinkStatus::Exited(_) => {
                    self.relink(Attempt::Restart);
                    return PaneKey::Consumed;
                }
                _ => {}
            }
        }
        if !self.live() {
            return PaneKey::Consumed;
        }
        self.view.clear_selection();
        self.view.set_follow(true);
        let modes = self.view.state().modes();
        let Some(bytes) = encode_key(key, &modes).and_then(|b| self.forward.key(b)) else {
            return PaneKey::Consumed;
        };
        if self.send_input(bytes, now_ms) == Sent::Queued {
            PaneKey::Forwarded
        } else {
            PaneKey::Consumed
        }
    }

    fn gesture(&mut self, now_ms: i64) {
        self.now_ms = now_ms;
        if self.live() {
            self.last_gesture_ms = Some(now_ms);
        }
    }

    fn wheel(&mut self, axis: Axis, delta: i32) -> Option<Outcome> {
        let modes = self.view.state().modes();
        if !modes.alt_screen {
            return None;
        }
        if axis != Axis::Vertical || self.view.state().mouse_reporting() || delta == 0 {
            return Some(Outcome::Consumed);
        }
        let code = if delta < 0 {
            KeyCode::Up
        } else {
            KeyCode::Down
        };
        let arrow = Key {
            code,
            mods: KeyModifiers::NONE,
        };
        let bytes = encode_key(&arrow, &modes)?;
        for _ in 0..delta.unsigned_abs() {
            if self.send(PaneCommand::Input(bytes.clone())) != Sent::Queued {
                break;
            }
        }
        Some(Outcome::Changed)
    }

    fn paste(&mut self, text: &str, now_ms: i64) -> PasteOutcome {
        self.now_ms = now_ms;
        if !self.live() {
            return PasteOutcome::Dropped;
        }
        let bracketed = self.view.state().modes().bracketed_paste;
        self.paste_with(self.forward.paste(text, bracketed), now_ms)
    }

    fn paste_confirmed(&mut self, text: &str, now_ms: i64) -> PasteOutcome {
        self.now_ms = now_ms;
        if !self.live() {
            return PasteOutcome::Dropped;
        }
        let bracketed = self.view.state().modes().bracketed_paste;
        let decided = if bracketed {
            self.forward.paste(text, true)
        } else {
            self.forward.confirmed(text)
        };
        self.paste_with(decided, now_ms)
    }

    fn tick(&mut self, now_ms: i64) -> bool {
        self.now_ms = now_ms;
        self.pump(now_ms)
    }

    fn take_effects(&mut self, words: &PaneWords) -> Vec<PaneEffect> {
        let agent = self.label(words);
        self.said
            .drain(..)
            .map(|say| match say {
                Say::Detached(DetachCause::Operator) => PaneEffect::Status {
                    text: fill_in(&words.detached, &[("agent", &agent)]),
                    tone: Tone::Warning,
                },
                Say::Detached(DetachCause::GatewayLost) => PaneEffect::Status {
                    text: words.connection_lost.clone(),
                    tone: Tone::Warning,
                },
                Say::Failed(fault) => PaneEffect::Status {
                    text: words.failure(&agent, &fault),
                    tone: Tone::Error,
                },
                Say::Notice(text) => PaneEffect::Status {
                    text,
                    tone: Tone::Secondary,
                },
                Say::Blocked(refusal) => {
                    let template = match refusal {
                        ClipRefusal::Write => &words.clip_write_blocked,
                        ClipRefusal::Read => &words.clip_read_blocked,
                        ClipRefusal::TooLarge => &words.clip_too_large,
                    };
                    PaneEffect::Status {
                        text: fill_in(template, &[("source", &agent)]),
                        tone: Tone::Warning,
                    }
                }
                Say::Copied { text } => {
                    let chars = plural(text.chars().count(), "character", "characters");
                    PaneEffect::Copy {
                        what: fill_in(&words.copied_from, &[("source", &agent), ("chars", &chars)]),
                        text,
                    }
                }
            })
            .collect()
    }

    fn set_policy(&mut self, policy: &Arc<PanePolicy>) {
        self.forward = ForwardPolicy::new(policy.paste_max_bytes);
        self.policy = Arc::clone(policy);
    }

    fn sync_size(&mut self, inner: Rect) {
        self.area = inner;
        self.view.set_area(inner);
        let (rows, cols) = TerminalViewport::grid_size(inner);
        let fits = cols >= MIN_PANE_COLS && rows >= MIN_PANE_ROWS;
        self.too_small = !fits;
        if !fits {
            return;
        }
        let size = PaneSize { rows, cols };
        self.wanted = Some(size);
        match self.status {
            LinkStatus::Pending => {
                let attempt = std::mem::take(&mut self.attempt);
                self.open_link(attempt);
            }
            LinkStatus::Attaching | LinkStatus::Attached(_) if self.forwarded != Some(size) => {
                self.view.resize(rows, cols);
                if let Some(link) = &self.link {
                    link.set_size(size);
                }
                self.forwarded = Some(size);
            }
            _ => {}
        }
    }

    fn focus_changed(&mut self, focused: bool, now_ms: i64) {
        self.now_ms = now_ms;
        if focused == self.focused {
            return;
        }
        self.focused = focused;
        if focused {
            self.attention = false;
        }
        if self.live() && self.view.state().focus_events() {
            let report = if focused { FOCUS_IN } else { FOCUS_OUT };
            self.send(PaneCommand::Input(report.to_vec()));
        }
    }

    fn redraw(&mut self) {
        if self.live() {
            self.send(PaneCommand::Redraw);
        }
    }

    fn clear(&mut self) {
        self.view.clear_scrollback();
    }

    fn touched(&self) -> &[String] {
        &[]
    }

    fn sends_prefix(&self) -> bool {
        true
    }

    fn close_needs_confirm(&self) -> bool {
        matches!(self.status, LinkStatus::Attached(_))
    }
}

impl TerminalPane {
    fn paste_with(&mut self, decided: Paste, now_ms: i64) -> PasteOutcome {
        match decided {
            Paste::Send(bytes) if bytes.is_empty() => PasteOutcome::Sent,
            Paste::Send(bytes) => match self.send_input(bytes, now_ms) {
                Sent::Queued => PasteOutcome::Sent,
                Sent::Full => PasteOutcome::Full,
                Sent::Closed => PasteOutcome::Dropped,
            },
            Paste::NeedsConfirm { lines } => PasteOutcome::NeedsConfirm { lines },
            Paste::TooLarge { .. } => PasteOutcome::TooLarge,
        }
    }
}
