mod common;

use junie_tui::core::event::{Key, Outcome};
use junie_tui::core::focus::{Focus, FocusRing};
use junie_tui::core::hit::HitRegistry;
use junie_tui::core::id::WidgetId;
use junie_tui::core::refusal::{Control, Reason, ReasonId, clear_refusal, take_refusal};
use junie_tui::shell::{Cx, Request, StatusLine};
use junie_tui::theme::{ColorLevel, Theme, Tone};
use junie_tui::ui::ctx::{Interaction, RenderCtx};
use junie_tui::widgets::button::Button;
use junie_tui::widgets::choice::{Checkbox, RadioGroup};
use junie_tui::widgets::list::{ListBox, ListItem, SelectMode};
use junie_tui::widgets::select::Select;
use ratatui::buffer::Buffer;
use ratatui::crossterm::event::{KeyCode, KeyModifiers};
use ratatui::layout::Rect;

const WHY: &str = "Always off · the host decides";

fn key(code: KeyCode) -> Key {
    Key {
        code,
        mods: KeyModifiers::NONE,
    }
}

fn reason() -> Reason {
    Reason::new(WHY).short("managed")
}

fn taken() -> Option<String> {
    take_refusal().map(ReasonId::text)
}

fn draw(
    focus: Option<WidgetId>,
    paint: impl FnOnce(&mut Buffer, &mut RenderCtx),
) -> (Buffer, FocusRing) {
    let theme = Theme::for_level(ColorLevel::TrueColor);
    let mut buf = Buffer::empty(Rect::new(0, 0, 60, 8));
    let mut hits = HitRegistry::default();
    let mut ring = FocusRing::default();
    let interaction = Interaction {
        focus,
        ..Default::default()
    };
    {
        let mut ctx = RenderCtx::new(&theme, interaction, &mut hits, &mut ring);
        paint(&mut buf, &mut ctx);
    }
    (buf, ring)
}

fn row(buf: &Buffer, y: u16) -> String {
    (0..buf.area.width)
        .map(|x| buf[(x, y)].symbol().to_owned())
        .collect::<String>()
}

fn fg_at(buf: &Buffer, x: u16, y: u16) -> ratatui::style::Color {
    buf[(x, y)].fg
}

#[test]
fn a_fixed_checkbox_refuses_every_edit() {
    clear_refusal();
    let mut fixed = Checkbox::new(WidgetId::of("t.fixed"), "Keep awake", false).fixed(reason());
    for k in [KeyCode::Char(' '), KeyCode::Enter] {
        assert_eq!(fixed.on_key(&key(k)), Outcome::Consumed);
        assert_eq!(taken().as_deref(), Some(WHY));
    }
    assert_eq!(fixed.on_click(), Outcome::Consumed);
    assert_eq!(taken().as_deref(), Some(WHY));
    assert!(!fixed.checked);
    assert_eq!(fixed.on_key(&key(KeyCode::Tab)), Outcome::Ignored);
    assert_eq!(taken(), None);

    let mut live = Checkbox::new(WidgetId::of("t.live"), "Keep awake", false);
    assert_eq!(live.on_key(&key(KeyCode::Char(' '))), Outcome::Changed);
    assert!(live.checked);
    assert_eq!(live.on_click(), Outcome::Changed);
    assert!(!live.checked);
    assert_eq!(taken(), None);
}

#[test]
fn a_fixed_checkbox_draws_disabled_with_its_mark_and_stays_focusable() {
    let id = WidgetId::of("t.fixed");
    let render = |mut cb: Checkbox, focus: bool| {
        let (buf, ring) = draw(focus.then_some(id), |buf, ctx| {
            cb.render(Rect::new(0, 0, 40, 1), buf, ctx, ctx.theme.canvas)
        });
        (row(&buf, 0), fg_at(&buf, 6, 0), ring.contains(id))
    };
    let base = Checkbox::new(id, "Keep awake", true);
    let (live_row, live_ink, live_ring) = render(base.clone(), false);
    let (off_row, off_ink, off_ring) = render(
        Checkbox {
            disabled: true,
            ..base.clone()
        },
        false,
    );
    let (fixed_row, fixed_ink, fixed_ring) = render(base.clone().fixed(reason()), false);
    let (focused_row, _, _) = render(base.clone().fixed(reason()), true);
    let (off_focused, _, off_focused_ring) = render(
        Checkbox {
            disabled: true,
            ..base
        },
        true,
    );

    assert!(live_row.contains("[✓] Keep awake") && live_ring);
    assert!(fixed_row.contains("[✓] Keep awake"));
    assert_eq!(fixed_ink, off_ink);
    assert_ne!(fixed_ink, live_ink);
    assert!(fixed_ring && !off_ring && !off_focused_ring);
    assert!(focused_row.starts_with('▎'), "{focused_row:?}");
    assert!(!off_focused.starts_with('▎'), "{off_row:?} {off_focused:?}");
}

#[test]
fn a_fixed_select_refuses_open_arrows_typing_and_clicks() {
    clear_refusal();
    let id = WidgetId::of("t.select");
    let mut fixed = Select::new(id, "Dirty exit", &["ask", "stop", "keep"], 0).fixed(reason());
    for k in [
        KeyCode::Enter,
        KeyCode::Char(' '),
        KeyCode::Down,
        KeyCode::Up,
        KeyCode::Right,
        KeyCode::Char('s'),
    ] {
        let (out, action) = fixed.on_key(&key(k));
        assert_eq!((out, action), (Outcome::Consumed, None), "{k:?}");
        assert_eq!(taken().as_deref(), Some(WHY), "{k:?}");
    }
    assert!(!fixed.open);
    assert_eq!(fixed.value(), "ask");
    assert_eq!(fixed.on_click(id), (Outcome::Consumed, None));
    assert_eq!(taken().as_deref(), Some(WHY));
    assert_eq!(
        fixed.on_press(id, ratatui::layout::Position::new(1, 1)),
        Outcome::Consumed
    );
    assert_eq!(taken().as_deref(), Some(WHY));
    assert!(!fixed.open && fixed.value() == "ask");
    for k in [KeyCode::Tab, KeyCode::Esc] {
        assert_eq!(fixed.on_key(&key(k)).0, Outcome::Ignored, "{k:?}");
        assert_eq!(taken(), None, "{k:?}");
    }

    let mut live = Select::new(id, "Dirty exit", &["ask", "stop", "keep"], 0);
    assert_eq!(live.on_key(&key(KeyCode::Enter)).0, Outcome::Changed);
    assert!(live.open);
    assert_eq!(taken(), None);
}

#[test]
fn a_fixed_select_keeps_its_value_and_shows_the_focus_gutter() {
    let id = WidgetId::of("t.select");
    let render = |mut sel: Select, focus: bool| {
        let (buf, ring) = draw(focus.then_some(id), |buf, ctx| {
            sel.render(Rect::new(0, 0, 40, 3), buf, ctx, ctx.theme.canvas)
        });
        (row(&buf, 1), ring.contains(id))
    };
    let base = Select::new(id, "Dirty exit", &["ask", "stop"], 0);
    let (fixed, in_ring) = render(base.clone().fixed(reason()), true);
    let (off, off_ring) = render(base.disabled(true), true);
    assert!(fixed.starts_with('▎') && fixed.contains("ask"), "{fixed:?}");
    assert!(in_ring && !off_ring);
    assert!(!off.starts_with('▎'), "{off:?}");
}

#[test]
fn a_fixed_radio_option_takes_the_cursor_but_is_never_chosen() {
    clear_refusal();
    let id = WidgetId::of("t.radio");
    let mut group =
        RadioGroup::new(id, "Source", &["Local", "Manager", "Typed"], 0).fixed_option(2, reason());
    assert_eq!(group.on_key(&key(KeyCode::Down)), Outcome::Changed);
    assert_eq!((group.cursor, group.selected), (1, 1));
    assert_eq!(group.on_key(&key(KeyCode::Down)), Outcome::Changed);
    assert_eq!((group.cursor, group.selected), (2, 1));
    assert_eq!(taken(), None);
    assert_eq!(group.on_key(&key(KeyCode::Char(' '))), Outcome::Consumed);
    assert_eq!(taken().as_deref(), Some(WHY));
    assert_eq!(group.on_key(&key(KeyCode::Enter)), Outcome::Consumed);
    assert_eq!(taken().as_deref(), Some(WHY));
    group.on_click(0);
    assert_eq!((group.cursor, group.selected), (0, 0));
    assert_eq!(taken(), None);
    group.on_click(2);
    assert_eq!((group.cursor, group.selected), (2, 0));
    assert_eq!(taken().as_deref(), Some(WHY));

    let (buf, ring) = draw(Some(id), |buf, ctx| {
        group.render(Rect::new(0, 0, 40, 4), buf, ctx, ctx.theme.canvas)
    });
    assert!(ring.contains(id));
    assert!(row(&buf, 3).starts_with('▎'), "{:?}", row(&buf, 3));
    assert!(row(&buf, 3).contains("( ) Typed") && row(&buf, 1).contains("(●) Local"));
    assert_ne!(fg_at(&buf, 6, 3), fg_at(&buf, 6, 2));
}

#[test]
fn a_fixed_button_refuses_and_does_not_activate() {
    clear_refusal();
    let mut fixed = Button::secondary(WidgetId::of("t.btn"), "Disable").fixed(reason());
    assert!(!fixed.can_activate());
    assert_eq!(
        fixed.on_key(&key(KeyCode::Enter)),
        (Outcome::Consumed, false)
    );
    assert_eq!(taken().as_deref(), Some(WHY));
    assert_eq!(
        fixed.on_key(&key(KeyCode::Char(' '))),
        (Outcome::Consumed, false)
    );
    assert_eq!(taken().as_deref(), Some(WHY));
    assert!(!fixed.on_click());
    assert_eq!(taken().as_deref(), Some(WHY));

    let mut live = Button::secondary(WidgetId::of("t.btn"), "Disable");
    assert_eq!(live.on_key(&key(KeyCode::Enter)), (Outcome::Changed, true));
    assert!(live.on_click());
    assert_eq!(taken(), None);

    let id = WidgetId::of("t.btn");
    let (buf, ring) = draw(Some(id), |buf, ctx| {
        let mut b = Button::secondary(id, "Disable").fixed(reason());
        b.render(Rect::new(0, 0, 12, 1), buf, ctx, ctx.theme.canvas);
    });
    assert!(ring.contains(id) && row(&buf, 0).starts_with("▎Disable"));
}

#[test]
fn a_fixed_list_row_takes_the_cursor_and_refuses_to_be_ticked() {
    clear_refusal();
    let items = vec![
        ListItem::new("Team policy"),
        ListItem::new("Managed policy").fixed(reason()),
        ListItem::new("Personal policy"),
    ];
    let mut list = ListBox::new(WidgetId::of("t.list"), items, SelectMode::Multi);
    assert_eq!(list.on_key(&key(KeyCode::Down)), Outcome::Changed);
    assert_eq!(list.core.cursor, 1);
    assert_eq!(list.on_key(&key(KeyCode::Char(' '))), Outcome::Consumed);
    assert_eq!(taken().as_deref(), Some(WHY));
    assert!(!list.is_checked(1));
    list.on_click(1);
    assert_eq!(taken().as_deref(), Some(WHY));
    assert!(!list.is_checked(1));
    list.on_key(&key(KeyCode::Char('a')));
    assert!(list.is_checked(0) && !list.is_checked(1) && list.is_checked(2));
    assert_eq!(taken(), None);
    list.on_key(&key(KeyCode::Up));
    assert_eq!(list.on_key(&key(KeyCode::Char(' '))), Outcome::Changed);
    assert!(!list.is_checked(0));
    assert_eq!(taken(), None);
}

#[test]
fn taking_a_refusal_clears_the_slot() {
    clear_refusal();
    assert!(Control::fixed(reason()).refuse());
    assert_eq!(taken().as_deref(), Some(WHY));
    assert_eq!(taken(), None);
    assert!(!Control::Live.refuse());
    assert_eq!(taken(), None);
}

#[test]
fn the_shell_clears_an_untaken_refusal_at_the_end_of_each_input() {
    use common::*;
    use jackin_console::fixtures::world_for;
    use jackin_console::scenario::Scenario;
    clear_refusal();
    let mut app = app_with(world_for(Scenario::Returning));
    Control::fixed(reason()).refuse();
    press(&mut app, "down");
    assert_eq!(taken(), None);
}

#[test]
fn a_refusal_posts_the_reason_alone_in_a_neutral_tone() {
    clear_refusal();
    Control::fixed(reason()).refuse();
    let id = take_refusal().expect("a refusal");
    let mut focus = Focus::default();
    let ring = FocusRing::default();
    let mut cx: Cx<'_, (), (), ()> = Cx::new(&mut focus, &ring);
    cx.refused(id);
    let mut requests = cx.take();
    assert_eq!(requests.len(), 1);
    let Some(Request::Status(text, tone)) = requests.pop() else {
        panic!("expected one status request");
    };
    assert_eq!(text, WHY);
    assert_ne!(tone, Tone::Error);
    let mut line = StatusLine::<()>::new();
    line.set(text, tone);
    assert_eq!(line.text(), Some(WHY));
    assert!(!line.is_error());
}

#[test]
fn a_refusal_status_lasts_five_seconds() {
    use junie_tui::clock::Moment;
    let mut line = StatusLine::<()>::new();
    line.set_at(WHY, Tone::Secondary, Moment::ZERO);
    assert!(!line.expire(Moment::from_millis(4_999)));
    assert_eq!(line.text(), Some(WHY));
    assert!(line.expire(Moment::from_millis(5_000)));
    assert_eq!(line.text(), None);
}

#[test]
fn a_fixed_row_shows_the_focus_through_the_shared_row_painter() {
    use junie_tui::theme::CursorStyle;
    use junie_tui::ui::ctx::VisualState;
    use junie_tui::ui::row::{RowLook, RowUi};

    let theme = Theme::for_level(ColorLevel::TrueColor);
    let paint = |look: RowLook, state: VisualState| {
        let mut buf = Buffer::empty(Rect::new(0, 0, 20, 1));
        let mut hits = HitRegistry::default();
        let mut ring = FocusRing::default();
        let ctx = RenderCtx::new(&theme, Interaction::default(), &mut hits, &mut ring);
        RowUi::paint(
            &ctx,
            &mut buf,
            Rect::new(0, 0, 20, 1),
            state,
            look,
            theme.canvas,
        );
        (
            buf[(0, 0)].symbol().to_owned(),
            buf[(5, 0)].bg,
            buf[(5, 0)].fg,
        )
    };
    let off = VisualState {
        disabled: true,
        ..Default::default()
    };
    let fixed = VisualState {
        disabled: true,
        fixed: true,
        focused: true,
        ..Default::default()
    };
    let fixed_rest = VisualState {
        focused: false,
        ..fixed
    };
    let fill = RowLook::Fill(CursorStyle::Fill);

    let (gutter, _, _) = paint(RowLook::Bar, fixed);
    assert_eq!(gutter, "▎");
    assert_eq!(paint(RowLook::Bar, off).0, " ");
    assert_eq!(paint(RowLook::Bar, fixed_rest).0, " ");

    let (_, fixed_bg, fixed_ink) = paint(fill, fixed);
    let (_, off_bg, off_ink) = paint(fill, off);
    assert_eq!(fixed_bg, theme.ladders.highlight.rest);
    assert_ne!(off_bg, theme.ladders.highlight.rest);
    assert_eq!(fixed_ink, off_ink);
}
