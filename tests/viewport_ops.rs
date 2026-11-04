mod common;

use common::*;
use junie_tui::core::hit::Axis;
use junie_tui::core::id::WidgetId;
use junie_tui::core::scroll::Scrollable;
use junie_tui::widgets::viewport::{CellPos, Line, Span, TextViewport};
use junie_tui::widgets::viewport_ops::ViewportOps;
use ratatui::buffer::Buffer;
use ratatui::layout::{Position, Rect};

const ID: WidgetId = WidgetId::of("t.viewport");
const AREA: Rect = Rect::new(0, 0, 40, 8);

fn lines() -> Vec<Line> {
    (0..60)
        .map(|n| vec![Span::plain(format!("row {n:02} the quick brown fox"))])
        .collect()
}

fn viewport() -> TextViewport {
    let mut v = TextViewport::with_lines(ID, lines());
    v.follow = true;
    v.set_area(AREA);
    v
}

fn paint(v: &dyn ViewportOps, caret_allowed: bool) -> (Buffer, Option<Position>) {
    let mut cursor = None;
    let (buf, _) = draw(None, |buf, ctx| {
        cursor = v.draw(AREA, buf, ctx, ctx.theme.canvas, caret_allowed);
    });
    (buf, cursor)
}

fn rows(buf: &Buffer) -> Vec<String> {
    (0..AREA.height).map(|y| row(buf, y)).collect()
}

#[test]
fn the_ops_scroll_exactly_like_the_text_viewport() {
    let mut direct = viewport();
    let mut via = viewport();
    let ops: &mut dyn ViewportOps = &mut via;

    assert!(ops.follow());
    assert_eq!(ops.id(), ID);
    for _ in 0..3 {
        let a = Scrollable::wheel(&mut direct, Axis::Vertical, -1).outcome();
        assert_eq!(ops.wheel(Axis::Vertical, -1), a);
    }
    assert!(!ops.follow() && !direct.follow);
    assert_eq!(ops.scroll().offset, direct.scroll.offset);
    assert_eq!(ops.scroll().visible_range(), direct.scroll.visible_range());
    assert_eq!(ops.scroll().content_len, 60);

    ops.page_up();
    direct.scroll.page_up();
    assert_eq!(ops.scroll().offset, direct.scroll.offset);
    assert!(!ops.follow(), "page up never returns to the live tail");

    ops.jump_start();
    direct.scroll.jump_start();
    assert_eq!(ops.scroll().offset, 0);
    assert_eq!(direct.scroll.offset, 0);

    for _ in 0..30 {
        ops.page_down();
        direct.scroll.page_down();
        direct.follow = direct.is_at_tail();
        assert_eq!(ops.scroll().offset, direct.scroll.offset);
        assert_eq!(ops.follow(), direct.follow);
    }
    assert!(
        ops.follow(),
        "paging down to the end follows the tail again"
    );

    ops.wheel(Axis::Vertical, -1);
    assert!(!ops.follow());
    ops.set_follow(true);
    assert!(ops.follow());
    assert_eq!(ops.scroll().offset, ops.scroll().max_offset());
}

#[test]
fn the_ops_select_and_copy_exactly_like_the_text_viewport() {
    let mut direct = viewport();
    let mut via = viewport();
    let ops: &mut dyn ViewportOps = &mut via;

    assert!(!ops.has_selection());
    assert_eq!(ops.selected_text(), None);
    let from = Position::new(4, 1);
    let to = Position::new(13, 1);
    direct.on_click(from);
    ops.on_click(from);
    assert_eq!(ops.on_drag(to), direct.on_drag(to));
    assert!(ops.has_selection() && direct.has_selection());
    assert_eq!(ops.selected_text(), direct.selected_text());
    assert_eq!(ops.selected_text().as_deref(), Some("53 the qu"));
    assert_eq!(ops.context_point(), direct.context_point());

    ops.clear_selection();
    assert!(!ops.has_selection());
    assert_eq!(ops.selected_text(), None);

    let at = Position::new(9, 2);
    assert_eq!(ops.select_word_at(at), direct.select_word_at(at));
    assert_eq!(ops.selected_text(), direct.selected_text());
    assert_eq!(ops.selected_text().as_deref(), Some("the"));
}

#[test]
fn the_ops_report_the_caret_and_the_bars_like_the_text_viewport() {
    let mut direct = viewport();
    direct.caret = Some(CellPos { line: 59, col: 4 });
    direct.caret_visible = true;
    let ops: &dyn ViewportOps = &direct;

    assert_eq!(ops.caret(), Some(CellPos { line: 59, col: 4 }));
    assert!(ops.caret_visible());
    assert_eq!(
        ops.screen_pos(CellPos { line: 59, col: 4 }),
        direct.screen_pos(CellPos { line: 59, col: 4 })
    );
    assert_eq!(
        ops.screen_pos(CellPos { line: 59, col: 4 }),
        Some(Position::new(4, 7))
    );
    assert_eq!(ops.hbar(), direct.hbar());
    assert!(
        ops.keyboard_copy(),
        "the text viewport copies with the y key"
    );

    let mut resized = viewport();
    let ops: &mut dyn ViewportOps = &mut resized;
    let narrow = Rect::new(2, 1, 20, 4);
    ops.set_area(narrow);
    let mut expect = viewport();
    expect.set_area(narrow);
    assert_eq!(ops.hbar(), expect.hbar());
    assert_eq!(ops.scroll().visible_range(), expect.scroll.visible_range());
    assert_eq!(
        ops.hbar().height,
        1,
        "text wider than the area gets a bottom bar"
    );
    assert_eq!(
        ops.scroll().visible_range().len(),
        3,
        "the bar takes the fourth row"
    );
}

#[test]
fn draw_paints_the_viewport_and_leaves_it_untouched() {
    let mut direct = viewport();
    let (want, _) = draw(None, |buf, ctx| {
        let mut copy = direct.clone();
        copy.render(AREA, buf, ctx, ctx.theme.canvas);
    });
    let (got, cursor) = paint(&direct, true);
    assert_eq!(rows(&got), rows(&want));
    assert!(
        row(&got, 0).starts_with("row 52 the quick brown fox"),
        "{}",
        row(&got, 0)
    );
    assert!(
        row(&got, 7).starts_with("row 59 the quick brown fox"),
        "{}",
        row(&got, 7)
    );
    assert_eq!(cursor, None, "no caret, no hardware cursor");

    let before = (direct.scroll.offset, direct.follow, direct.revision());
    let again = paint(&direct, true);
    assert_eq!(rows(&again.0), rows(&got));
    assert_eq!(
        (direct.scroll.offset, direct.follow, direct.revision()),
        before
    );

    direct.set_follow(false);
    direct.scroll.jump_start();
    let (top, _) = paint(&direct, true);
    assert!(
        row(&top, 0).starts_with("row 00 the quick brown fox"),
        "{}",
        row(&top, 0)
    );
}

#[test]
fn draw_shows_the_hardware_cursor_only_when_the_caret_is_allowed_and_live() {
    let mut v = viewport();
    v.caret = Some(CellPos { line: 59, col: 4 });
    v.caret_visible = true;
    let (_, shown) = paint(&v, true);
    assert_eq!(shown, Some(Position::new(4, 7)));
    let (_, withheld) = paint(&v, false);
    assert_eq!(
        withheld, None,
        "a dialog, a drag or another focus withholds it"
    );

    v.caret_visible = false;
    assert_eq!(paint(&v, true).1, None, "the program hid the caret");

    v.caret_visible = true;
    v.set_follow(false);
    v.scroll.jump_start();
    assert_eq!(
        paint(&v, true).1,
        None,
        "a scrolled-back view has no live caret"
    );

    let mut short = TextViewport::with_lines(ID, lines().into_iter().take(5).collect());
    short.set_area(AREA);
    short.caret = Some(CellPos { line: 4, col: 2 });
    short.caret_visible = true;
    short.follow = true;
    assert_eq!(paint(&short, true).1, Some(Position::new(2, 4)));
    short.follow = false;
    assert_eq!(
        paint(&short, true).1,
        None,
        "off the live tail the caret is not where the program is typing"
    );
}
