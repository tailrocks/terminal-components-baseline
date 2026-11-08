mod common;

use common::*;
use junie_tui::core::hit::HitRegistry;
use junie_tui::core::id::WidgetId;
use junie_tui::core::intent::Phase;
use junie_tui::core::pointer::{MouseTarget, PointerEvent};
use junie_tui::widgets::nav_list::{NavItem, NavList};
use junie_tui::widgets::scrollbar;
use ratatui::buffer::Buffer;
use ratatui::crossterm::event::KeyModifiers;
use ratatui::layout::{Position, Rect};

const ID: WidgetId = WidgetId::of("t.nav");
const AREA: Rect = Rect::new(0, 0, 30, 8);

fn nav() -> NavList {
    NavList::new(
        ID,
        (0..12)
            .map(|n| NavItem::new(&format!("item {n:02}")))
            .collect(),
    )
}

fn render(nav: &mut NavList) -> (Buffer, HitRegistry) {
    let mut hits = HitRegistry::default();
    let (buf, _) = draw(None, |buf, ctx| {
        nav.render(AREA, buf, ctx, ctx.theme.canvas);
        hits = ctx.hits.clone();
    });
    (buf, hits)
}

fn thumb_start(nav: &mut NavList, x: u16) -> u16 {
    let (buf, _) = render(nav);
    (0..AREA.height)
        .find(|&y| buf[(x, y)].symbol() == scrollbar::THUMB)
        .expect("a thumb")
}

fn mouse(nav: &mut NavList, phase: Phase, pos: Position) {
    let (_, hits) = render(nav);
    let id = scrollbar::id_for(ID);
    let ev = PointerEvent::new(&hits, phase, id, pos, 1, KeyModifiers::NONE);
    nav.route(&ev);
}

#[test]
fn a_press_off_centre_on_the_nav_thumb_drags_it_cell_for_cell() {
    let mut nav = nav();
    let x = AREA.right() - 1;
    let start = thumb_start(&mut nav, x);
    let (buf, _) = render(&mut nav);
    let len = (0..AREA.height)
        .filter(|&y| buf[(x, y)].symbol() == scrollbar::THUMB)
        .count();
    assert!(len >= 3, "the thumb is long enough to grab off-centre");
    let grab = start + 1;

    mouse(&mut nav, Phase::Press, Position::new(x, grab));
    assert_eq!(
        thumb_start(&mut nav, x),
        start,
        "a press on the thumb does not move it"
    );

    mouse(&mut nav, Phase::Drag, Position::new(x, grab + 1));
    assert_eq!(
        thumb_start(&mut nav, x),
        start + 1,
        "one cell of drag moves the thumb one cell"
    );
    mouse(&mut nav, Phase::DragEnd, Position::new(x, grab + 1));
}
