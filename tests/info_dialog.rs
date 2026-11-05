mod common;

use common::*;
use junie_tui::core::focus::FocusRing;
use junie_tui::core::hit::HitRegistry;
use junie_tui::core::id::WidgetId;
use junie_tui::theme::{ColorLevel, Theme};
use junie_tui::ui::ctx::{Interaction, RenderCtx};
use junie_tui::widgets::info_dialog::InfoDialog;
use junie_tui::widgets::props::Prop;
use ratatui::buffer::Buffer;
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
        shown >= 6,
        "the window fills the dialog, only {shown} props shown:\n{text}"
    );
}
