#![allow(dead_code)]

use jackin_console::app::App;
use jackin_console::fixtures::world_for;
use jackin_console::scenario::{Motion, Scenario};
use jackin_console::world::World;
use junie_tui::core::event::{Input, Key};
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
    let mut terminal = Terminal::new(TestBackend::new(140, 40)).expect("test terminal");
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
        let code = match name {
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
        Application::handle(
            app,
            Input::Key(Key {
                code,
                mods: KeyModifiers::NONE,
            }),
        );
        screen(app);
    }
}
