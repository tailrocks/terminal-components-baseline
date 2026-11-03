#![allow(dead_code)]

use jackin_console::app::App;
use jackin_console::fixtures::world_for;
use jackin_console::scenario::{Motion, Scenario};
use jackin_console::world::World;
use junie_tui::core::event::{Input, Key, Mouse, MouseKind};
use junie_tui::runtime::Application;
use junie_tui::theme::{ColorLevel, Theme};
use ratatui::Terminal;
use ratatui::backend::TestBackend;
use ratatui::crossterm::event::{KeyCode, KeyModifiers};

pub fn app_with(world: World) -> App {
    let scenario = world.scenario;
    App::for_scenario(
        scenario,
        world,
        Motion::Paused,
        10_000,
        Theme::for_level(ColorLevel::TrueColor),
    )
}

pub fn app(scenario: Scenario) -> App {
    app_with(world_for(scenario))
}

pub fn screen(app: &mut App) -> String {
    screen_at(app, 140, 40)
}

pub fn screen_at(app: &mut App, width: u16, height: u16) -> String {
    let mut terminal = Terminal::new(TestBackend::new(width, height)).expect("test terminal");
    terminal
        .draw(|f| Application::render(app, f))
        .expect("draw");
    let buffer = terminal.backend().buffer();
    buffer
        .content()
        .chunks(buffer.area.width as usize)
        .map(|row| row.iter().map(|c| c.symbol()).collect::<String>())
        .collect::<Vec<_>>()
        .join("\n")
}

pub fn press(app: &mut App, keys: &str) {
    for name in keys.split_whitespace() {
        let (mods, name) = match (name.strip_prefix("ctrl+"), name.strip_prefix("shift+")) {
            (Some(rest), _) => (KeyModifiers::CONTROL, rest),
            (_, Some(rest)) => (KeyModifiers::SHIFT, rest),
            _ => (KeyModifiers::NONE, name),
        };
        let code = match name {
            "f10" => KeyCode::F(10),
            "enter" => KeyCode::Enter,
            "esc" => KeyCode::Esc,
            "down" => KeyCode::Down,
            "up" => KeyCode::Up,
            "right" => KeyCode::Right,
            "left" => KeyCode::Left,
            "tab" => KeyCode::Tab,
            "space" => KeyCode::Char(' '),
            c if c.chars().count() == 1 => KeyCode::Char(c.chars().next().expect("one char")),
            other => panic!("unknown key {other}"),
        };
        Application::handle(app, Input::Key(Key { code, mods }));
        screen(app);
    }
}

pub fn click(app: &mut App, x: u16, y: u16) {
    for kind in [MouseKind::Down, MouseKind::Up] {
        Application::handle(
            app,
            Input::Mouse(Mouse::new(kind, ratatui::layout::Position::new(x, y))),
        );
        screen(app);
    }
}

pub fn click_text(app: &mut App, text: &str) {
    let screen = screen(app);
    let (y, line) = screen
        .lines()
        .enumerate()
        .find(|(_, l)| l.contains(text))
        .unwrap_or_else(|| panic!("{text} not on screen:\n{screen}"));
    let x = line[..line.find(text).expect("found")].chars().count();
    click(app, x as u16 + 1, y as u16);
}
