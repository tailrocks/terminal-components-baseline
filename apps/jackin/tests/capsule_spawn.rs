#![cfg(feature = "demo")]
//! The new-tab picker asks the backend whether a pane may start.

mod common;

use std::sync::Arc;

use common::*;
use jackin_console::backend::{Backend, Completion, Effect};
use jackin_console::domain::instance::Instance;
use jackin_console::domain::launch::LaunchContent;
use jackin_console::fixtures::{Scenario, world_for};
use jackin_console::launch::{LaunchFeed, LaunchTarget};
use jackin_console::panes::{PaneBody, PaneKind, PaneRequest, PaneSpawn};
use jackin_console::sim::SimBackend;
use jackin_console::world::{Msg, World};
use junie_tui::core::refusal::Reason;

const WHY: &str = "one Codex pane per sandbox";

/// The simulator, except that it refuses to start an agent pane.
#[derive(Debug)]
struct OneAgent(Arc<dyn Backend>);

impl Backend for OneAgent {
    fn perform(&self, effect: &Effect, world: &World) -> Option<Completion> {
        self.0.perform(effect, world)
    }

    fn due(&self, world: &World) -> Vec<Msg> {
        self.0.due(world)
    }

    fn settle(&self, effect: &Effect, world: &World) -> Option<Msg> {
        self.0.settle(effect, world)
    }

    fn launch_content(&self) -> Arc<LaunchContent> {
        self.0.launch_content()
    }

    fn launch(&self, world: &World, target: &LaunchTarget) -> Box<dyn LaunchFeed> {
        self.0.launch(world, target)
    }

    fn spawn_pane(&self, request: &PaneRequest, spawn: PaneSpawn) -> Box<dyn PaneBody> {
        self.0.spawn_pane(request, spawn)
    }

    fn initial_panes(&self, instance: &Instance) -> Vec<PaneRequest> {
        self.0.initial_panes(instance)
    }

    fn spawnable(&self, _: &World, _: &Instance, kind: &PaneKind) -> Result<(), Reason> {
        match kind {
            PaneKind::Agent { .. } => Err(Reason::new(WHY)),
            PaneKind::Shell => Ok(()),
        }
    }
}

fn row_with(screen: &str, needle: &str) -> String {
    screen
        .lines()
        .find(|l| l.contains(needle))
        .unwrap_or_else(|| panic!("{needle:?} is not on the screen:\n{screen}"))
        .to_owned()
}

#[test]
fn spawnable_refusal_shows_its_reason() {
    let mut world = world_for(Scenario::CapsuleMulti);
    world.backend = Arc::new(OneAgent(SimBackend::shared()));
    let mut app = app_with(Scenario::CapsuleMulti, world);
    let tabs = app.world.daemons["jk-7f3a"].tabs.len();

    press(&mut app, "ctrl+b c");
    let picker = screen(&mut app);
    let codex = row_with(&picker, "▪ Codex");
    assert!(codex.contains(WHY), "{codex}");
    let shell = row_with(&picker, "$ Shell");
    assert!(shell.contains("zsh") && !shell.contains(WHY), "{shell}");

    click_text(&mut app, "▪ Codex");
    assert_eq!(
        app.world.daemons["jk-7f3a"].tabs.len(),
        tabs,
        "a refused row starts nothing"
    );
    assert!(screen(&mut app).contains(WHY), "the picker stays open");
}

#[test]
fn the_default_backend_refuses_nothing() {
    let mut app = app(Scenario::CapsuleMulti);
    press(&mut app, "ctrl+b c");
    let picker = screen(&mut app);
    assert!(!picker.contains(WHY));
    assert!(!row_with(&picker, "▪ Codex").contains("unavailable"));
}
