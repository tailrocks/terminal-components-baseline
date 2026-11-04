//! What a pane tells the program about its focus, and what it learns from the
//! program: a bell or a notification from a pane nobody is looking at.

mod pane_support;

use jackin_console::domain::instance::AgentState;
use jackin_console::panes::{EndKind, Lease, LinkEvent, LinkFault, PaneBody};
use pane_support::*;

const FOCUS_OUT: &[u8] = b"\x1b[O";
const FOCUS_IN: &[u8] = b"\x1b[I";

#[test]
fn focus_events_follow_pane_focus() {
    let bench = Bench::new();
    let mut pane = live(&bench);
    pane.focus_changed(true, 900);
    pane.focus_changed(false, 1_000);
    assert_eq!(
        bench.commands(0),
        [],
        "nothing is sent before the program asks for focus events"
    );

    bench.output(0, b"\x1b[?1004h");
    pane.tick(1_100);
    pane.focus_changed(true, 1_200);
    pane.focus_changed(true, 1_300);
    pane.focus_changed(false, 1_400);
    assert_eq!(
        bench.commands(0),
        [input(FOCUS_IN), input(FOCUS_OUT)],
        "one report for each change, none for a repeat"
    );

    bench.output(0, b"\x1b[?1004l");
    pane.tick(1_500);
    pane.focus_changed(true, 1_600);
    assert_eq!(bench.commands(0).len(), 2, "the program turned them off");
}

#[test]
fn a_viewing_pane_reports_no_focus_and_an_owning_one_does_again() {
    let bench = Bench::new();
    let mut pane = live(&bench);
    bench.output(0, b"\x1b[?1004h");
    bench.push(0, LinkEvent::Lease(Lease::ReadOnly));
    pane.tick(1_000);
    pane.focus_changed(true, 1_100);
    pane.focus_changed(false, 1_200);
    assert_eq!(
        bench.commands(0),
        [],
        "switching tabs on a viewer must not type into the program"
    );

    bench.push(0, LinkEvent::Lease(Lease::Owner));
    pane.tick(1_300);
    pane.focus_changed(true, 1_400);
    assert_eq!(bench.commands(0), [input(FOCUS_IN)]);
}

#[test]
fn a_pane_without_a_live_link_reports_nothing() {
    let bench = Bench::new();
    let mut pane = live(&bench);
    bench.output(0, b"\x1b[?1004h");
    bench.push(0, LinkEvent::Ended(EndKind::Detached));
    pane.tick(1_000);
    pane.focus_changed(true, 1_100);
    assert_eq!(bench.commands(0), []);
}

fn state_after(
    request: jackin_console::panes::PaneRequest,
    focused: bool,
    event: &[u8],
) -> AgentState {
    let bench = Bench::new();
    let mut pane = pane_for(request, &bench, Default::default());
    pane.sync_size(inner(80, 24));
    bench.push(0, LinkEvent::Attached);
    pane.tick(0);
    pane.tick(800);
    pane.focus_changed(focused, 900);
    bench.output(0, event);
    pane.tick(1_000);
    pane.state()
}

#[test]
fn bell_in_background_marks_attention() {
    assert_eq!(state_after(codex(), false, b"\x07"), AgentState::Blocked);
    assert_eq!(
        state_after(codex(), false, b"\x1b]9;done\x07"),
        AgentState::Blocked,
        "a notification does too"
    );
    assert_eq!(
        state_after(shell(), false, b"\x07"),
        AgentState::Blocked,
        "it is a pane state, not a Codex one"
    );
}

#[test]
fn focused_bell_sets_nothing() {
    assert_eq!(state_after(codex(), true, b"\x07"), AgentState::Unknown);
    assert_eq!(
        state_after(codex(), true, b"\x1b]9;done\x07"),
        AgentState::Unknown
    );
    assert_eq!(
        state_after(codex(), false, b"plain output"),
        AgentState::Unknown
    );
}

#[test]
fn focusing_the_pane_clears_the_mark() {
    let bench = Bench::new();
    let mut pane = live(&bench);
    pane.focus_changed(false, 900);
    bench.output(0, b"\x07");
    pane.tick(1_000);
    assert_eq!(pane.state(), AgentState::Blocked);
    pane.focus_changed(true, 1_100);
    assert_eq!(pane.state(), AgentState::Unknown);
    pane.focus_changed(false, 1_200);
    assert_eq!(
        pane.state(),
        AgentState::Unknown,
        "leaving does not bring it back"
    );
}

#[test]
fn exit_codes_set_done_or_blocked() {
    let after = |event: LinkEvent| {
        let bench = Bench::new();
        let mut pane = live(&bench);
        bench.push(0, event);
        pane.tick(1_000);
        pane.state()
    };
    assert_eq!(after(LinkEvent::Exited(Some(0))), AgentState::Done);
    assert_eq!(after(LinkEvent::Exited(Some(3))), AgentState::Blocked);
    assert_eq!(after(LinkEvent::Exited(None)), AgentState::Unknown);
    assert_eq!(
        after(LinkEvent::Ended(EndKind::Refused(LinkFault::new("no")))),
        AgentState::Blocked,
        "a failed attach waits for the operator"
    );
}

#[test]
fn a_bell_in_the_replay_marks_nothing() {
    for event in [&b"old\x07"[..], b"old\x1b]9;done\x07"] {
        let bench = Bench::new();
        let mut pane = pane(&bench);
        pane.sync_size(inner(80, 24));
        bench.push(0, LinkEvent::Attached);
        pane.tick(0);
        pane.focus_changed(false, 50);
        bench.output(0, event);
        pane.tick(100);
        assert_eq!(
            pane.state(),
            AgentState::Unknown,
            "a bell from before the attach is not a new one"
        );
        pane.tick(800);
        assert_eq!(pane.state(), AgentState::Unknown);

        bench.output(0, event);
        pane.tick(900);
        assert_eq!(
            pane.state(),
            AgentState::Blocked,
            "the same bell after the window is"
        );
    }
}
