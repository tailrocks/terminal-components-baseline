mod pane_support;

use jackin_console::panes::{Lease, LinkEvent, PaneBody, PaneCommand};
use pane_support::*;

const QUERIES: [(&str, &[u8]); 8] = [
    ("DA1", b"\x1b[c"),
    ("DSR", b"\x1b[5n"),
    ("CPR", b"\x1b[6n"),
    ("kitty keyboard", b"\x1b[?u"),
    ("OSC 10", b"\x1b]10;?\x07"),
    ("OSC 11", b"\x1b]11;?\x07"),
    ("DECRQM", b"\x1b[?2004$p"),
    ("DA2", b"\x1b[>c"),
];

fn replies(bench: &Bench) -> Vec<PaneCommand> {
    bench
        .commands(0)
        .into_iter()
        .filter(|c| matches!(c, PaneCommand::Reply(_)))
        .collect()
}

#[test]
fn a_viewing_pane_answers_no_query_of_the_program() {
    for (name, query) in QUERIES {
        let bench = Bench::new();
        let mut pane = live(&bench);
        bench.push(0, LinkEvent::Lease(Lease::ReadOnly));
        pane.tick(900);
        bench.output(0, query);
        pane.tick(1_000);
        assert_eq!(
            bench.commands(0),
            [],
            "a viewer must send nothing for {name}"
        );
    }
}

#[test]
fn an_owning_pane_still_answers_each_query_of_the_program() {
    for (name, query) in QUERIES {
        let bench = Bench::new();
        let mut pane = live(&bench);
        bench.output(0, query);
        pane.tick(1_000);
        assert_eq!(
            replies(&bench).len(),
            1,
            "an owner answers {name}: {:?}",
            bench.commands(0)
        );
    }
}

#[test]
fn a_pane_answers_again_once_it_owns_the_lease() {
    let bench = Bench::new();
    let mut pane = live(&bench);
    bench.push(0, LinkEvent::Lease(Lease::ReadOnly));
    pane.tick(900);
    bench.output(0, b"\x1b[6n");
    pane.tick(1_000);
    assert_eq!(bench.commands(0), []);

    bench.push(0, LinkEvent::Lease(Lease::Owner));
    pane.tick(1_100);
    bench.output(0, b"\x1b[6n");
    pane.tick(1_200);
    assert_eq!(replies(&bench).len(), 1, "{:?}", bench.commands(0));
}
