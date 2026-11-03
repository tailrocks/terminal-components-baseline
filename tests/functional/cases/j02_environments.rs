//! Jackin workspace environments, driven through the real binary.
//!
//! Tuiscotti test ids are `[a-z0-9_-]` segments, so the baseline directory is
//! `pre_refactor/j02_environments/`. The shots are the open New workspace
//! environment key form after the key and a masked value are typed, before
//! Save. A same-size resize does not force a frame, and the cursor column is
//! part of the hash, so each shot nudges the width and comes back.

use std::time::Duration;

use tui_test::TerminalSnapshot;

use crate::support::{App, JACKIN, Launch, Point, find_all, row_text, text, try_launch};

const DIRECTORY: &str = "Current directory";
const PAYMENTS: &str = "payments-platform";
const NAME_HELP: &str = "Directory basename by default";
const FIXTURE_SECRET: &str = "pw-fixture-only";
const DUP_SECRET: &str = "duplicate-secret-value";
const TOKEN_SECRET: &str = "field-token-secret-91";
const TOKEN_MASK: &str = "************t-91";
const DATABASE: &str = "DATABASE_URL";
const STRIPE: &str = "STRIPE_KEY";
const LOG_LEVEL: &str = "LOG_LEVEL";
const GH_TOKEN: &str = "GH_TOKEN";
const FIELD_TOKEN: &str = "FIELD_TOKEN";
const NEW_ENV: &str = "New workspace environment key";
const EDIT_LOG: &str = "Edit LOG_LEVEL";
const DUP_ERROR: &str = "DATABASE_URL already exists in this scope";
const ADDED: &str = "FIELD_TOKEN · added · save to apply";
const UPDATED: &str = "LOG_LEVEL · updated · save to apply";
const MOVED: &str = "FIELD_TOKEN moved to role backend";
const REMOVED: &str = "Removed LOG_LEVEL · u restores until save";
const DELETE_TITLE: &str = "Delete environment variable LOG_LEVEL?";
const SCOPE_TITLE: &str = "Scope for FIELD_TOKEN";
const PREVIEW_TITLE: &str = "Save workspace";
const CLEANUP: &str = "Clean up & save…";
const SAVED: &str = "Workspace payments-platform saved";
const UNSAVED: &str = "Unsaved changes";
const CANCEL: &str = "Cancel";
const CONFIRM: &str = "Save";
const DELETE: &str = "Delete";
const WIDE: &str = "J02.environments.env.120x40";
const NARROW: &str = "J02.environments.env.narrow";

fn visual_id(checkpoint: &str) -> String {
    format!(
        "pre_refactor/j02_environments/{}",
        checkpoint.replace('.', "_").to_ascii_lowercase()
    )
}

fn open(name: &str) -> App {
    let mut launch = Launch::new(name, JACKIN);
    launch.args = vec!["--scenario".to_string(), "returning".to_string()];
    launch.cols = 120;
    launch.rows = 40;
    launch.color = Some("truecolor".to_string());
    launch.motion = Some("reduced".to_string());
    // `Launch::new` leaves the clock off. `App::new` would enable it for jackin.
    launch.clock = false;
    try_launch(launch).unwrap_or_else(|error| panic!("{error}"))
}

fn reach_manager(app: &App) -> TerminalSnapshot {
    let screen = app.wait_screen("manager or intro", |screen| {
        let visible = text(screen);
        visible.contains(DIRECTORY) || visible.contains("Stand up, operator")
    });
    if text(&screen).contains("Stand up, operator") {
        app.key("space");
        return app.wait_text(DIRECTORY);
    }
    screen
}

fn focused_line(screen: &TerminalSnapshot) -> String {
    (0..screen.rows())
        .map(|row| row_text(screen, row))
        .find(|line| line.contains('▎'))
        .unwrap_or_default()
}

fn require(screen: &TerminalSnapshot, needle: &str) {
    let visible = text(screen);
    if !visible.contains(needle) {
        panic!("missing {needle:?}\n{visible}");
    }
}

fn forbid(screen: &TerminalSnapshot, needle: &str) {
    let visible = text(screen);
    if visible.contains(needle) {
        panic!("forbidden {needle:?}\n{visible}");
    }
}

fn key_token(line: &str, key: &str) -> bool {
    line.match_indices(key).any(|(at, _)| {
        let prev_ok = line[..at]
            .chars()
            .next_back()
            .is_none_or(|c| !c.is_ascii_alphanumeric() && c != '_');
        let rest = &line[at + key.len()..];
        prev_ok && !rest.starts_with(|c: char| c.is_ascii_alphanumeric() || c == '_')
    })
}

fn key_lines(screen: &TerminalSnapshot, key: &str) -> Vec<String> {
    text(screen)
        .lines()
        .filter(|line| key_token(line, key))
        .map(str::to_string)
        .collect()
}

fn only_line(screen: &TerminalSnapshot, key: &str) -> String {
    let lines = key_lines(screen, key);
    assert_eq!(lines.len(), 1, "{key} lines:\n{}", text(screen));
    lines[0].clone()
}

fn star_count(line: &str) -> usize {
    line.chars().filter(|c| *c == '*').count()
}

fn bullet_count(line: &str) -> usize {
    line.chars().filter(|c| *c == '•').count()
}

fn secrets_hidden(screen: &TerminalSnapshot) {
    forbid(screen, FIXTURE_SECRET);
    forbid(screen, DUP_SECRET);
    forbid(screen, TOKEN_SECRET);
}

fn select_payments(app: &App) -> TerminalSnapshot {
    let screen = app.screen();
    assert!(
        focused_line(&screen).contains(DIRECTORY),
        "manager did not start on Current directory\n{}",
        text(&screen)
    );
    app.key("down");
    app.wait_screen("payments-platform", |screen| {
        focused_line(screen).contains(PAYMENTS)
    })
}

fn open_editor(app: &App) -> TerminalSnapshot {
    let screen = app.screen();
    assert!(
        focused_line(&screen).contains(PAYMENTS),
        "refusing e unless payments-platform is focused\n{}",
        text(&screen)
    );
    app.key("e");
    app.wait_screen("editor", |screen| text(screen).contains(NAME_HELP))
}

fn open_environments(app: &App) -> TerminalSnapshot {
    let before = app.screen();
    assert!(
        text(&before).contains(NAME_HELP) && !text(&before).contains("Add environment variable"),
        "editor did not open on General\n{}",
        text(&before)
    );
    app.key("]");
    app.wait_screen("mounts tab", |screen| text(screen).contains("Add mount"));
    app.key("]");
    app.wait_screen("roles tab", |screen| {
        let visible = text(screen);
        visible.contains("the-architect") && !visible.contains("Add mount")
    });
    app.key("]");
    // A saved workspace no longer has LOG_LEVEL, so the tab wait cannot require it.
    app.wait_screen("environments tab", |screen| {
        text(screen).contains("Add environment variable")
    })
}

fn semantic_rows(screen: &TerminalSnapshot) {
    let visible = text(screen);
    assert!(
        visible.contains("Add environment variable"),
        "environments list\n{visible}"
    );
    forbid(screen, FIXTURE_SECRET);
    let database = only_line(screen, DATABASE);
    assert_eq!(star_count(&database), 16, "DATABASE_URL mask\n{database}");
    let stripe = only_line(screen, STRIPE);
    assert!(
        stripe.contains("[op]") && star_count(&stripe) == 16,
        "STRIPE_KEY mask\n{stripe}"
    );
    let log_level = only_line(screen, LOG_LEVEL);
    assert!(
        star_count(&log_level) == 5 && log_level.contains("plain"),
        "LOG_LEVEL mask\n{log_level}"
    );
    let token = only_line(screen, GH_TOKEN);
    assert!(token.contains("$GH_TOKEN"), "GH_TOKEN host mask\n{token}");
}

fn tab_strip_focused(screen: &TerminalSnapshot) -> bool {
    let line = focused_line(screen);
    line.contains("General") && line.contains("Environments") && !line.contains(DATABASE)
}

fn focus_list(app: &App) {
    if !tab_strip_focused(&app.screen()) {
        return;
    }
    app.key("enter");
    app.wait_screen("environments list", |screen| {
        let line = focused_line(screen);
        line.contains("Workspace") || key_token(&line, DATABASE) || line.contains("Add environment")
    });
}

fn step_until(app: &App, key: &str, marker: &str) {
    for _ in 0..16 {
        if key_token(&focused_line(&app.screen()), marker) {
            return;
        }
        let before = focused_line(&app.screen());
        app.key(key);
        let deadline = std::time::Instant::now() + Duration::from_millis(800);
        loop {
            let line = focused_line(&app.screen());
            if key_token(&line, marker) || line != before || std::time::Instant::now() >= deadline {
                break;
            }
            std::thread::sleep(Duration::from_millis(25));
        }
        if key_token(&focused_line(&app.screen()), marker) {
            return;
        }
        if focused_line(&app.screen()) == before {
            break;
        }
    }
    let screen = app.screen();
    panic!("never focused {marker} with {key}\n{}", text(&screen));
}

fn focus_key(app: &App, key: &str) {
    focus_list(app);
    if key_token(&focused_line(&app.screen()), key) {
        return;
    }
    // `g` lands on the workspace header, and the next paint skips that header
    // (`config.rs` `build_rows`), so the stable top row is DATABASE_URL.
    app.key("g");
    app.wait_screen("list top", |screen| {
        let line = focused_line(screen);
        key_token(&line, DATABASE) || key_token(&line, key)
    });
    if key_token(&focused_line(&app.screen()), key) {
        return;
    }
    step_until(app, "down", key);
}

fn button(app: &App, label: &str, row_ok: impl Fn(&str) -> bool) -> Point {
    let screen = app.screen();
    let hits: Vec<_> = find_all(&screen, label)
        .into_iter()
        .filter(|point| row_ok(&row_text(&screen, u32::from(point.y))))
        .collect();
    assert_eq!(hits.len(), 1, "one button {label:?}:\n{}", text(&screen));
    Point {
        x: hits[0].x + 2,
        y: hits[0].y,
    }
}

fn form_save(app: &App) -> Point {
    button(app, CONFIRM, |row| {
        row.contains(CANCEL)
            && row.contains(CONFIRM)
            && !row.contains("Save…")
            && !row.contains(CLEANUP)
            && !row.contains(PREVIEW_TITLE)
    })
}

fn preview_save(app: &App) -> Point {
    button(app, CONFIRM, |row| {
        row.contains(CANCEL)
            && row.contains(CONFIRM)
            && row.contains('│')
            && !row.contains("Save…")
            && !row.contains(CLEANUP)
            && !row.contains(UNSAVED)
    })
}

fn delete_button(app: &App) -> Point {
    button(app, DELETE, |row| {
        row.contains(CANCEL) && row.contains(DELETE) && !row.contains(DELETE_TITLE)
    })
}

fn open_new_env(app: &App) -> TerminalSnapshot {
    focus_list(app);
    // The workspace header cannot hold the cursor. `a` on a workspace env row
    // (`RowKey::Env(None, _)` in `config.rs`) opens the workspace-scoped form.
    let before = focused_line(&app.screen());
    assert!(
        key_token(&before, DATABASE),
        "refusing a unless the workspace section row is current\n{before}"
    );
    app.key("a");
    // The dialog title paints before the kind control.
    let form = app.wait_screen("new env", |screen| {
        let visible = text(screen);
        visible.contains(NEW_ENV) && visible.contains("Plain text")
    });
    require(&form, "Plain text");
    forbid(&form, DUP_SECRET);
    forbid(&form, FIXTURE_SECRET);
    form
}

fn type_masked(app: &App, key_name: &str, value: &str) {
    app.type_text(key_name);
    app.wait_screen("key text", |screen| {
        let line = focused_line(screen);
        line.contains(key_name) && line.matches(key_name).count() == 1
    });
    app.key("tab");
    app.key("tab");
    app.wait_screen("value field", |screen| {
        let line = focused_line(screen);
        !line.contains(key_name) && !line.contains("Plain text")
    });
    app.type_text(value);
    let typed = app.screen();
    if text(&typed).contains(value) || focused_line(&typed).contains(value) {
        panic!(
            "focused field paints the plaintext secret\n{}",
            text(&typed)
        );
    }
    app.wait_screen("masked value", |screen| {
        bullet_count(&focused_line(screen)) == value.chars().count()
            && !text(screen).contains(value)
            && !text(screen).contains(FIXTURE_SECRET)
    });
}

fn prepared_form(screen: &TerminalSnapshot) -> bool {
    let visible = text(screen);
    let line = focused_line(screen);
    visible.contains(NEW_ENV)
        && visible.contains(DATABASE)
        && bullet_count(&line) == DUP_SECRET.chars().count()
        && !visible.contains(DUP_SECRET)
        && !visible.contains(FIXTURE_SECRET)
        && !visible.contains(DUP_ERROR)
        && !visible.contains(ADDED)
        && !visible.contains(SAVED)
        && !visible.contains(CLEANUP)
}

/// A same-size resize does not force a frame, and the cursor column is part
/// of the hash. Nudge the width and come back while the form stays open.
fn nudge(app: &App, quiet_frame: impl Fn(&TerminalSnapshot) -> bool) {
    std::thread::sleep(Duration::from_millis(200));
    let screen = app.screen();
    let cols = u16::try_from(screen.cols()).expect("cols");
    let rows = u16::try_from(screen.rows()).expect("rows");
    assert!(
        quiet_frame(&screen),
        "nudge left the frame\n{}",
        text(&screen)
    );
    let wider = cols + 1;
    app.resize(wider, rows);
    app.wait_screen("nudged", |screen| {
        screen.cols() == u32::from(wider) && screen.rows() == u32::from(rows) && quiet_frame(screen)
    });
    app.resize(cols, rows);
    app.wait_screen("size restored", |screen| {
        screen.cols() == u32::from(cols) && screen.rows() == u32::from(rows) && quiet_frame(screen)
    });
}

fn shoot(app: &App, checkpoint: &str) {
    nudge(app, prepared_form);
    app.visual(&visual_id(checkpoint));
}

fn form_closed(screen: &TerminalSnapshot) -> bool {
    let visible = text(screen);
    // The list hint is "Enter Edit", so a bare "Edit " match stays true on the list.
    !visible.contains(NEW_ENV) && !visible.contains(EDIT_LOG)
}

fn has_value_row(screen: &TerminalSnapshot, key: &str) -> bool {
    key_lines(screen, key)
        .iter()
        .filter(|line| line.contains('*') || line.contains('$') || line.contains("[op]"))
        .count()
        == 1
}

fn value_row(screen: &TerminalSnapshot, key: &str) -> String {
    let lines: Vec<_> = key_lines(screen, key)
        .into_iter()
        .filter(|line| line.contains('*') || line.contains('$') || line.contains("[op]"))
        .collect();
    assert_eq!(lines.len(), 1, "{key} value rows:\n{}", text(screen));
    lines[0].clone()
}

fn section_start(line: &str, header: &str) -> bool {
    if header == "Workspace" {
        line.contains("Workspace") && !line.contains("Workspaces") && line.contains("var")
    } else {
        line.contains(header)
    }
}

fn section_body(screen: &TerminalSnapshot, header: &str, next: &str) -> String {
    let visible = text(screen);
    let lines: Vec<&str> = visible.lines().collect();
    let start = lines
        .iter()
        .position(|line| section_start(line, header))
        .unwrap_or_else(|| panic!("missing section {header}\n{visible}"));
    let end = lines
        .iter()
        .skip(start + 1)
        .position(|line| line.contains(next))
        .map(|index| start + 1 + index)
        .unwrap_or(lines.len());
    lines[start..end].join("\n")
}

fn submit_form(app: &App) {
    app.click(form_save(app));
}

#[test]
fn j02_environments_route() {
    let app = open("j02-environments-route");
    let manager = reach_manager(&app);
    require(&manager, DIRECTORY);
    assert_eq!((manager.cols(), manager.rows()), (120, 40));
    let selected = select_payments(&app);
    assert!(
        focused_line(&selected).contains(PAYMENTS),
        "{}",
        text(&selected)
    );
    let editor = open_editor(&app);
    require(&editor, NAME_HELP);
    let environments = open_environments(&app);
    semantic_rows(&environments);
    assert!(
        text(&environments).contains(PAYMENTS),
        "workspace name\n{}",
        text(&environments)
    );
    app.key("escape");
    let back = app.wait_screen("clean leave", |screen| {
        text(screen).contains(DIRECTORY)
            && !text(screen).contains(NAME_HELP)
            && !text(screen).contains(UNSAVED)
            && !text(screen).contains("Add environment variable")
    });
    assert!(!text(&back).contains(SAVED), "{}", text(&back));
    assert!(!text(&back).contains("save to apply"), "{}", text(&back));
    app.finish();
}

#[test]
fn j02_environments_env() {
    let app = open("j02-environments-env");
    let manager = reach_manager(&app);
    require(&manager, DIRECTORY);
    assert_eq!((manager.cols(), manager.rows()), (120, 40));
    select_payments(&app);
    open_editor(&app);
    let environments = open_environments(&app);
    semantic_rows(&environments);

    open_new_env(&app);
    type_masked(&app, DATABASE, DUP_SECRET);
    let wide = app.wait_screen("duplicate form", prepared_form);
    assert_eq!((wide.cols(), wide.rows()), (120, 40));
    if text(&wide).contains(DUP_SECRET) || focused_line(&wide).contains(DUP_SECRET) {
        panic!("focused field paints the plaintext secret\n{}", text(&wide));
    }
    shoot(&app, WIDE);
    app.resize(80, 24);
    let narrow = app.wait_screen("80x24", |screen| {
        screen.cols() == 80 && screen.rows() == 24 && prepared_form(screen)
    });
    assert_eq!((narrow.cols(), narrow.rows()), (80, 24));
    if text(&narrow).contains(DUP_SECRET) {
        panic!(
            "focused field paints the plaintext secret\n{}",
            text(&narrow)
        );
    }
    shoot(&app, NARROW);
    app.resize(120, 40);
    app.wait_screen("120x40 again", |screen| {
        screen.cols() == 120 && screen.rows() == 40 && prepared_form(screen)
    });

    submit_form(&app);
    let dropped = app.wait_screen("duplicate dropped", |screen| {
        form_closed(screen)
            && !text(screen).contains(DUP_ERROR)
            && key_lines(screen, DATABASE).len() == 1
    });
    secrets_hidden(&dropped);
    assert!(
        !text(&dropped).contains(ADDED) && !text(&dropped).contains(NEW_ENV),
        "duplicate stored a row\n{}",
        text(&dropped)
    );
    assert_eq!(star_count(&only_line(&dropped, DATABASE)), 16);

    open_new_env(&app);
    type_masked(&app, FIELD_TOKEN, TOKEN_SECRET);
    submit_form(&app);
    let added = app.wait_screen("field token added", |screen| {
        text(screen).contains(ADDED) && form_closed(screen) && has_value_row(screen, FIELD_TOKEN)
    });
    secrets_hidden(&added);
    let field = value_row(&added, FIELD_TOKEN);
    assert!(field.contains(TOKEN_MASK), "FIELD_TOKEN mask\n{field}");

    focus_key(&app, LOG_LEVEL);
    app.key("enter");
    let editing = app.wait_screen("edit log level", |screen| text(screen).contains(EDIT_LOG));
    require(&editing, LOG_LEVEL);
    forbid(&editing, TOKEN_SECRET);
    app.key("tab");
    app.key("tab");
    app.wait_screen("log value", |screen| {
        let line = focused_line(screen);
        !line.contains("Plain text") && !key_token(&line, LOG_LEVEL)
    });
    app.key("ctrl+l");
    app.key("backspace");
    app.type_text("info");
    app.wait_screen("info masked", |screen| {
        bullet_count(&focused_line(screen)) == 4 && !text(screen).contains("debug")
    });
    submit_form(&app);
    let updated = app.wait_screen("log updated", |screen| {
        text(screen).contains(UPDATED) && form_closed(screen)
    });
    secrets_hidden(&updated);
    let log_level = value_row(&updated, LOG_LEVEL);
    assert_eq!(star_count(&log_level), 4, "updated LOG_LEVEL\n{log_level}");
    assert!(!log_level.contains("info"), "{log_level}");

    focus_key(&app, FIELD_TOKEN);
    app.key("s");
    let scope = app.wait_screen("scope dialog", |screen| text(screen).contains(SCOPE_TITLE));
    require(&scope, "role backend");
    app.key("down");
    app.key("down");
    app.wait_screen("role backend selected", |screen| {
        text(screen)
            .lines()
            .any(|line| line.contains("role backend") && line.contains("(●)"))
    });
    app.key("enter");
    let moved = app.wait_screen("moved", |screen| text(screen).contains(MOVED));
    secrets_hidden(&moved);
    let workspace = section_body(&moved, "Workspace", "Role:");
    assert!(
        !workspace.contains(FIELD_TOKEN),
        "workspace still lists FIELD_TOKEN\n{workspace}"
    );
    let backend = section_body(&moved, "Role: backend", "Add role override");
    assert!(
        backend.contains(FIELD_TOKEN) && backend.contains(TOKEN_MASK),
        "role backend row\n{backend}"
    );

    focus_key(&app, LOG_LEVEL);
    app.key("d");
    let confirm = app.wait_screen("delete dialog", |screen| {
        text(screen).contains(DELETE_TITLE) && focused_line(screen).contains(CANCEL)
    });
    assert!(
        focused_line(&confirm).contains(CANCEL),
        "destructive dialog focus\n{}",
        focused_line(&confirm)
    );
    app.click(delete_button(&app));
    let removed = app.wait_screen("removed", |screen| text(screen).contains(REMOVED));
    secrets_hidden(&removed);
    let log_rows: Vec<_> = key_lines(&removed, LOG_LEVEL)
        .into_iter()
        .filter(|line| !line.contains(REMOVED))
        .collect();
    assert!(
        !log_rows.is_empty() && log_rows.iter().all(|line| line.contains("plain text")),
        "LOG_LEVEL stayed active\n{}",
        text(&removed)
    );

    app.key("ctrl+s");
    let preview = app.wait_screen("save preview", |screen| {
        let visible = text(screen);
        visible.contains(PREVIEW_TITLE)
            && focused_line(screen).contains(CANCEL)
            && !visible.contains(SAVED)
            && !visible.contains(CLEANUP)
    });
    secrets_hidden(&preview);
    app.click(preview_save(&app));
    let saved = app.wait_screen("saved", |screen| {
        text(screen).contains(SAVED) && text(screen).contains(DIRECTORY)
    });
    secrets_hidden(&saved);
    assert!(!text(&saved).contains(NEW_ENV), "{}", text(&saved));

    if !focused_line(&saved).contains(PAYMENTS) {
        select_payments(&app);
    }
    open_editor(&app);
    let again = open_environments(&app);
    secrets_hidden(&again);
    assert!(
        text(&again).contains(PAYMENTS),
        "workspace name after save\n{}",
        text(&again)
    );
    assert_eq!(key_lines(&again, DATABASE).len(), 1);
    assert_eq!(star_count(&only_line(&again, DATABASE)), 16);
    assert!(only_line(&again, STRIPE).contains("[op]"));
    assert!(only_line(&again, GH_TOKEN).contains("$GH_TOKEN"));
    assert!(
        !text(&again).contains(LOG_LEVEL),
        "LOG_LEVEL returned\n{}",
        text(&again)
    );
    let workspace = section_body(&again, "Workspace", "Role:");
    assert!(
        !workspace.contains(FIELD_TOKEN),
        "saved workspace lists FIELD_TOKEN\n{workspace}"
    );
    let backend = section_body(&again, "Role: backend", "Add role override");
    assert!(
        backend.contains(FIELD_TOKEN) && backend.contains(TOKEN_MASK),
        "saved role row\n{backend}"
    );
    app.finish();
}
