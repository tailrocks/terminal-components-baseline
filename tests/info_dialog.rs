mod common;

use common::*;
use junie_tui::core::focus::Focus;
use junie_tui::core::focus::FocusRing;
use junie_tui::core::hit::HitRegistry;
use junie_tui::core::id::WidgetId;
use junie_tui::theme::{ColorLevel, Theme};
use junie_tui::ui::ctx::{Interaction, RenderCtx};
use junie_tui::widgets::info_dialog::InfoDialog;
use junie_tui::widgets::props::Prop;
use ratatui::buffer::Buffer;
use ratatui::crossterm::event::KeyCode;
use ratatui::layout::Rect;

const ID: WidgetId = WidgetId::of("t.info");

fn dialog(screen: Rect) -> Buffer {
    let mut props: Vec<Prop> = (0..10)
        .map(|n| Prop::new(&format!("prop{n:02}"), format!("value {n:02}")))
        .collect();
    let long = "a very long note that needs several rows to be read whole ".repeat(6);
    props.push(Prop::new("note", long).wrap());
    let mut info = InfoDialog::new(ID, "Info", props);
    let theme = Theme::for_level(ColorLevel::TrueColor);
    let mut buf = Buffer::empty(screen);
    let mut hits = HitRegistry::default();
    let mut ring = FocusRing::default();
    let mut ctx = RenderCtx::new(&theme, Interaction::default(), &mut hits, &mut ring);
    info.render(screen, &mut buf, &mut ctx);
    buf
}

#[test]
fn a_wrapped_value_below_the_window_does_not_shrink_the_rows_shown() {
    let screen = Rect::new(0, 0, 80, 18);
    let buf = dialog(screen);
    let text = (0..screen.height)
        .map(|y| row(&buf, y))
        .collect::<Vec<_>>()
        .join("\n");
    let shown = (0..10)
        .filter(|n| text.contains(&format!("prop{n:02}")))
        .count();
    assert!(
        shown == 10,
        "the window fills the dialog, only {shown} props shown:\n{text}"
    );
}

fn lines_of(buf: &Buffer) -> Vec<String> {
    (0..buf.area.height).map(|y| row(buf, y)).collect()
}

#[test]
fn walking_the_keyboard_onto_a_wrapped_last_prop_keeps_the_cursor_drawn() {
    let screen = Rect::new(0, 0, 80, 14);
    let mut props: Vec<Prop> = (0..9)
        .map(|n| Prop::new(&format!("prop{n:02}"), format!("value {n:02}")))
        .collect();
    let long = "a very long note that needs several rows to be read whole ".repeat(4);
    props.push(Prop::new("note", long).wrap());
    let mut info = InfoDialog::new(ID, "Info", props);
    let theme = Theme::for_level(ColorLevel::TrueColor);
    let mut focus = Focus::default();
    focus.focus(info.initial_focus());
    let paint = |info: &mut InfoDialog, focus: &Focus| {
        let mut buf = Buffer::empty(screen);
        let mut hits = HitRegistry::default();
        let mut ring = FocusRing::default();
        let interaction = Interaction {
            focus: focus.current(),
            ..Default::default()
        };
        let mut ctx = RenderCtx::new(&theme, interaction, &mut hits, &mut ring);
        info.render(screen, &mut buf, &mut ctx);
        (buf, ring)
    };
    let (_, mut ring) = paint(&mut info, &focus);
    for step in 1..=9 {
        info.on_key(&key(KeyCode::Down), &mut focus, &ring);
        let (buf, next) = paint(&mut info, &focus);
        ring = next;
        let lines = lines_of(&buf);
        let label = if step == 9 {
            "note".to_owned()
        } else {
            format!("prop{step:02}")
        };
        assert!(
            lines.iter().any(|l| l.contains('▎') && l.contains(&label)),
            "step {step}: the marker sits on `{label}`:\n{}",
            lines.join("\n")
        );
    }
    let text = lines_of(&paint(&mut info, &focus).0).join("\n");
    assert!(
        text.contains("read whole"),
        "the wrapped value of the last prop is drawn:\n{text}"
    );
}
