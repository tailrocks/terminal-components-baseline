//! The pointer over a terminal pane in the Capsule: its scrollbar and the
//! Shift-click that extends a selection.

mod capsule_support;
mod pane_support;
mod product_support;

use capsule_support::*;
use junie_tui::core::event::MouseKind;
use ratatui::crossterm::event::KeyModifiers;

fn history() -> Vec<u8> {
    (1..=100)
        .flat_map(|i| format!("line {i:03}\r\n").into_bytes())
        .collect()
}

#[test]
fn a_press_off_centre_on_the_pane_thumb_drags_it_cell_for_cell() {
    let mut rig = Rig::new();
    rig.attach();
    rig.bench.output(0, &history());
    rig.advance(80);
    let (x, start, len) = rig.thumb();
    assert!(len >= 4, "the thumb is long enough to grab off-centre");
    let grab = start + 1;

    rig.mouse(MouseKind::Down, (x, grab));
    assert_eq!(rig.thumb().1, start, "pressing the thumb does not move it");

    rig.mouse(MouseKind::Drag, (x, grab - 1));
    assert_eq!(
        rig.thumb().1,
        start - 1,
        "one cell of drag moves the thumb one cell"
    );

    rig.mouse(MouseKind::Up, (x, grab - 1));
    assert_eq!(rig.thumb().1, start - 1, "letting go leaves it there");
    assert!(
        !rig.daemon().panes[0].body.viewport().has_selection(),
        "the track is not text to select"
    );
}

#[test]
fn a_shift_click_extends_the_selection_from_the_last_click() {
    let mut rig = Rig::new();
    rig.attach();
    rig.bench.output(0, b"line one\r\nline two");
    rig.advance(80);
    let at = rig.locate("line one");
    let from = (at.0 + 2, at.1);
    let to = (at.0 + 6, at.1);

    rig.mouse(MouseKind::Down, from);
    rig.mouse(MouseKind::Up, from);
    rig.mouse_with(MouseKind::Down, to, KeyModifiers::SHIFT);
    rig.mouse_with(MouseKind::Up, to, KeyModifiers::SHIFT);

    assert_eq!(
        rig.daemon().panes[0].body.viewport().selected_text(),
        Some("ne o".to_owned())
    );
}
