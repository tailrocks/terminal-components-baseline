#![cfg(feature = "demo")]

mod common;

use common::*;
use jackin_console::fixtures::{Scenario, world_for};
use junie_tui::core::event::{Input, Mouse, MouseKind};
use junie_tui::runtime::Application;
use ratatui::layout::Position;

const TRACK: char = '┆';

fn many_variables() -> jackin_console::app::App {
    let mut world = world_for(Scenario::RoleEditor);
    let doc = world
        .surfaces
        .roles
        .as_mut()
        .and_then(|admin| admin.role_mut("sentinel"))
        .and_then(|rec| rec.manifest.as_mut().ok())
        .expect("the sentinel manifest");
    let first = doc.env[0].clone();
    for i in 0..40 {
        let mut entry = first.clone();
        entry.name = format!("VAR_{i:02}");
        doc.env.push(entry);
    }
    let mut app = app_with(Scenario::RoleEditor, world);
    press(&mut app, "right right");
    app
}

fn track_end(app: &mut jackin_console::app::App) -> (u16, u16) {
    let shown = screen(app);
    let (y, line) = shown
        .lines()
        .enumerate()
        .filter(|(_, l)| l.contains(TRACK))
        .last()
        .expect("a scroll track");
    let x = line.chars().position(|c| c == TRACK).expect("track column");
    (x as u16, y as u16)
}

#[test]
fn a_click_on_the_environment_track_scrolls_it() {
    let mut app = many_variables();
    assert!(screen(&mut app).contains("SENTINEL_FAIL_PREFLIGHT"));
    let (x, y) = track_end(&mut app);
    click(&mut app, x, y);

    let shown = screen(&mut app);
    assert!(shown.contains("VAR_39"), "{shown}");
    assert!(!shown.contains("SENTINEL_FAIL_PREFLIGHT"), "{shown}");
}

#[test]
fn a_drag_on_the_environment_track_scrolls_it() {
    let mut app = many_variables();
    let (x, y) = track_end(&mut app);
    let thumb = shown_thumb_row(&mut app, x);
    for (kind, at) in [(MouseKind::Down, thumb), (MouseKind::Drag, y)] {
        Application::handle(
            &mut app,
            Input::Mouse(Mouse::new(kind, Position::new(x, at))),
        );
        screen(&mut app);
    }

    let shown = screen(&mut app);
    assert!(shown.contains("VAR_39"), "{shown}");
}

#[test]
fn the_wheel_over_the_environment_track_scrolls_it() {
    let mut app = many_variables();
    assert!(screen(&mut app).contains("SENTINEL_FAIL_PREFLIGHT"));
    let (x, y) = track_end(&mut app);
    for _ in 0..60 {
        Application::handle(
            &mut app,
            Input::Mouse(Mouse::new(MouseKind::WheelDown, Position::new(x, y))),
        );
        screen(&mut app);
    }

    let shown = screen(&mut app);
    assert!(shown.contains("VAR_39"), "{shown}");
    assert!(!shown.contains("SENTINEL_FAIL_PREFLIGHT"), "{shown}");
}

fn shown_thumb_row(app: &mut jackin_console::app::App, x: u16) -> u16 {
    let shown = screen(app);
    shown
        .lines()
        .position(|l| l.chars().nth(x as usize) == Some('┃'))
        .expect("the thumb") as u16
}

#[test]
fn typing_in_the_file_chooser_filters_the_files() {
    let mut app = app(Scenario::RoleEditor);
    click_text(&mut app, "Choose…");
    let all = screen(&mut app);
    assert!(
        all.contains("hooks/setup.sh") && all.contains("policy.yaml"),
        "{all}"
    );

    press(&mut app, "p o l");
    let filtered = screen(&mut app);
    assert!(filtered.contains("policy.yaml"), "{filtered}");
    assert!(!filtered.contains("hooks/setup.sh"), "{filtered}");

    press(&mut app, "enter");
    let chosen = screen(&mut app);
    let mut lines = chosen.lines().skip_while(|l| !l.contains("Dockerfile *"));
    let field = lines
        .by_ref()
        .skip(1)
        .find(|l| !l.trim().is_empty())
        .expect("the field value");
    assert!(field.contains("policy.yaml"), "{chosen}");
}
