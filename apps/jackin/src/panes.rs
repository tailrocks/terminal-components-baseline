use junie_tui::core::event::Key;
use junie_tui::theme::Tone;
use junie_tui::ui::layout::SplitDir;
use junie_tui::widgets::viewport_ops::ViewportOps;
use ratatui::layout::Rect;

use crate::domain::account::AccountId;
use crate::domain::agent::Agent;
use crate::domain::instance::{AgentState, InstanceId};

pub use junie_tui::ui::pane_tree::PaneId;

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
