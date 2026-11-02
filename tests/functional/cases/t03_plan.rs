//! TablePro explain plan: the tree, the raw plan, and analysis are distinct.
//!
//! Tuiscotti test ids are `[a-z0-9_-]` segments, so the baseline directory is
//! `pre_refactor/t03_plan/` and each checkpoint's dots become underscores.
//! TablePro has no `--page` flag. It opens on Connections.

use std::time::{Duration, Instant};

use tui_test::TerminalSnapshot;

use crate::support::{App, Launch, TABLEPRO, find_all, row_text, text, try_launch};

const HOST: &str = "dev-db.internal.acme.io";
const DATABASE: &str = "acme_dev";
const CONNECTED: &str = "Connected to Development";
const QUERY: &str = "Query 1";
const SQL: &str = "SELECT sku FROM products";
const PLACEHOLDER: &str = "Type SQL";
const PLANNING: &str = "Planning 0.210 ms · r Raw";
const PLANNING_TIME: &str = "Planning Time: 0.210 ms";
const RAW_TITLE: &str = "EXPLAIN · raw";
const COST: &str = "Seq Scan on public.products  (cost=0.00..15.90 rows=312 width=64)";
const EXECUTION: &str = "Planning 0.210 ms · Execution 0.466 ms";

fn visual_id(checkpoint: &str) -> String {
    format!(
        "pre_refactor/t03_plan/{}",
        checkpoint.replace('.', "_").to_ascii_lowercase()
    )
}

fn open(name: &str) -> App {
    let mut launch = Launch::new(name, TABLEPRO);
    launch.cols = 120;
    launch.rows = 40;
    launch.color = Some("truecolor".to_string());
    launch.motion = Some("reduced".to_string());
    let app = try_launch(launch).unwrap_or_else(|error| panic!("{error}"));
    app.wait_text("Filter connections");
    app
}

fn see(screen: &TerminalSnapshot, present: &[&str], absent: &[&str]) {
    let visible = text(screen);
    for item in present {
        assert!(visible.contains(item), "missing {item:?}\n{visible}");
    }
    for item in absent {
        assert!(!visible.contains(item), "forbidden {item:?}\n{visible}");
    }
}

fn click_listed(app: &App, label: &str) {
    let screen = app.wait_text(label);
    let hits: Vec<_> = find_all(&screen, label)
        .into_iter()
        .filter(|hit| hit.x <= 35)
        .collect();
    assert_eq!(
        hits.len(),
        1,
        "list hit {label:?}: {hits:?}\n{}",
        text(&screen)
    );
    app.click(hits[0]);
}

fn click_connect(app: &App) {
    let screen = app.settle();
    let hits: Vec<_> = find_all(&screen, "Connect")
        .into_iter()
        .filter(|hit| {
            let row = row_text(&screen, u32::from(hit.y));
            hit.x >= 40
                && row.contains("Edit")
                && row.contains("Delete")
                && !row.contains("Reconnect")
        })
        .collect();
    assert_eq!(hits.len(), 1, "connect button {hits:?}\n{}", text(&screen));
    app.click(hits[0]);
}

fn connect_development(app: &App) {
    click_listed(app, "Development");
    let detail = app.wait_text(HOST);
    see(
        &detail,
        &[HOST, DATABASE, "Development"],
        &["Authentication failed", "Could not reach the host"],
    );
    click_connect(app);
    // Query 1 is drawn during "Loading schema…", before the connected status.
    let work = app.wait_screen("development connected", |screen| {
        let visible = text(screen);
        visible.contains(CONNECTED)
            && visible.contains(QUERY)
            && !visible.contains("Loading schema")
    });
    see(
        &work,
        &[CONNECTED, "acme_prod › public", QUERY, PLACEHOLDER],
        &[
            "Authentication failed",
            "Could not reach the host",
            "Scratch",
            "~/scratch.db",
            "Execute query?",
        ],
    );
}

fn shot(app: &App, checkpoint: &str) {
    app.visual(&visual_id(checkpoint));
}

fn finished(screen: &TerminalSnapshot) -> bool {
    !text(screen).contains("Esc cancels")
}

fn tree_body(screen: &TerminalSnapshot) -> bool {
    let visible = text(screen);
    finished(screen)
        && visible.contains("Operation")
        && visible.contains("Seq Scan")
        && visible.contains(PLANNING)
        && visible.contains(SQL)
        && !visible.contains(RAW_TITLE)
        && !visible.contains("Execution")
        && !visible.contains("Execute query?")
        && !visible.contains("Gather")
        && !visible.contains("Parallel")
        && !visible.contains("Sort")
}

fn raw_body(screen: &TerminalSnapshot) -> bool {
    let visible = text(screen);
    finished(screen)
        && visible.contains(RAW_TITLE)
        && visible.contains(PLANNING_TIME)
        && visible.contains(COST)
        && visible.contains(SQL)
        && !visible.contains("productsr")
        && !visible.contains("Operation")
        && !visible.contains("Execution")
        && !visible.contains("Execute query?")
}

fn raw_narrow(screen: &TerminalSnapshot) -> bool {
    let visible = text(screen);
    finished(screen)
        && visible.contains(RAW_TITLE)
        && visible.contains("Planning Time:")
        && visible.contains(SQL)
        && !visible.contains("Operation")
        && !visible.contains("productsr")
}

fn type_statement(app: &App) {
    let editor = app.wait_text(PLACEHOLDER);
    let hits = find_all(&editor, PLACEHOLDER);
    assert_eq!(
        hits.len(),
        1,
        "placeholder hits {hits:?}\n{}",
        text(&editor)
    );
    app.click(hits[0]);
    app.wait_screen("editor focused", |screen| {
        (0..screen.rows()).any(|row| {
            let line = row_text(screen, row);
            line.contains('▎') && line.contains(PLACEHOLDER)
        })
    });
    app.type_text(SQL);
    app.wait_text(SQL);
}

fn focus_plan(app: &App) {
    let screen = app.wait_text("Seq Scan");
    let mut hits = find_all(&screen, "Seq Scan");
    hits.sort_by_key(|hit| (hit.x, hit.y));
    assert!(!hits.is_empty(), "no Seq Scan row\n{}", text(&screen));
    app.click(hits[0]);
}

fn quit_unsaved(app: &App) {
    app.key("ctrl+q");
    let quit = app.wait_text("Quit TablePro?");
    see(&quit, &["unsaved", "Quit"], &["Query history"]);
    app.key("right");
    app.key("enter");
    let deadline = Instant::now() + Duration::from_secs(5);
    while app.child_running() {
        assert!(
            Instant::now() < deadline,
            "tablepro stayed up after confirming quit"
        );
        std::thread::sleep(Duration::from_millis(25));
    }
    app.expect_exited(0);
}

#[test]
fn tablepro_plan_route_reaches_query_1() {
    let app = open("t03-plan-route");
    let landing = app.screen();
    assert_eq!((landing.cols(), landing.rows()), (120, 40));
    see(&landing, &["Filter connections", "Development"], &[QUERY]);
    connect_development(&app);
    let work = app.screen();
    see(
        &work,
        &[QUERY, PLACEHOLDER, CONNECTED, "Development"],
        &["Loading schema", RAW_TITLE, "Execute query?"],
    );
    app.finish();
}

#[test]
fn tablepro_plan_raw_is_distinct_from_the_tree_and_analysis() {
    let app = open("t03-plan-bodies");
    connect_development(&app);
    type_statement(&app);

    app.key("ctrl+x");
    let tree = app.wait_screen("explain tree", tree_body);
    assert_eq!((tree.cols(), tree.rows()), (120, 40));
    see(
        &tree,
        &["Operation", "Seq Scan", PLANNING, SQL],
        &[
            RAW_TITLE,
            "Execution",
            "Esc cancels",
            "Gather",
            "Parallel",
            "Sort",
            "Execute query?",
        ],
    );

    focus_plan(&app);
    app.key("r");
    let raw = app.wait_screen("raw plan", raw_body);
    assert_eq!((raw.cols(), raw.rows()), (120, 40));
    see(
        &raw,
        &[RAW_TITLE, PLANNING_TIME, COST, SQL],
        &["Operation", "Execution", "productsr", "Execute query?"],
    );
    shot(&app, "T03.plan.plan.120x40");

    app.resize(80, 24);
    let narrow = app.wait_screen("raw plan at 80x24", |screen| {
        screen.cols() == 80 && screen.rows() == 24 && raw_narrow(screen)
    });
    assert_eq!((narrow.cols(), narrow.rows()), (80, 24));
    see(
        &narrow,
        &[RAW_TITLE, "Planning Time:", SQL],
        &["Operation", "productsr"],
    );
    shot(&app, "T03.plan.plan.narrow");

    app.resize(120, 40);
    app.wait_screen("raw plan back at 120x40", |screen| {
        screen.cols() == 120 && screen.rows() == 40 && raw_body(screen)
    });

    app.key("alt+x");
    // Analysis replaces the raw card. "Execution" is the distinct body;
    // the tree header "Operation" is not that proof.
    let analyzed = app.wait_screen("explain analyze", |screen| {
        let visible = text(screen);
        finished(screen)
            && visible.contains("Execution")
            && visible.contains(EXECUTION)
            && visible.contains(SQL)
            && !visible.contains("Execute query?")
    });
    see(
        &analyzed,
        &["Execution", EXECUTION, "0.466 ms", SQL, "EXPLAIN ANALYZE"],
        &["r Raw", "Execute query?", "Esc cancels", "productsr"],
    );
    quit_unsaved(&app);
}
