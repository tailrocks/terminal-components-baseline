mod common;

use common::*;
use junie_tui::core::focus::{Focus, FocusRing};
use junie_tui::core::hit::HitRegistry;
use junie_tui::core::id::WidgetId;
use junie_tui::theme::ButtonKind;
use junie_tui::theme::{ColorLevel, Theme};
use junie_tui::ui::ctx::{Interaction, RenderCtx};
use junie_tui::ui::layout::Split;
use junie_tui::widgets::list_detail::{Action, Event, Frame, Line, ListDetail, ListRow};
use junie_tui::widgets::scrollbar;
use ratatui::buffer::Buffer;
use ratatui::crossterm::event::KeyCode;
use ratatui::layout::{Position, Rect};

const MASTER: WidgetId = WidgetId::of("t.ld.master");
const DETAIL: WidgetId = WidgetId::of("t.ld.detail");
const AREA: Rect = Rect::new(0, 0, 120, 20);
const ITEMS: usize = 16;
const ACTIONS: [&str; 6] = ["alpha", "bravo", "charlie", "delta", "echo", "foxtrot"];

fn rows() -> Vec<ListRow> {
    vec![ListRow::new("first"), ListRow::new("second")]
}

fn lines() -> Vec<Line> {
    let mut v: Vec<Line> = (0..30)
        .map(|n| {
            Line::Prop(
                format!("key{n:02}"),
                format!("value {n:02}"),
                junie_tui::theme::Tone::Secondary,
            )
        })
        .collect();
    for n in 0..ITEMS {
        v.push(Line::Item {
            mark: None,
            cols: [format!("item{n:02}"), "agent".into(), "meta".into()],
            tone: junie_tui::theme::Tone::Secondary,
        });
    }
    v
}

struct Rig {
    ld: ListDetail,
    focus: Focus,
    rows: Vec<ListRow>,
    hits: HitRegistry,
    area: Rect,
    actions: usize,
}

impl Rig {
    fn new() -> Self {
        Self::sized(AREA, 0)
    }

    fn sized(area: Rect, actions: usize) -> Self {
        let mut focus = Focus::default();
        focus.focus(DETAIL);
        let mut rig = Self {
            ld: ListDetail::new(MASTER, DETAIL, Split::new(42, 36, 44)),
            focus,
            rows: rows(),
            hits: HitRegistry::default(),
            area,
            actions,
        };
        rig.render();
        rig
    }

    fn render(&mut self) -> Buffer {
        let theme = Theme::for_level(ColorLevel::TrueColor);
        let mut buf = Buffer::empty(self.area);
        self.hits = HitRegistry::default();
        let mut ring = FocusRing::default();
        let interaction = Interaction {
            focus: self.focus.current(),
            ..Default::default()
        };
        let mut ctx = RenderCtx::new(&theme, interaction, &mut self.hits, &mut ring);
        let frame = Frame {
            title: "List",
            meta: String::new(),
            head: vec![],
            rows: &self.rows,
            under: None,
            detail_title: "Detail".into(),
            detail_meta: String::new(),
            lines: lines(),
            actions: ACTIONS[..self.actions]
                .iter()
                .map(|name| {
                    Action::new(
                        name,
                        &format!("{name} the whole selection"),
                        ButtonKind::Secondary,
                    )
                })
                .collect(),
        };
        self.ld.render(self.area, &mut buf, &mut ctx, &frame);
        buf
    }

    fn key(&mut self, code: KeyCode) -> Option<Event> {
        let len = self.rows.len();
        let (_, event) = self.ld.on_key(&key(code), &mut self.focus, len);
        event
    }

    fn click(&mut self, pos: Position) -> Option<Event> {
        let id = self
            .hits
            .hit(pos)
            .expect("a clickable region under the pointer");
        let len = self.rows.len();
        self.ld.click(id, pos, &mut self.focus, len).1
    }
}

fn find(buf: &Buffer, needle: &str) -> Option<u16> {
    (0..buf.area.height).find(|&y| row(buf, y).contains(needle))
}

#[test]
fn moving_the_item_cursor_scrolls_the_detail_to_keep_it_on_screen() {
    let mut rig = Rig::new();
    assert!(
        find(&rig.render(), "item15").is_none(),
        "the items start below the viewport"
    );

    for _ in 0..ITEMS - 1 {
        rig.key(KeyCode::Down);
    }
    let buf = rig.render();

    let y = find(&buf, "item15").expect("the cursor item is on screen");
    assert!(
        row(&buf, y).contains('▎'),
        "the focus gutter is on the cursor row"
    );
    assert_eq!(rig.key(KeyCode::Enter), Some(Event::Item(ITEMS - 1)));

    for _ in 0..ITEMS - 1 {
        rig.key(KeyCode::Up);
    }
    let buf = rig.render();
    let y = find(&buf, "item00").expect("moving back up keeps the cursor item on screen");
    assert!(row(&buf, y).contains('▎'));
}

#[test]
fn clicking_an_item_after_the_detail_scrolled_picks_that_item() {
    let mut rig = Rig::new();
    for _ in 0..ITEMS - 1 {
        rig.key(KeyCode::Down);
    }
    let buf = rig.render();
    let first_on_screen = (0..ITEMS)
        .find(|n| find(&buf, &format!("item{n:02}")).is_some())
        .expect("some item is visible");
    assert!(
        first_on_screen > 0,
        "the detail scrolled past the first item"
    );

    let target = first_on_screen + 1;
    let y = find(&buf, &format!("item{target:02}")).expect("target visible");
    let x = row(&buf, y)
        .find(&format!("item{target:02}"))
        .expect("column") as u16;

    assert_eq!(rig.click(Position::new(x, y)), Some(Event::Item(target)));
    assert_eq!(rig.ld.item_cursor, target);

    let buf = rig.render();
    let y = find(&buf, &format!("item{target:02}")).expect("still visible");
    assert!(
        row(&buf, y).contains('▎'),
        "the clicked row now holds the gutter"
    );
}

#[test]
fn clicking_an_item_after_wheel_scrolling_picks_the_global_item() {
    let mut rig = Rig::new();
    rig.ld.wheel(DETAIL, 40);
    let buf = rig.render();
    assert!(
        find(&buf, "item00").is_none(),
        "the first item scrolled out"
    );
    let y = find(&buf, "item10").expect("a later item is visible");
    let x = row(&buf, y).find("item10").expect("column") as u16;

    assert_eq!(rig.click(Position::new(x, y)), Some(Event::Item(10)));
    assert_eq!(rig.ld.item_cursor, 10);
}

#[test]
fn focusing_the_detail_shows_the_cursor_item_before_enter_acts_on_it() {
    let mut rig = Rig::new();
    let buf = rig.render();
    assert!(
        find(&buf, "item00").is_some(),
        "the cursor item is on screen once the detail has focus"
    );
    assert_eq!(rig.key(KeyCode::Enter), Some(Event::Item(0)));
}

#[test]
fn enter_after_the_detail_scrolled_away_reveals_the_cursor_item_first() {
    let mut rig = Rig::new();
    rig.ld.wheel(DETAIL, -40);
    assert!(find(&rig.render(), "item00").is_none(), "scrolled away");

    assert_eq!(
        rig.key(KeyCode::Enter),
        None,
        "nothing acts on a hidden item"
    );
    let buf = rig.render();
    let y = find(&buf, "item00").expect("Enter brought the cursor item into view");
    assert!(row(&buf, y).contains('▎'));
    assert_eq!(rig.key(KeyCode::Enter), Some(Event::Item(0)));
}

fn detail_track(rig: &Rig) -> Vec<Position> {
    let bar = scrollbar::id_for(DETAIL);
    let mut cells = vec![];
    for y in 0..rig.area.height {
        for x in 0..rig.area.width {
            let pos = Position::new(x, y);
            if rig.hits.hit(pos) == Some(bar) {
                cells.push(pos);
            }
        }
    }
    cells
}

#[test]
fn dragging_the_detail_scrollbar_scrolls_the_detail() {
    let mut rig = Rig::new();
    assert!(find(&rig.render(), "item15").is_none());
    let track = detail_track(&rig);
    assert!(
        !track.is_empty(),
        "the overflowing detail draws a scrollbar"
    );

    let first = track[0];
    let last = *track.last().expect("a track");
    rig.click(first);
    rig.ld.drag(scrollbar::id_for(DETAIL), last);

    assert!(
        find(&rig.render(), "item15").is_some(),
        "dragging the thumb to the bottom of the track reaches the end of the detail"
    );
}

fn thumb_start(rig: &mut Rig, track: &[Position]) -> (usize, usize) {
    let buf = rig.render();
    let cells: Vec<usize> = track
        .iter()
        .enumerate()
        .filter(|(_, p)| buf[(p.x, p.y)].symbol() == scrollbar::THUMB)
        .map(|(i, _)| i)
        .collect();
    (cells[0], cells.len())
}

#[test]
fn a_press_off_centre_on_the_detail_thumb_drags_it_cell_for_cell() {
    let mut rig = Rig::new();
    let track = detail_track(&rig);
    let (start, len) = thumb_start(&mut rig, &track);
    assert!(len >= 4, "the thumb is long enough to grab off-centre");
    let grab = start + 1;

    rig.click(track[grab]);
    let (pressed, _) = thumb_start(&mut rig, &track);
    assert_eq!(pressed, start, "pressing the thumb does not move it");

    rig.ld.drag(scrollbar::id_for(DETAIL), track[grab + 1]);
    let (dragged, _) = thumb_start(&mut rig, &track);
    assert_eq!(
        dragged,
        start + 1,
        "one cell of drag moves the thumb one cell"
    );
}

#[test]
fn the_bottom_of_the_detail_track_reaches_the_end_when_actions_shorten_it() {
    let mut rig = Rig::sized(AREA, 6);
    assert!(find(&rig.render(), "item15").is_none());
    let track = detail_track(&rig);
    let near_end = track[track.len() - 3];

    rig.click(near_end);

    let buf = rig.render();
    assert_eq!(
        buf[(near_end.x, near_end.y)].symbol(),
        scrollbar::THUMB,
        "the thumb lands under the clicked track cell"
    );
}

#[test]
fn a_narrow_detail_keeps_its_item_columns_inside_the_frame() {
    let area = Rect::new(0, 0, 26, 20);
    let mut rig = Rig::sized(area, 0);
    let buf = rig.render();
    let right = area.width - 1;
    let edge: std::collections::BTreeSet<String> = (1..area.height - 1)
        .map(|y| buf[(right, y)].symbol().to_owned())
        .collect();
    assert_eq!(
        edge.len(),
        1,
        "only the frame's right border is in the last column, got {edge:?}"
    );
    let bar = scrollbar::id_for(DETAIL);
    for y in 1..area.height - 1 {
        let pos = Position::new(right - 1, y);
        if rig.hits.hit(pos) == Some(bar) {
            let s = buf[(right - 1, y)].symbol();
            assert!(
                s == scrollbar::TRACK || s == scrollbar::THUMB,
                "row {y} wrote {s:?} over the scrollbar"
            );
        }
    }
}
