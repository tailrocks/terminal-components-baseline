use std::collections::VecDeque;
use std::sync::Arc;

use junie_tui::core::event::Key;
use junie_tui::core::id::WidgetId;
use junie_tui::theme::Tone;
use junie_tui::ui::layout::SplitDir;
use junie_tui::widgets::viewport::{Line, Span, TextViewport};
use junie_tui::widgets::viewport_ops::ViewportOps;
use ratatui::crossterm::event::KeyCode;
use ratatui::layout::Rect;

use crate::domain::account::AccountId;
use crate::domain::agent::Agent;
use crate::domain::instance::{AgentState, InstanceId};

pub use junie_tui::ui::pane_tree::PaneId;

pub const SCROLLBACK: usize = 2_000;
pub const MIN_PANE_COLS: u16 = 20;
pub const MIN_PANE_ROWS: u16 = 4;
pub const MAX_LABEL: usize = 16;

pub use junie_tui::ui::pane_tree::{Direction, PaneNode, Seam, nearest};

#[derive(Debug, Clone)]
pub struct Tab {
    pub custom_label: Option<String>,
    pub root: PaneNode,
    pub focused: PaneId,
    pub zoomed: Option<PaneId>,
}

impl Tab {
    pub fn leaves(&self) -> Vec<PaneId> {
        self.root.leaves()
    }
}

pub trait SessionScript: std::fmt::Debug + Send + Sync {
    fn script(&self, agent: Option<Agent>, workspace: &str) -> Vec<Step>;

    fn respond(
        &self,
        proc: &mut AgentProcess,
        input: &str,
        now_ms: i64,
        workspace: &str,
    ) -> Vec<Line>;
}

#[derive(Debug, Clone)]
pub enum Step {
    Emit(i64, Line),
    State(AgentState),

    Touch(&'static str),

    Await,
}

#[derive(Debug, Clone)]
pub struct AgentProcess {
    pub agent: Option<Agent>,
    pub account: Option<AccountId>,
    pub state: AgentState,
    pub script: Vec<Step>,
    pub pc: usize,
    pub next_at_ms: i64,

    scheduled: bool,
    scripts: Arc<dyn SessionScript>,
    pub reply_queue: VecDeque<(i64, Line)>,
    pub reply_cursor: usize,
    pub prompt: String,
    pub cursor_hidden: bool,
    pub awaiting_permission: bool,
    pub touched: Vec<String>,
}

fn sp(text: &str, tone: Tone) -> Span {
    Span::new(text, tone)
}

impl AgentProcess {
    pub fn new(
        agent: Option<Agent>,
        account: Option<AccountId>,
        workspace: &str,
        start_ms: i64,
        scripts: Arc<dyn SessionScript>,
    ) -> Self {
        let prompt = match agent {
            Some(Agent::ClaudeCode) => "❯ ".to_owned(),
            Some(Agent::Codex) => "❯ ".to_owned(),
            Some(Agent::OpenCode) | Some(Agent::GrokBuild) => "> ".to_owned(),
            Some(_) => "› ".to_owned(),
            None => format!("{workspace} ❯ "),
        };
        Self {
            agent,
            account,
            state: if agent.is_some() {
                AgentState::Working
            } else {
                AgentState::Unknown
            },
            script: scripts.script(agent, workspace),
            pc: 0,
            next_at_ms: start_ms,
            scheduled: false,
            scripts,
            reply_queue: VecDeque::new(),
            reply_cursor: 0,
            prompt,
            cursor_hidden: false,
            awaiting_permission: false,
            touched: vec![],
        }
    }

    pub fn boot_all(&mut self, out: &mut Vec<Line>) {
        while self.pc < self.script.len() {
            match &self.script[self.pc] {
                Step::Emit(_, l) => out.push(l.clone()),
                Step::State(s) => self.state = *s,
                Step::Touch(p) => self.touched.push((*p).to_owned()),
                Step::Await => {
                    self.pc += 1;
                    self.awaiting_permission = self.state == AgentState::Blocked;
                    return;
                }
            }
            self.pc += 1;
        }
    }

    pub fn awaiting(&self) -> bool {
        self.pc > 0
            && matches!(self.script.get(self.pc - 1), Some(Step::Await))
            && self.reply_queue.is_empty()
    }

    pub fn tick(&mut self, now_ms: i64) -> Vec<Line> {
        let mut out = vec![];
        while let Some((due, _)) = self.reply_queue.front() {
            if *due <= now_ms {
                let (_, l) = self.reply_queue.pop_front().unwrap();
                out.push(l);
            } else {
                break;
            }
        }
        if self.reply_queue.is_empty()
            && self.state == AgentState::Working
            && self.awaiting()
            && !self.awaiting_permission
        {
            self.state = AgentState::Done;
        }
        while self.pc < self.script.len() {
            match self.script[self.pc].clone() {
                Step::Emit(delay, l) => {
                    if !self.scheduled {
                        self.next_at_ms += delay;
                        self.scheduled = true;
                    }
                    if now_ms < self.next_at_ms {
                        break;
                    }
                    self.scheduled = false;
                    out.push(l);
                    self.pc += 1;
                }
                Step::State(s) => {
                    self.state = s;
                    self.pc += 1;
                }
                Step::Touch(p) => {
                    self.touched.push(p.to_owned());
                    self.pc += 1;
                }
                Step::Await => {
                    self.awaiting_permission = self.state == AgentState::Blocked;
                    self.pc += 1;
                    break;
                }
            }
        }
        out
    }

    pub fn on_input(&mut self, input: &str, now_ms: i64, workspace: &str) -> Vec<Line> {
        let scripts = self.scripts.clone();
        scripts.respond(self, input, now_ms, workspace)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PaneKindTag {
    Agent,
    Shell,
    Script,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PaneKey {
    /// The capsule swallowed the key: nothing reached the pane.
    Consumed,
    /// The pane took the key.
    Forwarded,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PaneNote {
    pub text: String,
    pub tone: Tone,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PasteOutcome {
    Sent,
    NeedsConfirm { lines: usize },
    TooLarge,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PaneKind {
    Agent {
        agent: Agent,
        account: Option<AccountId>,
    },
    Shell,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PaneRequest {
    pub instance: InstanceId,
    pub kind: PaneKind,
}

impl PaneRequest {
    pub fn new(instance: &str, agent: Option<Agent>, account: Option<AccountId>) -> Self {
        Self {
            instance: instance.to_owned(),
            kind: match agent {
                Some(agent) => PaneKind::Agent { agent, account },
                None => PaneKind::Shell,
            },
        }
    }

    pub fn agent(&self) -> Option<Agent> {
        match self.kind {
            PaneKind::Agent { agent, .. } => Some(agent),
            PaneKind::Shell => None,
        }
    }

    pub fn account(&self) -> Option<&AccountId> {
        match &self.kind {
            PaneKind::Agent { account, .. } => account.as_ref(),
            PaneKind::Shell => None,
        }
    }
}

/// What the daemon decides before a body exists and hands to the backend that
/// builds it: the id the pane will have, the daemon's workspace name and the
/// time of the request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PaneSpawn {
    pub id: PaneId,
    pub workspace: String,
    pub now_ms: i64,
}

/// The inside of a pane. The capsule draws, sizes, scrolls and types into a
/// pane only through this trait, so it does not know what runs behind it.
///
/// There is no `Send` bound: `TextViewport` holds an `Rc` (the edit menu's
/// validator), and the `World` that owns the panes is not `Send` either.
pub trait PaneBody: std::fmt::Debug {
    fn kind(&self) -> PaneKindTag;
    fn state(&self) -> AgentState;
    fn title(&self) -> Option<&str>;
    fn note(&self) -> Option<PaneNote>;
    fn viewport(&self) -> &dyn ViewportOps;
    fn viewport_mut(&mut self) -> &mut dyn ViewportOps;
    fn key(&mut self, key: &Key, now_ms: i64) -> PaneKey;
    fn paste(&mut self, text: &str, now_ms: i64) -> PasteOutcome;
    fn tick(&mut self, now_ms: i64) -> bool;
    fn sync_size(&mut self, inner: Rect);
    fn focus_changed(&mut self, focused: bool, now_ms: i64);
    fn redraw(&mut self);
    fn clear(&mut self);
    fn touched(&self) -> &[String];
    fn sends_prefix(&self) -> bool;
    fn close_needs_confirm(&self) -> bool;
}

#[derive(Debug)]
pub struct ScriptPane {
    proc: AgentProcess,
    term: TextViewport,
    input: String,
    workspace: String,
}

impl ScriptPane {
    pub fn new(request: &PaneRequest, spawn: &PaneSpawn, scripts: Arc<dyn SessionScript>) -> Self {
        let mut term = TextViewport::new(WidgetId::of("capsule.pane").child(spawn.id as usize))
            .max_lines(SCROLLBACK);
        term.follow = true;
        Self {
            proc: AgentProcess::new(
                request.agent(),
                request.account().cloned(),
                &spawn.workspace,
                spawn.now_ms,
                scripts,
            ),
            term,
            input: String::new(),
            workspace: spawn.workspace.clone(),
        }
    }

    pub fn booted(mut self) -> Self {
        let mut out = vec![];
        self.proc.boot_all(&mut out);
        self.push_lines(out);
        self.refresh_prompt();
        self
    }

    pub fn with_history_above(mut self, mut history: Vec<Line>) -> Self {
        history.extend(self.term.lines().cloned());
        self.term.set_lines(history);
        self
    }

    fn prompt_line(&self) -> Line {
        vec![
            sp(&self.proc.prompt, Tone::Secondary),
            sp(&self.input, Tone::Normal),
        ]
    }

    fn refresh_prompt(&mut self) {
        let waiting = self.proc.awaiting() || !self.input.is_empty();
        if waiting {
            let last_is_prompt = self
                .term
                .last()
                .is_some_and(|l| l.first().is_some_and(|s| s.text == self.proc.prompt));
            let pl = self.prompt_line();
            if last_is_prompt {
                self.term.replace_last(pl);
            } else {
                self.term.push(pl);
            }
            let n = self.term.len().saturating_sub(1);
            let col = junie_tui::ui::text::width(&self.proc.prompt)
                + junie_tui::ui::text::width(&self.input);
            self.term.caret = Some(junie_tui::widgets::viewport::CellPos { line: n, col });
        } else {
            self.term.caret = None;
        }
        self.term.caret_visible = !self.proc.cursor_hidden;
    }

    fn push_lines(&mut self, lines: Vec<Line>) {
        if lines.is_empty() {
            return;
        }

        if self
            .term
            .last()
            .is_some_and(|l| l.first().is_some_and(|s| s.text == self.proc.prompt))
        {
            self.term.pop();
        }
        for l in lines {
            if l.first().is_some_and(|s| s.text == "\u{0}clear") {
                self.term.clear();
                continue;
            }
            if l.first().is_some_and(|s| s.text == "⠋ ") {
                self.term.push(l);
                continue;
            }

            if self
                .term
                .last()
                .is_some_and(|x| x.first().is_some_and(|s| s.text == "⠋ "))
            {
                self.term.pop();
            }
            self.term.push(l);
        }
        self.refresh_prompt();
    }

    fn type_char(&mut self, c: char) {
        self.term.set_follow(true);
        self.input.push(c);
        self.refresh_prompt();
    }

    fn backspace(&mut self) {
        self.term.set_follow(true);
        self.input.pop();
        self.refresh_prompt();
    }

    fn commit(&mut self, now_ms: i64) {
        self.term.set_follow(true);
        let text = std::mem::take(&mut self.input);
        if self
            .term
            .last()
            .is_some_and(|l| l.first().is_some_and(|s| s.text == self.proc.prompt))
        {
            self.term.pop();
        }
        let out = self.proc.on_input(&text, now_ms, &self.workspace);
        self.push_lines(out);
        self.refresh_prompt();
    }
}

impl PaneBody for ScriptPane {
    fn kind(&self) -> PaneKindTag {
        PaneKindTag::Script
    }

    fn state(&self) -> AgentState {
        self.proc.state
    }

    fn title(&self) -> Option<&str> {
        None
    }

    fn note(&self) -> Option<PaneNote> {
        None
    }

    fn viewport(&self) -> &dyn ViewportOps {
        &self.term
    }

    fn viewport_mut(&mut self) -> &mut dyn ViewportOps {
        &mut self.term
    }

    fn key(&mut self, key: &Key, now_ms: i64) -> PaneKey {
        match key.code {
            KeyCode::Char(c) if !key.chord() => {
                self.term.clear_selection();
                self.type_char(c);
            }
            KeyCode::Backspace => self.backspace(),
            KeyCode::Enter => self.commit(now_ms),
            KeyCode::Esc => {
                self.term.set_follow(true);
            }
            _ => return PaneKey::Consumed,
        }
        PaneKey::Forwarded
    }

    fn paste(&mut self, text: &str, _now_ms: i64) -> PasteOutcome {
        for c in text.chars() {
            self.type_char(c);
        }
        PasteOutcome::Sent
    }

    fn tick(&mut self, now_ms: i64) -> bool {
        let out = self.proc.tick(now_ms);
        let changed = !out.is_empty();
        self.push_lines(out);
        self.refresh_prompt();
        changed
    }

    fn sync_size(&mut self, inner: Rect) {
        self.term.set_area(inner);
    }

    fn focus_changed(&mut self, _focused: bool, _now_ms: i64) {}

    fn redraw(&mut self) {}

    fn clear(&mut self) {
        self.term.clear();
        self.term.set_follow(true);
        self.refresh_prompt();
    }

    fn touched(&self) -> &[String] {
        &self.proc.touched
    }

    fn sends_prefix(&self) -> bool {
        false
    }

    fn close_needs_confirm(&self) -> bool {
        true
    }
}

#[derive(Debug)]
pub struct Pane {
    pub id: PaneId,
    pub label: String,
    pub agent: Option<Agent>,
    pub account: Option<AccountId>,
    pub body: Box<dyn PaneBody>,
}

#[derive(Debug)]
pub struct Daemon {
    pub workspace: String,
    pub tabs: Vec<Tab>,
    pub active: usize,
    pub panes: Vec<Pane>,
    pub next_id: u64,

    pub attached_by: Option<String>,
}

impl Daemon {
    pub fn new(workspace: &str) -> Self {
        Self {
            workspace: workspace.to_owned(),
            tabs: vec![],
            active: 0,
            panes: vec![],
            next_id: 1,
            attached_by: None,
        }
    }

    pub fn pane(&self, id: PaneId) -> Option<&Pane> {
        self.panes.iter().find(|p| p.id == id)
    }

    pub fn pane_mut(&mut self, id: PaneId) -> Option<&mut Pane> {
        self.panes.iter_mut().find(|p| p.id == id)
    }

    pub fn active_tab(&self) -> Option<&Tab> {
        self.tabs.get(self.active)
    }

    pub fn active_tab_mut(&mut self) -> Option<&mut Tab> {
        self.tabs.get_mut(self.active)
    }

    pub fn focused_pane(&self) -> Option<PaneId> {
        self.active_tab().map(|t| t.focused)
    }

    fn alloc(&mut self) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        id
    }

    pub fn new_pane(
        &mut self,
        request: &PaneRequest,
        now_ms: i64,
        make: impl FnOnce(PaneSpawn) -> Box<dyn PaneBody>,
    ) -> PaneId {
        let id = self.alloc();
        let agent = request.agent();
        let body = make(PaneSpawn {
            id,
            workspace: self.workspace.clone(),
            now_ms,
        });
        self.panes.push(Pane {
            id,
            label: agent.map_or_else(|| "Shell".to_owned(), |a| a.label().to_owned()),
            agent,
            account: request.account().cloned(),
            body,
        });
        id
    }

    pub fn new_tab(
        &mut self,
        request: &PaneRequest,
        now_ms: i64,
        make: impl FnOnce(PaneSpawn) -> Box<dyn PaneBody>,
    ) -> usize {
        let pid = self.new_pane(request, now_ms, make);
        self.tabs.push(Tab {
            custom_label: None,
            root: PaneNode::Leaf(pid),
            focused: pid,
            zoomed: None,
        });
        self.active = self.tabs.len() - 1;
        self.active
    }

    pub fn split(
        &mut self,
        dir: SplitDir,
        new_first: bool,
        request: &PaneRequest,
        now_ms: i64,
        make: impl FnOnce(PaneSpawn) -> Box<dyn PaneBody>,
    ) -> Option<PaneId> {
        let target = self.focused_pane()?;
        let pid = self.new_pane(request, now_ms, make);
        let tab = self.active_tab_mut()?;
        if tab.root.split_leaf(target, pid, dir, new_first) {
            tab.focused = pid;
            tab.zoomed = None;
            Some(pid)
        } else {
            self.panes.retain(|p| p.id != pid);
            None
        }
    }

    pub fn close_pane(&mut self, id: PaneId) -> bool {
        let Some(ti) = self.tabs.iter().position(|t| t.leaves().contains(&id)) else {
            return false;
        };
        self.panes.retain(|p| p.id != id);
        let tab = &mut self.tabs[ti];
        if tab.leaves().len() <= 1 {
            self.tabs.remove(ti);
            if self.active >= self.tabs.len() {
                self.active = self.tabs.len().saturating_sub(1);
            }
            return true;
        }
        tab.root.remove_leaf(id);
        if tab.focused == id {
            tab.focused = tab.leaves()[0];
        }
        if tab.zoomed == Some(id) {
            tab.zoomed = None;
        }
        false
    }

    pub fn close_tab(&mut self, i: usize) {
        if i >= self.tabs.len() {
            return;
        }
        let leaves = self.tabs[i].leaves();
        self.panes.retain(|p| !leaves.contains(&p.id));
        self.tabs.remove(i);
        if self.active >= self.tabs.len() {
            self.active = self.tabs.len().saturating_sub(1);
        }
    }

    pub fn tab_label(&self, tab: &Tab, account_suffix: &dyn Fn(&Pane) -> Option<String>) -> String {
        if let Some(c) = &tab.custom_label {
            return c.clone();
        }
        let leaves = tab.leaves();
        let mut agents: Vec<String> = vec![];
        let mut has_shell = false;
        for id in &leaves {
            if let Some(p) = self.pane(*id) {
                match p.agent {
                    Some(a) => {
                        let mut l = a.label().to_owned();
                        if let Some(s) = account_suffix(p) {
                            l = format!("{l} ({s})");
                        }
                        if !agents.contains(&l) {
                            agents.push(l);
                        }
                    }
                    None => has_shell = true,
                }
            }
        }
        let base = match (agents.len(), has_shell) {
            (0, _) => "Shell".to_owned(),
            (1, false) => agents[0].clone(),
            (_, false) => "Agents".to_owned(),
            (_, true) => "Mix".to_owned(),
        };
        if leaves.len() > 1 {
            format!("{base} ({})", leaves.len())
        } else {
            base
        }
    }

    pub fn tab_state(&self, tab: &Tab) -> AgentState {
        tab.leaves()
            .iter()
            .filter_map(|id| self.pane(*id).map(|p| p.body.state()))
            .max_by_key(|s| s.rank())
            .unwrap_or(AgentState::Unknown)
    }

    pub fn tick(&mut self, now_ms: i64) -> bool {
        let mut changed = false;
        for p in &mut self.panes {
            changed |= p.body.tick(now_ms);
        }
        changed
    }

    pub fn touched_files(&self) -> Vec<String> {
        let mut v: Vec<String> = self
            .panes
            .iter()
            .flat_map(|p| p.body.touched().iter().cloned())
            .collect();
        v.sort();
        v.dedup();
        v
    }

    pub fn snapshot(&self) -> crate::domain::instance::DaemonSnapshot {
        use crate::domain::instance::{DaemonSnapshot, PaneSnapshot, TabSnapshot};
        if self.tabs.is_empty() {
            return DaemonSnapshot::NoTabs;
        }
        DaemonSnapshot::Tabs(
            self.tabs
                .iter()
                .enumerate()
                .map(|(i, t)| TabSnapshot {
                    label: self.tab_label(t, &|_| None),
                    active: i == self.active,
                    panes: t
                        .leaves()
                        .iter()
                        .filter_map(|id| self.pane(*id))
                        .map(|p| PaneSnapshot {
                            label: p.label.clone(),
                            agent: p.agent,
                            state: p.body.state(),
                            focused: t.focused == p.id,
                        })
                        .collect(),
                })
                .collect(),
        )
    }
}
