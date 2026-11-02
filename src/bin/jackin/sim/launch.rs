use std::sync::Arc;

use junie_tui::console::launch::{
    LaunchContent, LaunchFailure, LaunchWords, StageKind, StageList, StageSpec,
};

pub const BUILD_LOG: [&str; 44] = [
    "#1 [internal] load build definition from Dockerfile.derived",
    "#1 transferring dockerfile: 2.31kB done",
    "#1 DONE 0.0s",
    "#2 [internal] load metadata for ghcr.io/jackin/construct:0.6.4",
    "#2 DONE 0.4s",
    "#3 [internal] load .dockerignore",
    "#3 transferring context: 2B done",
    "#3 DONE 0.0s",
    "#4 [ 1/9] FROM ghcr.io/jackin/construct:0.6.4@sha256:9c41e2f0a7b3c5d6e8f9a0b1c2d3e4f5a6b7c8d9e0f1a2b3c4d5e6f7a8b9c0d1",
    "#4 resolve ghcr.io/jackin/construct:0.6.4 0.1s done",
    "#4 CACHED",
    "#5 [internal] load build context",
    "#5 transferring context: 41.02kB 0.0s done",
    "#5 DONE 0.0s",
    "#6 [ 2/9] RUN --mount=type=cache,target=/var/cache/apt apt-get update && apt-get install -y --no-install-recommends git-lfs ripgrep fd-find jq",
    "#6 0.812 Get:1 http://deb.debian.org/debian bookworm InRelease [151 kB]",
    "#6 1.204 Get:2 http://deb.debian.org/debian bookworm-updates InRelease [55.4 kB]",
    "#6 2.331 Reading package lists...",
    "#6 3.118 Building dependency tree...",
    "#6 3.902 The following NEW packages will be installed: fd-find git-lfs jq ripgrep",
    "#6 5.410 Setting up ripgrep (13.0.0-4+b2) ...",
    "#6 5.612 Setting up git-lfs (3.3.0-1+b5) ...",
    "#6 DONE 6.1s",
    "#7 [ 3/9] RUN corepack enable && corepack prepare pnpm@9.12.0 --activate",
    "#7 1.930 Preparing pnpm@9.12.0 for immediate activation...",
    "#7 DONE 2.4s",
    "#8 [ 4/9] COPY roles/the-architect/manifest.toml /jackin/role/manifest.toml",
    "#8 DONE 0.0s",
    "#9 [ 5/9] RUN /jackin/bin/role-install --manifest /jackin/role/manifest.toml",
    "#9 0.211 role: the-architect@1.4.2 (github.com/chainargos/roles)",
    "#9 0.480 install: cargo-nextest 0.9.72",
    "#9 4.115 install: sqlfluff 3.1.0",
    "#9 7.309 warning: hook post-install.sh exited 0 with output on stderr",
    "#9 DONE 7.6s",
    "#10 [ 6/9] RUN useradd --uid 501 --create-home operator && chown -R operator /workspace",
    "#10 DONE 0.5s",
    "#11 [ 7/9] COPY --chown=operator agent-home/ /home/operator/.claude/",
    "#11 DONE 0.1s",
    "#12 [ 8/9] RUN /jackin/bin/agent-install claude@2.1.14",
    "#12 0.104 claude 2.1.14 already present in layer cache",
    "#12 DONE 0.2s",
    "#13 [ 9/9] LABEL org.jackin.run=9c41e2f0 org.jackin.workspace=payments-platform",
    "#13 DONE 0.0s",
    "#14 exporting to image",
];

fn stages() -> StageList {
    StageList::new(
        StageSpec::new("identity", "Identity", "Resolving launch identity…", 14),
        [
            StageSpec::new("role", "Role", "Loading role manifest…", 18),
            StageSpec::new("credentials", "Credentials", "Resolving credentials…", 26)
                .kind(StageKind::Credentials { hold_at: 10 }),
            StageSpec::new(
                "construct",
                "Construct",
                "Preparing the Construct image…",
                30,
            )
            .container_at(6),
            StageSpec::new(
                "agent-binaries",
                "Agent Binaries",
                "Installing {agent} binaries…",
                8,
            )
            .skipped(),
            StageSpec::new(
                "derived-image",
                "Derived Image",
                "Building derived image…",
                92,
            )
            .kind(StageKind::BuildLog),
            StageSpec::new("workspace", "Workspace", "Mounting the Workspace…", 22),
            StageSpec::new("network", "Network", "Attaching the network…", 20),
            StageSpec::new("sidecar", "Sidecar", "Starting the sidecar…", 18),
            StageSpec::new("capsule", "Capsule", "Waiting for the Capsule daemon…", 34),
            StageSpec::new("hardline", "Hardline", "Opening the hardline…", 16),
        ],
    )
}

fn words() -> LaunchWords {
    LaunchWords {
        unit: "Container".into(),
        unit_missing: "not created".into(),
        log_title: "Docker build".into(),
        log_waiting: "(waiting for docker build output…)".into(),
        build_failed: "Docker build failed".into(),
        ready_activity: "Hardline open · handing off to the Capsule".into(),
        credentials_resolved: "resolved in memory · discarded after injection".into(),
    }
}

pub fn content() -> Arc<LaunchContent> {
    let stages = stages();
    let network = stages.position("network").unwrap_or(0);
    Arc::new(LaunchContent {
        stages,
        words: words(),
        build_log: BUILD_LOG.iter().map(|l| (*l).to_owned()).collect(),
        agent_provider_only: false,
        name_slug: None,
        network_failure: LaunchFailure {
                    stage: network,
                    summary: "The Construct network could not be attached".into(),
                    next_step: "Check that Docker's jackin-net bridge exists (docker network ls), then launch again.".into(),
                    detail: vec![
                        "network: attach jackin-net to jackin-payments-platform-7f3a".into(),
                        "docker: Error response from daemon: network jackin-net not found".into(),
                        "retry 1/3 after 400 ms: network jackin-net not found".into(),
                        "retry 2/3 after 800 ms: network jackin-net not found".into(),
                        "retry 3/3 after 1600 ms: network jackin-net not found".into(),
                        "sidecar: not started (network stage failed)".into(),
                        "capsule: not started".into(),
                        "cleanup: container jackin-payments-platform-7f3a kept for inspection".into(),
                        "credentials: material was resolved in memory and discarded".into(),
                        "run: 9c41e2f0 · backend docker 27.1 · host darwin 25.5".into(),
                    ],
                },
    })
}

pub const OPENSHELL_BUILD_LOG: [&str; 12] = [
    "#1 [internal] load build definition from Dockerfile",
    "#1 DONE 0.0s",
    "#2 [internal] load metadata for registry.jackin.local/construct:0.7.0",
    "#2 DONE 0.3s",
    "#3 [1/4] FROM registry.jackin.local/construct:0.7.0",
    "#3 CACHED",
    "#4 [2/4] COPY role/ /jackin/role/",
    "#4 DONE 0.1s",
    "#5 [3/4] RUN /jackin/bin/role-install --manifest /jackin/role/jackin.role.toml",
    "#5 4.210 install: codex 0.46.0",
    "#5 DONE 5.2s",
    "#6 exporting to image",
];

fn openshell_stages() -> StageList {
    StageList::new(
        StageSpec::new("image", "Image build", "Building the role image…", 30)
            .kind(StageKind::BuildLog),
        [
            StageSpec::new(
                "provider",
                "OpenShell provider",
                "Preparing the OpenShell provider…",
                10,
            ),
            StageSpec::new("sandbox", "Sandbox", "Creating the sandbox…", 22).container_at(6),
            StageSpec::new("credentials", "Credentials", "Resolving credentials…", 14)
                .kind(StageKind::Credentials { hold_at: 10 }),
            StageSpec::new("upload", "Upload", "Uploading the project…", 30),
            StageSpec::new(
                "hooks",
                "Hooks",
                "Running the setup and preflight hooks…",
                18,
            ),
            StageSpec::new("codex", "Codex", "Starting {agent}…", 16),
        ],
    )
}

pub fn openshell_content() -> Arc<LaunchContent> {
    let stages = openshell_stages();
    let sandbox = stages.position("sandbox").unwrap_or(0);
    Arc::new(LaunchContent {
        stages,
        words: LaunchWords {
            unit: "Sandbox".into(),
            unit_missing: "not created".into(),
            log_title: "Image build".into(),
            log_waiting: "(waiting for build output…)".into(),
            build_failed: "Image build failed".into(),
            ready_activity: "Codex is running · opening the session".into(),
            credentials_resolved: "resolved in memory · discarded".into(),
        },
        build_log: OPENSHELL_BUILD_LOG
            .iter()
            .map(|l| (*l).to_owned())
            .collect(),
        agent_provider_only: true,
        name_slug: Some(7),
        network_failure: LaunchFailure {
            stage: sandbox,
            summary: "The sandbox could not be created".into(),
            next_step: "Run jackin doctor, fix the failing check, then launch again.".into(),
            detail: vec![
                "gateway: jackin-local".into(),
                "sandbox: create failed".into(),
            ],
        },
    })
}
