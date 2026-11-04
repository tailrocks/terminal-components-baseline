#![cfg(feature = "demo")]

mod common;

use std::sync::{Arc, Mutex};

use jackin_console::app::Route;
use jackin_console::backend::{Backend, Completion, Effect, FileSource};
use jackin_console::domain::instance::Instance;
use jackin_console::domain::launch::LaunchContent;
use jackin_console::fixtures::{Scenario, app_with_world, world_for};
use jackin_console::launch::{LaunchFeed, LaunchTarget};
use jackin_console::panes::{PaneBody, PaneRequest, PaneSpawn};
use jackin_console::sim::SimBackend;
use jackin_console::world::{DaemonHealth, Msg, World};
use junie_tui::core::event::Input;
use junie_tui::motion::Motion;
use junie_tui::runtime::Application;
use junie_tui::theme::{ColorLevel, Theme};

/// The simulator, plus one message the backend raises on its own at its first
/// `due` call, the way a second client attaching would be.
#[derive(Debug)]
struct Raising {
    inner: Arc<dyn Backend>,
    once: Mutex<Option<Msg>>,
}

impl Backend for Raising {
    fn perform(&self, effect: &Effect, world: &World) -> Option<Completion> {
        self.inner.perform(effect, world)
    }

    fn due(&self, _: &World) -> Vec<Msg> {
        self.once.lock().expect("once").take().into_iter().collect()
    }

    fn settle(&self, effect: &Effect, world: &World) -> Option<Msg> {
        self.inner.settle(effect, world)
    }

    fn launch_content(&self) -> Arc<LaunchContent> {
        self.inner.launch_content()
    }

    fn launch(&self, world: &World, target: &LaunchTarget) -> Box<dyn LaunchFeed> {
        self.inner.launch(world, target)
    }

    fn spawn_pane(&self, request: &PaneRequest, spawn: PaneSpawn) -> Box<dyn PaneBody> {
        self.inner.spawn_pane(request, spawn)
    }

    fn initial_panes(&self, instance: &Instance) -> Vec<PaneRequest> {
        self.inner.initial_panes(instance)
    }

    fn file_source(&self, world: &World) -> Box<dyn FileSource> {
        self.inner.file_source(world)
    }
}

#[test]
fn a_message_raised_during_the_intro_is_delivered_when_the_manager_shows() {
    let mut world = world_for(Scenario::FirstUse);
    world.backend = Arc::new(Raising {
        inner: SimBackend::shared(),
        once: Mutex::new(Some(Msg::Refreshed {
            result: Err("gateway unreachable".into()),
        })),
    });
    let mut app = app_with_world(
        Scenario::FirstUse,
        world,
        Motion::Full,
        0,
        Theme::for_level(ColorLevel::TrueColor),
    );
    assert_eq!(app.route(), Route::Intro, "the run starts on the intro");

    Application::handle(&mut app, Input::Tick);
    assert_eq!(
        app.route(),
        Route::Intro,
        "the first tick is still the intro"
    );
    assert_eq!(
        app.world.daemon_health,
        DaemonHealth::Healthy,
        "the intro cannot deliver it yet"
    );

    for _ in 0..4_000 {
        if app.route() != Route::Intro {
            break;
        }
        Application::handle(&mut app, Input::Tick);
    }
    assert_eq!(app.route(), Route::Manager, "the intro ended");
    for _ in 0..3 {
        Application::handle(&mut app, Input::Tick);
    }
    assert_eq!(
        app.world.daemon_health,
        DaemonHealth::Stale,
        "the message was held, not dropped"
    );
}
