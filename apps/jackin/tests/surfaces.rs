use jackin_console::domain::agent::{Agent, AgentBlocked};
use jackin_console::domain::docker::{DockerSetting, EngineState};
use jackin_console::domain::doctor::{DoctorState, DoctorSurface};
use jackin_console::domain::fixed::Fixed;
use jackin_console::domain::role::{AccountOrder, LaunchRoleRule, RoleAdmin};
use jackin_console::domain::surfaces::{LiveFacts, Surfaces};
use jackin_console::domain::vocabulary::Vocabulary;
use jackin_console::domain::workspace::WizardDefaults;

fn live() -> Surfaces {
    Surfaces::openshell(LiveFacts {
        version: "1.4.0".into(),
    })
}

#[test]
fn surfaces_openshell_sets_every_field() {
    let Surfaces {
        roles,
        docker,
        doctor,
        vocabulary,
        account_form,
        agent_blocked,
        launch_role,
        wizard,
        fixed,
        version,
        cockpit_version,
        about,
    } = live();
    assert_eq!(fixed, Some(Fixed::openshell()));
    assert_eq!(vocabulary, Vocabulary::openshell());
    assert_eq!(launch_role, LaunchRoleRule::Configured);
    assert_eq!(version, "1.4.0");
    assert_eq!(cockpit_version, None);
    assert_eq!(account_form.default_source, Some(1));
    let labels: Vec<&str> = account_form
        .sources
        .iter()
        .map(|s| s.label.as_str())
        .collect();
    assert_eq!(
        labels,
        [
            "1Password item / field  (usage only)",
            "Local agent folder  (recommended)",
            "Plain-text API key",
        ]
    );
    assert_eq!(
        account_form.op_note.as_deref(),
        Some("Usage only · launching Codex needs a local login")
    );
    assert_eq!(
        agent_blocked,
        Some(AgentBlocked {
            launchable: Agent::Codex,
            text: "usage only · jackin launches Codex only".into(),
        })
    );
    assert_eq!(
        wizard,
        Some(WizardDefaults {
            destination: "/sandbox/project".into(),
            destination_label: "Project folder".into(),
        })
    );
    assert!(roles.is_none() && docker.is_none() && doctor.is_none() && about.is_empty());

    // The data the host supplies arrives through builders, never as text.
    let roles = RoleAdmin {
        agents: vec![Agent::Codex],
        roles: Vec::new(),
        order: AccountOrder::Jackin,
    };
    let docker = DockerSetting {
        gateway: "gateway".into(),
        engine: EngineState::Missing,
        risks: Vec::new(),
        role_enabled: Default::default(),
    };
    let doctor = DoctorSurface {
        noun: "gateway".into(),
        state: DoctorState::NeverRun,
    };
    let about = vec![("Product".to_owned(), "jackin 1.4.0".to_owned())];
    let surfaces = live()
        .roles(roles.clone())
        .docker(docker.clone())
        .doctor(doctor.clone())
        .about(about.clone());
    assert_eq!(surfaces.roles, Some(roles));
    assert_eq!(surfaces.docker, Some(docker));
    assert_eq!(surfaces.doctor, Some(doctor));
    assert_eq!(surfaces.about, about);
    assert_eq!(surfaces.vocabulary, live().vocabulary);
}

#[cfg(feature = "demo")]
mod common;

#[cfg(feature = "demo")]
mod demo {
    use super::common::*;
    use jackin_console::app::App;
    use jackin_console::domain::account::LaunchBlock;
    use jackin_console::domain::agent::{Agent, Provider};
    use jackin_console::domain::fixed::Fixed;
    use jackin_console::domain::role::LaunchRoleRule;
    use jackin_console::domain::surfaces::{LiveFacts, Surfaces};
    use jackin_console::domain::vocabulary::Vocabulary;
    use jackin_console::fixtures::{Scenario, world_for};
    use jackin_console::world::World;

    fn status(app: &App) -> String {
        app.shell.status.text().unwrap_or_default().to_owned()
    }

    fn pair(scenario: Scenario) -> [World; 2] {
        let demo = world_for(scenario);
        let mut live = world_for(scenario);
        live.surfaces.vocabulary = Vocabulary::openshell();
        [demo, live]
    }

    fn after(scenario: Scenario, world: World, keys: &str) -> App {
        let mut app = app_with(scenario, world);
        screen(&mut app);
        press(&mut app, keys);
        app
    }

    #[test]
    fn openshell_capsule_word_is_capsule() {
        let mut world = world_for(Scenario::CapsuleMulti);
        world.surfaces = Surfaces::openshell(LiveFacts {
            version: "1.4.0".into(),
        });
        let mut app = app_with(Scenario::CapsuleMulti, world);
        press(&mut app, "f10 right right right right enter");
        let live = screen(&mut app);
        assert!(
            live.contains("Capsule › payments") && live.contains("  Capsule  "),
            "{live}"
        );
        assert!(!live.contains("Terminal"));
    }

    #[test]
    fn demo_surfaces_stay_unset() {
        for scenario in Scenario::ALL {
            let surfaces = Surfaces::demo(scenario);
            assert!(surfaces.fixed.is_none(), "{}", scenario.name());
            assert_eq!(surfaces.account_form.default_source, None);
            assert_eq!(surfaces.account_form.op_note, None);
            assert!(surfaces.agent_blocked.is_none() && surfaces.wizard.is_none());
            assert_eq!(surfaces.launch_role, LaunchRoleRule::Demo);
            assert_eq!(surfaces.vocabulary, Vocabulary::demo());
            assert_eq!(surfaces.version, "0.9.2");
            assert_eq!(surfaces.cockpit_version.as_deref(), Some("0.6.4 · preview"));
        }
        for scenario in [
            Scenario::FirstUse,
            Scenario::Returning,
            Scenario::CapsuleMulti,
        ] {
            assert_eq!(
                world_for(scenario).surfaces,
                Surfaces::demo(scenario),
                "{}",
                scenario.name()
            );
        }
    }

    #[test]
    fn launch_block_text_comes_from_fixed() {
        fn codex_row(world: World) -> String {
            let mut app = app_with(Scenario::Returning, world);
            press(&mut app, "down enter");
            let screen = screen(&mut app);
            screen
                .lines()
                .find(|l| l.contains("▪ Codex"))
                .unwrap_or_else(|| panic!("no Codex row\n{screen}"))
                .to_owned()
        }
        fn blocked(block: Option<LaunchBlock>, fixed: &Fixed) -> World {
            let mut world = world_for(Scenario::Returning);
            world.surfaces.fixed = Some(fixed.clone());
            let ids: Vec<_> = world
                .accounts
                .by_provider(Provider::OpenAi)
                .map(|a| a.id.clone())
                .collect();
            for id in ids {
                world.accounts.get_mut(&id).expect("account").launch_blocked = block;
            }
            world
        }
        for block in LaunchBlock::ALL {
            let mut fixed = Fixed::openshell();
            let row = codex_row(blocked(Some(block), &fixed));
            let shown: String = block.text(Some(&fixed)).chars().take(30).collect();
            assert!(row.contains(&shown), "{row}");
            assert!(row.contains("unavailable"), "{row}");
            fixed.launch_usage_only = "needs a local login".into();
            let row = codex_row(blocked(Some(block), &fixed));
            assert!(row.contains("needs a local login"), "{row}");
        }
        let world = blocked(None, &Fixed::openshell());
        let ws = world.workspaces[0].clone();
        for ws in [Some(&ws), None] {
            let offer = world.offer_for(Agent::Codex, ws, Some("the-architect"));
            assert!(offer.blocked.is_none() && offer.preselected.is_some());
        }
        assert!(!codex_row(world).contains("unavailable"));
    }

    #[test]
    fn demo_literals_are_data() {
        const DEMO: [&str; 10] = [
            "Preview clipboard is empty",
            "not available in the preview",
            "Not in this preview",
            "About jackin-preview",
            "revealed in Finder",
            "spawning",
            "Pane cleared",
            "Paste clipboard",
            "Reap every pane in this tab",
            "Reap the focused pane's agent",
        ];
        let [demo_paste, live_paste] = pair(Scenario::CapsuleMulti).map(|w| {
            let mut app = app_with(Scenario::CapsuleMulti, w);
            screen(&mut app);
            press(&mut app, "f10 right");
            let menu = screen(&mut app);
            press(&mut app, "down enter");
            format!("{menu}\n{}", status(&app))
        });
        assert!(demo_paste.contains("Paste clipboard"));
        assert!(demo_paste.ends_with("Preview clipboard is empty"));
        assert!(live_paste.contains("Paste last copy"));
        assert!(live_paste.ends_with("Nothing copied in this console yet"));
        let [demo_clear, live_clear] = pair(Scenario::CapsuleMulti).map(|w| {
            status(&after(
                Scenario::CapsuleMulti,
                w,
                "f10 right down down down enter",
            ))
        });
        assert_eq!(demo_clear, "Pane cleared");
        assert_eq!(live_clear, "Scrollback cleared");
        let export =
            "ctrl+\\ E x p o r t space f i l e space a n d space r e v e a l enter a . t x t enter";
        let [demo_export, live_export] =
            pair(Scenario::CapsuleMulti).map(|w| status(&after(Scenario::CapsuleMulti, w, export)));
        assert_eq!(
            demo_export,
            "Exported to ~/Downloads/jackin/payments-platform/a.txt · revealed in Finder"
        );
        assert_eq!(
            live_export,
            "Exported to ~/Downloads/jackin/payments-platform/a.txt"
        );
        let [demo_stage, live_stage] = pair(Scenario::CapsuleMulti).map(|w| {
            let mut app = after(Scenario::CapsuleMulti, w, "ctrl+\\ S t a g e");
            screen(&mut app)
        });
        assert!(demo_stage.contains("Not in this preview"));
        assert!(
            live_stage.contains("Not available") && !live_stage.contains("Not in this preview")
        );
        let [demo_close, live_close] = pair(Scenario::CapsuleMulti)
            .map(|w| screen(&mut after(Scenario::CapsuleMulti, w, "ctrl+b x")));
        assert!(demo_close.contains("Reap the focused pane's agent"));
        assert!(live_close.contains("Codex keeps running in the sandbox."));
        let [demo_tab, live_tab] = pair(Scenario::CapsuleMulti)
            .map(|w| screen(&mut after(Scenario::CapsuleMulti, w, "ctrl+b 2 ctrl+b x")));
        assert!(demo_tab.contains("Reap every pane in this tab"));
        assert!(live_tab.contains("Closes every pane in this tab."));
        let [demo_shell, live_shell] = pair(Scenario::CapsuleMulti).map(|w| {
            let mut app = app_with(Scenario::CapsuleMulti, w);
            screen(&mut app);
            click_text(&mut app, "╭─ Shell");
            press(&mut app, "ctrl+b x");
            screen(&mut app)
        });
        assert!(demo_shell.contains("Reap the focused pane's agent"));
        assert!(live_shell.contains("The shell ends. Unsaved state in it is lost."));
        let [demo_about, live_about] = pair(Scenario::CapsuleMulti).map(|w| {
            let mut app = app_with(Scenario::CapsuleMulti, w);
            screen(&mut app);
            click(&mut app, 3, 0);
            screen(&mut app)
        });
        assert!(demo_about.contains("About jackin-preview"));
        assert!(live_about.contains("About jackin") && !live_about.contains("jackin-preview"));
        let [demo_info, live_info] = pair(Scenario::CapsuleMulti)
            .map(|w| screen(&mut after(Scenario::CapsuleMulti, w, "ctrl+b i")));
        assert!(demo_info.contains("Host log") && demo_info.contains("file:///Users/alexey/"));
        assert!(!live_info.contains("Host log") && !live_info.contains("/Users/alexey"));
        let editor = "down e ] enter r i ctrl+s";
        let [demo_scope, live_scope] =
            pair(Scenario::Returning).map(|w| screen(&mut after(Scenario::Returning, w, editor)));
        assert!(demo_scope.contains("workspace config · ~/.jackin/workspaces/"));
        assert!(live_scope.contains("workspace config") && !live_scope.contains(".jackin"));
        for banned in DEMO {
            for (name, text) in [
                ("paste", &live_paste),
                ("clear", &live_clear),
                ("export", &live_export),
                ("stage", &live_stage),
                ("close", &live_close),
                ("tab", &live_tab),
                ("shell", &live_shell),
                ("about", &live_about),
            ] {
                assert!(!text.contains(banned), "live {name} says {banned}");
            }
        }
    }
}
