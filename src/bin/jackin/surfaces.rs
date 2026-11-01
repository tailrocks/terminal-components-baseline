use std::collections::BTreeSet;

use junie_tui::console::domain::docker::{DockerRiskText, DockerSetting, EngineState};
use junie_tui::console::world::World;

pub const GATEWAY: &str = "jackin-local";
const ENDPOINT: &str = "host.openshell.internal:18476";

fn risk(label: &str, text: &str) -> DockerRiskText {
    DockerRiskText {
        label: label.into(),
        text: text.into(),
    }
}

pub fn docker_setting(engine: EngineState) -> DockerSetting {
    DockerSetting {
        gateway: GATEWAY.into(),
        engine,
        risks: vec![
            risk("Client key", "The agent can read the engine client key."),
            risk("Network", "Containers bypass the sandbox network policy."),
            risk(
                "Bind mounts",
                "A bind mount names a path on the engine host, not in the sandbox.",
            ),
            risk(
                "Lifetime",
                "The engine outlives the sandbox. Sandboxes that share one engine see each other's containers, and you remove them.",
            ),
        ],
        role_enabled: BTreeSet::from(["reviewer".to_owned()]),
    }
}

pub fn docker(w: &mut World, engine_missing: bool) {
    let engine = if engine_missing {
        EngineState::Missing
    } else {
        EngineState::Configured {
            endpoint: ENDPOINT.into(),
        }
    };
    w.surfaces.docker = Some(docker_setting(engine));
    if let Some(ws) = w.workspace_mut(2) {
        ws.docker = true;
    }
}
