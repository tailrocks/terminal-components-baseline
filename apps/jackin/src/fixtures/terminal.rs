//! The scenario `capsule-terminal`: a Capsule whose tabs hold terminal panes in
//! the link states a pane can be in, each over a scripted link.

use std::sync::Arc;

use junie_tui::ui::layout::SplitDir;

use crate::domain::agent::Agent;
use crate::panes::{Daemon, PaneBody, PaneNode, PaneRequest, PaneSpawn, PaneTiming, TerminalPane};
use crate::sim::link::{Play, SimSource, Stream};
use crate::world::{World, refresh_snapshots};

const INSTANCE: &str = "jk-7f3a";

fn opened(
    request: &PaneRequest,
    play: Play,
    timing: PaneTiming,
) -> impl FnOnce(PaneSpawn) -> Box<dyn PaneBody> + '_ {
    move |spawn| {
        Box::new(TerminalPane::new(
            spawn.id,
            request.clone(),
            Arc::new(SimSource::new(play)),
            timing,
            spawn.now_ms,
        ))
    }
}

fn label(d: &mut Daemon, tab: usize, text: &str) {
    if let Some(t) = d.tabs.get_mut(tab) {
        t.custom_label = Some(text.into());
    }
}

/// The OpenShell world with one running instance whose seven tabs are:
/// `live` (Codex beside a pager), `pending` (a pane with no room), `attaching`,
/// `viewing` (read only), `detached`, `exited` and `failed`.
pub fn capsule_terminal(w: &mut World) {
    super::surfaces::fixed(w);
    let timing = w.backend.pane_timing();
    let now = w.now_ms();
    let codex = PaneRequest::new(
        INSTANCE,
        Some(Agent::Codex),
        Some("acct-codex-primary".into()),
    );
    let shell = PaneRequest::new(INSTANCE, None, None);
    let mut d = Daemon::new("payments-platform");

    d.new_tab(
        &codex,
        now,
        opened(&codex, Play::Stays(Stream::Codex), timing),
    );
    d.split(
        SplitDir::Horizontal,
        false,
        &shell,
        now,
        opened(&shell, Play::Stays(Stream::Pager), timing),
    );
    if let Some(t) = d.active_tab_mut() {
        t.focused = 1;
    }
    label(&mut d, 0, "live");

    d.new_tab(
        &shell,
        now,
        opened(&shell, Play::Stays(Stream::Shell), timing),
    );
    d.split(
        SplitDir::Vertical,
        false,
        &codex,
        now,
        opened(&codex, Play::Stays(Stream::Codex), timing),
    );
    if let Some(PaneNode::Split { split, .. }) = d.active_tab_mut().map(|t| &mut t.root) {
        split.percent = 92;
    }
    label(&mut d, 1, "pending");

    d.new_tab(&codex, now, opened(&codex, Play::Hangs, timing));
    label(&mut d, 2, "attaching");

    d.new_tab(
        &codex,
        now,
        opened(&codex, Play::Watches(Stream::Codex), timing),
    );
    d.split(
        SplitDir::Horizontal,
        false,
        &shell,
        now,
        opened(&shell, Play::Stays(Stream::Shell), timing),
    );
    if let Some(t) = d.active_tab_mut() {
        t.focused = 6;
    }
    label(&mut d, 3, "viewing");

    d.new_tab(
        &codex,
        now,
        opened(&codex, Play::Detaches(Stream::Codex), timing),
    );
    label(&mut d, 4, "detached");

    d.new_tab(
        &codex,
        now,
        opened(&codex, Play::Exits(Stream::Codex), timing),
    );
    label(&mut d, 5, "exited");

    d.new_tab(
        &codex,
        now,
        opened(
            &codex,
            Play::Refuses("the gateway is unreachable · run jackin doctor".into()),
            timing,
        ),
    );
    label(&mut d, 6, "failed");

    d.active = 0;
    w.daemons.insert(INSTANCE.into(), d);
    refresh_snapshots(w);
    w.sync_arbiter();
}
