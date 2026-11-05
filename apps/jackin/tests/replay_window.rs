//! The replay window: after an attach the pane shows a placeholder, not the
//! grid, until the replay has quieted, and answers no query while it is open.

mod pane_support;

use jackin_console::panes::{
    DetachCause, EndKind, Lease, LinkEvent, LinkStatus, PaneBody, PaneCommand, PaneFace,
    TerminalPane,
};
use pane_support::*;

fn attached(bench: &std::sync::Arc<Bench>) -> TerminalPane {
    let mut pane = pane(bench);
    pane.sync_size(inner(80, 24));
    bench.push(0, LinkEvent::Attached);
    bench.push(0, LinkEvent::Lease(Lease::Owner));
    pane.tick(0);
    pane
}

/// Runs the pane through `steps`: at each time the output (if any) arrives and
/// the pane ticks. Returns the first time at which the window was closed.
fn closes_at(steps: &[(i64, Option<&[u8]>)], until: i64) -> Option<i64> {
    let bench = Bench::new();
    let mut pane = attached(&bench);
    for now in 1..=until {
        if let Some((_, Some(bytes))) = steps.iter().find(|(at, _)| *at == now) {
            bench.output(0, bytes);
        }
        pane.tick(now);
        if matches!(pane.status(), LinkStatus::Attached(_)) {
            return Some(now);
        }
    }
    None
}

#[test]
fn the_window_closes_at_the_nudge_when_nothing_arrives() {
    assert_eq!(closes_at(&[], 900), Some(600));
}

#[test]
fn output_before_the_nudge_does_not_hold_the_window() {
    let early: [(i64, Option<&[u8]>); 2] = [(100, Some(b"a")), (300, Some(b"b"))];
    assert_eq!(closes_at(&early, 900), Some(600));
}

#[test]
fn output_near_the_nudge_holds_it_until_the_stream_is_quiet() {
    assert_eq!(closes_at(&[(550, Some(b"x"))], 900), Some(700));
}

#[test]
fn a_stream_that_never_stops_is_cut_at_the_cap() {
    let steps: Vec<(i64, Option<&[u8]>)> = (1..=9).map(|n| (n * 100, Some(&b"x"[..]))).collect();
    assert_eq!(closes_at(&steps, 900), Some(800));
}

#[test]
fn the_pane_shows_a_placeholder_while_the_window_is_open() {
    let bench = Bench::new();
    let mut pane = attached(&bench);
    let words = words();
    assert_eq!(pane.status(), &LinkStatus::Attaching);
    assert!(matches!(
        pane.face(&words),
        PaneFace::Empty { ref title, .. } if title == "Attaching to Codex…"
    ));
    pane.tick(599);
    assert_eq!(pane.status(), &LinkStatus::Attaching);
    pane.tick(600);
    assert!(matches!(pane.status(), LinkStatus::Attached(_)));
    assert_eq!(pane.face(&words), PaneFace::Grid);
}

#[test]
fn queries_inside_the_window_get_no_reply() {
    let bench = Bench::new();
    let mut pane = attached(&bench);
    bench.output(0, b"\x1b[c");
    pane.tick(100);
    assert_eq!(bench.commands(0), [], "the replayed query is not answered");

    pane.tick(800);
    bench.output(0, b"\x1b[3;5H\x1b[6n");
    pane.tick(900);
    assert_eq!(
        bench.commands(0),
        [PaneCommand::Reply(b"\x1b[3;5R".to_vec())],
        "a query after the window gets one reply"
    );
}

#[test]
fn a_cut_sequence_is_never_shown() {
    let bench = Bench::new();
    let mut pane = attached(&bench);
    let words = words();
    bench.output(0, b";224;84mtail");
    pane.tick(100);
    assert!(matches!(pane.face(&words), PaneFace::Empty { .. }));
    assert_eq!(row(&pane, 0), ";224;84mtail", "it is in the hidden grid");

    bench.output(0, b"\x1b[H\x1b[2Jready $");
    pane.tick(650);
    pane.tick(800);
    assert_eq!(pane.face(&words), PaneFace::Grid);
    assert_eq!(
        row(&pane, 0),
        "ready $",
        "the repaint wrote over the fragment"
    );
    for r in 0..24 {
        assert!(!row(&pane, r).contains("224;84m"), "row {r}");
    }
}

#[test]
fn a_reattach_opens_a_new_window() {
    let bench = Bench::new();
    let mut pane = attached(&bench);
    pane.tick(800);
    assert!(matches!(pane.status(), LinkStatus::Attached(_)));
    bench.push(0, LinkEvent::Ended(EndKind::Detached));
    pane.tick(900);
    assert_eq!(pane.status(), &LinkStatus::Detached(DetachCause::Operator));

    pane.key(&key("enter"), 1_000);
    bench.push(1, LinkEvent::Attached);
    bench.push(1, LinkEvent::Lease(Lease::Owner));
    pane.tick(1_000);
    assert_eq!(pane.status(), &LinkStatus::Attaching);
    bench.output(1, b"\x1b[c");
    pane.tick(1_100);
    assert_eq!(bench.commands(1), [], "the new window drops replies too");
    pane.tick(1_600);
    assert!(matches!(pane.status(), LinkStatus::Attached(_)));
}
