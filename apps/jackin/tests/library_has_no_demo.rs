use std::path::{Path, PathBuf};

const DEMO_DIRS: [&str; 3] = ["fixtures/", "sim/", "bin/"];

#[derive(Clone, Copy)]
enum Scope {
    OutsideDemo,
    Everywhere,
    OnlyIn(&'static [&'static str]),
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    Tokens,
    Literal,
    Alias,
}

struct Rule {
    text: &'static str,
    kind: Kind,
    scope: Scope,
}

const fn code(text: &'static str) -> Rule {
    Rule {
        text,
        kind: Kind::Tokens,
        scope: Scope::OutsideDemo,
    }
}

const fn literal(text: &'static str) -> Rule {
    Rule {
        text,
        kind: Kind::Literal,
        scope: Scope::OutsideDemo,
    }
}

/// A demo module named without a `::` after it: an alias, a re-export or a
/// `use` group. Named in a path or in a `use` item, and not followed by `::`
/// (the `code` rule of the same name reports that case).
const fn alias(text: &'static str) -> Rule {
    Rule {
        text,
        kind: Kind::Alias,
        scope: Scope::OutsideDemo,
    }
}

/// A refusal sentence. The cockpit and the feed types draw none of their own;
/// the words of a refusal sit in `LaunchWords`.
const fn refusal(text: &'static str) -> Rule {
    Rule {
        text,
        kind: Kind::Literal,
        scope: Scope::OnlyIn(&["screens/cockpit.rs", "launch.rs"]),
    }
}

const RULES: &[Rule] = &[
    code("sim ::"),
    code("fixtures ::"),
    alias("sim"),
    alias("fixtures"),
    code("Scenario"),
    code("AgentProcess"),
    code("ScriptPane"),
    code("SessionScript"),
    code("LaunchPlan"),
    code("refresh_fails"),
    code("takeover_at_ms"),
    code("last_seen_secs = w . clock . now_secs"),
    code("next_workspace_id"),
    code("CLOCK"),
    Rule {
        text: "feature = \"product\"",
        kind: Kind::Tokens,
        scope: Scope::Everywhere,
    },
    literal("roles-experimental"),
    literal("payments-platform"),
    literal("file:///Users/alexey/.jackin/logs/"),
    literal("0.6.4 · preview"),
    literal("About jackin-preview"),
    literal("in the preview"),
    literal("Not in this preview"),
    literal("Preview clipboard is empty"),
    literal("revealed in Finder"),
    literal("/.jackin/"),
    literal("Reap every pane in this tab"),
    literal("Reap the focused pane's agent"),
    literal("spawning"),
    literal("Pane cleared"),
    literal("Paste clipboard"),
    literal("opened on host"),
    literal("The clipboard is empty"),
    literal("fix the permission and Save again"),
    literal("3f9c"),
    literal("-4f11"),
    literal("tabs and panes restored"),
    Rule {
        text: "1Password locked: unlock the app and retry",
        kind: Kind::Literal,
        scope: Scope::OnlyIn(&["launch.rs", "screens/accounts.rs", "screens/cockpit.rs"]),
    },
    literal("tty004"),
    literal("EACCES"),
    literal("otlp://"),
    literal("Q1zx"),
    literal("typed once for this launch"),
    code("ticks as f64"),
    code("0 . 0 3 3"),
    refusal("Role not allowed"),
    refusal("does not allow role"),
    refusal("nothing was created"),
];

/// A token that still sits in the library, the file it sits in, how many times,
/// and the step that moves it. A moving step deletes its rows. A row whose
/// count is not the real count fails the scan: fewer is stale, more reports
/// the extra occurrences.
type Pending = (&'static str, &'static str, usize, &'static str);

const PENDING: &[Pending] = &[];

struct Source {
    path: String,
    text: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Lexeme {
    Ident(String),
    Punct(String),
    Str(String),
}

impl Lexeme {
    fn word(&self) -> String {
        match self {
            Lexeme::Ident(s) | Lexeme::Punct(s) => s.clone(),
            Lexeme::Str(s) => format!("\"{s}\""),
        }
    }
}

fn lex(text: &str) -> Vec<(usize, Lexeme)> {
    let chars: Vec<char> = text.chars().collect();
    let mut out = Vec::new();
    let mut line = 1;
    let mut i = 0;
    let at = |i: usize| chars.get(i).copied();
    while i < chars.len() {
        let c = chars[i];
        if c == '\n' {
            line += 1;
            i += 1;
        } else if c.is_whitespace() {
            i += 1;
        } else if c == '/' && at(i + 1) == Some('/') {
            while i < chars.len() && chars[i] != '\n' {
                i += 1;
            }
        } else if c == '/' && at(i + 1) == Some('*') {
            let mut depth = 1;
            i += 2;
            while i < chars.len() && depth > 0 {
                match (chars[i], at(i + 1)) {
                    ('/', Some('*')) => {
                        depth += 1;
                        i += 2;
                    }
                    ('*', Some('/')) => {
                        depth -= 1;
                        i += 2;
                    }
                    ('\n', _) => {
                        line += 1;
                        i += 1;
                    }
                    _ => i += 1,
                }
            }
        } else if let Some((hashes, start)) = raw_string_start(&chars, i) {
            let begin = line;
            let mut body = String::new();
            i = start;
            while i < chars.len() {
                if chars[i] == '"' && (0..hashes).all(|k| at(i + 1 + k) == Some('#')) {
                    i += 1 + hashes;
                    break;
                }
                if chars[i] == '\n' {
                    line += 1;
                }
                body.push(chars[i]);
                i += 1;
            }
            out.push((begin, Lexeme::Str(body)));
        } else if c == '"' || (c == 'b' && at(i + 1) == Some('"')) {
            let begin = line;
            i += if c == 'b' { 2 } else { 1 };
            let mut body = String::new();
            while i < chars.len() && chars[i] != '"' {
                if chars[i] == '\\' && at(i + 1) == Some('\n') {
                    line += 1;
                    i += 2;
                    while i < chars.len() && chars[i].is_whitespace() {
                        if chars[i] == '\n' {
                            line += 1;
                        }
                        i += 1;
                    }
                    continue;
                }
                if chars[i] == '\\' {
                    body.push(chars[i]);
                    i += 1;
                }
                if at(i) == Some('\n') {
                    line += 1;
                }
                if let Some(ch) = at(i) {
                    body.push(ch);
                }
                i += 1;
            }
            i += 1;
            out.push((begin, Lexeme::Str(body)));
        } else if c == '\'' {
            let is_char = at(i + 1) == Some('\\') || at(i + 2) == Some('\'');
            if is_char {
                i += 2;
                while i < chars.len() && chars[i] != '\'' {
                    i += 1;
                }
                i += 1;
            } else {
                i += 1;
            }
        } else if c.is_alphabetic() || c == '_' {
            let start = i;
            while i < chars.len() && (chars[i].is_alphanumeric() || chars[i] == '_') {
                i += 1;
            }
            out.push((line, Lexeme::Ident(chars[start..i].iter().collect())));
        } else if c == ':' && at(i + 1) == Some(':') {
            out.push((line, Lexeme::Punct("::".into())));
            i += 2;
        } else {
            out.push((line, Lexeme::Punct(c.to_string())));
            i += 1;
        }
    }
    join_concat(out)
}

fn join_concat(tokens: Vec<(usize, Lexeme)>) -> Vec<(usize, Lexeme)> {
    let is = |t: Option<&(usize, Lexeme)>, word: &str| t.is_some_and(|(_, l)| l.word() == word);
    let mut out = Vec::with_capacity(tokens.len());
    let mut i = 0;
    while i < tokens.len() {
        if is(tokens.get(i), "concat") && is(tokens.get(i + 1), "!") && is(tokens.get(i + 2), "(") {
            let mut j = i + 3;
            let mut joined = String::new();
            let mut closed = false;
            while let Some((_, lexeme)) = tokens.get(j) {
                match lexeme {
                    Lexeme::Str(s) => joined.push_str(s),
                    Lexeme::Punct(p) if p == "," => {}
                    Lexeme::Punct(p) if p == ")" => {
                        closed = true;
                        break;
                    }
                    _ => break,
                }
                j += 1;
            }
            if closed {
                out.push((tokens[i].0, Lexeme::Str(joined)));
                i = j + 1;
                continue;
            }
        }
        out.push(tokens[i].clone());
        i += 1;
    }
    out
}

fn raw_string_start(chars: &[char], i: usize) -> Option<(usize, usize)> {
    let mut j = i;
    if chars.get(j) == Some(&'b') {
        j += 1;
    }
    if chars.get(j) != Some(&'r') {
        return None;
    }
    j += 1;
    let hashes = chars[j..].iter().take_while(|c| **c == '#').count();
    (chars.get(j + hashes) == Some(&'"')).then_some((hashes, j + hashes + 1))
}

#[derive(Debug, PartialEq, Eq)]
struct Finding {
    token: &'static str,
    file: String,
    line: usize,
}

#[derive(Debug, Default, PartialEq, Eq)]
struct Report {
    unexpected: Vec<Finding>,
    stale: Vec<String>,
}

impl Report {
    fn is_clean(&self) -> bool {
        self.unexpected.is_empty() && self.stale.is_empty()
    }

    fn describe(&self) -> String {
        let mut lines: Vec<String> = self
            .unexpected
            .iter()
            .map(|f| format!("{}:{} names demo item `{}`", f.file, f.line, f.token))
            .collect();
        lines.extend(
            self.stale
                .iter()
                .map(|s| format!("stale PENDING entry: {s}")),
        );
        lines.join("\n")
    }
}

fn in_demo_dir(path: &str) -> bool {
    DEMO_DIRS.iter().any(|d| path.starts_with(d))
}

fn applies(rule: &Rule, path: &str) -> bool {
    match rule.scope {
        Scope::OutsideDemo => !in_demo_dir(path),
        Scope::Everywhere => true,
        Scope::OnlyIn(files) => files.contains(&path),
    }
}

fn matches_at(rule: &Rule, tokens: &[(usize, Lexeme)]) -> Vec<usize> {
    match rule.kind {
        Kind::Literal => tokens
            .iter()
            .filter_map(|(line, l)| match l {
                Lexeme::Str(s) if s.contains(rule.text) => Some(*line),
                _ => None,
            })
            .collect(),
        Kind::Tokens => {
            let words: Vec<&str> = rule.text.split(' ').collect();
            tokens
                .windows(words.len())
                .filter(|w| {
                    w.iter()
                        .map(|(_, l)| l.word())
                        .eq(words.iter().map(|s| s.to_string()))
                })
                .map(|w| w[0].0)
                .collect()
        }
        Kind::Alias => alias_lines(rule.text, tokens),
    }
}

fn alias_lines(name: &str, tokens: &[(usize, Lexeme)]) -> Vec<usize> {
    let is = |at: Option<&(usize, Lexeme)>, word: &str| at.is_some_and(|(_, l)| l.word() == word);
    let mut in_use = false;
    let mut lines = Vec::new();
    for (i, (line, lexeme)) in tokens.iter().enumerate() {
        match lexeme.word().as_str() {
            "use" => in_use = true,
            ";" => in_use = false,
            word if word == name => {
                let after = is(tokens.get(i + 1), "::");
                let before = i > 0 && is(tokens.get(i - 1), "::");
                if !after && (before || in_use) {
                    lines.push(*line);
                }
            }
            _ => {}
        }
    }
    lines
}

fn scan(sources: &[Source], pending: &[Pending]) -> Report {
    let mut report = Report::default();
    let mut seen = vec![0usize; pending.len()];
    for source in sources {
        let tokens = lex(&source.text);
        for rule in RULES.iter().filter(|r| applies(r, &source.path)) {
            let lines = matches_at(rule, &tokens);
            if lines.is_empty() {
                continue;
            }
            let slot = pending
                .iter()
                .position(|(t, f, ..)| *t == rule.text && *f == source.path);
            let allowed = slot.map_or(0, |k| pending[k].2);
            if let Some(k) = slot {
                seen[k] += lines.len();
            }
            report
                .unexpected
                .extend(lines.into_iter().skip(allowed).map(|line| Finding {
                    token: rule.text,
                    file: source.path.clone(),
                    line,
                }));
        }
    }
    for (k, (token, file, count, step)) in pending.iter().enumerate() {
        if seen[k] < *count {
            report.stale.push(format!(
                "({token:?}, {file:?}, {count}, {step:?}) matches {} time(s)",
                seen[k]
            ));
        }
    }
    report
}

fn gate_problems(lib_rs: &str) -> Vec<String> {
    let tokens = lex(lib_rs);
    let words: Vec<String> = tokens.iter().map(|(_, l)| l.word()).collect();
    let mut problems = Vec::new();
    for module in ["fixtures", "sim"] {
        let declaration = ["mod", module, ";"];
        let Some(at) = words.windows(3).position(|w| w == declaration) else {
            problems.push(format!("lib.rs does not declare `mod {module};`"));
            continue;
        };
        if !ends_with_gate(&words[..at]) {
            problems.push(format!(
                "`mod {module};` is not under #[cfg(feature = \"demo\")]"
            ));
        }
    }
    problems
}

fn ends_with_gate(words: &[String]) -> bool {
    let gate = ["#", "[", "cfg", "(", "feature", "=", "\"demo\"", ")", "]"];
    let mut end = words.len();
    if words[..end].last().map(String::as_str) == Some("pub") {
        end -= 1;
    }
    end >= gate.len() && words[end - gate.len()..end] == gate
}

fn read_tree(root: &Path) -> Vec<Source> {
    fn walk(dir: &Path, root: &Path, out: &mut Vec<Source>) {
        let mut entries: Vec<PathBuf> = std::fs::read_dir(dir)
            .expect("read dir")
            .map(|e| e.expect("entry").path())
            .collect();
        entries.sort();
        for path in entries {
            if path.is_dir() {
                walk(&path, root, out);
            } else if path.extension().is_some_and(|e| e == "rs") {
                let rel = path.strip_prefix(root).expect("under root");
                out.push(Source {
                    path: rel.to_string_lossy().replace('\\', "/"),
                    text: std::fs::read_to_string(&path).expect("read source"),
                });
            }
        }
    }
    let mut out = Vec::new();
    walk(root, root, &mut out);
    out
}

fn src(path: &str, text: &str) -> Source {
    Source {
        path: path.into(),
        text: text.into(),
    }
}

fn real_tree() -> Vec<Source> {
    read_tree(&Path::new(env!("CARGO_MANIFEST_DIR")).join("src"))
}

#[test]
fn real_tree_is_clean() {
    let report = scan(&real_tree(), PENDING);
    assert!(report.is_clean(), "{}", report.describe());
}

#[test]
fn demo_tokens_only_under_demo_modules() {
    let tree = [
        src(
            "sim/mod.rs",
            "use crate::fixtures::world_for; struct AgentProcess; const CLOCK: u8 = 0;",
        ),
        src(
            "fixtures/mod.rs",
            "pub enum Scenario { A } fn f() { crate::sim::run(); }",
        ),
        src(
            "bin/jackin-preview/main.rs",
            "use jackin_console::sim::SimBackend; // Scenario",
        ),
        src("screens/manager.rs", "fn draw() { let x = 1; }"),
    ];
    assert_eq!(scan(&tree, &[]), Report::default());
    let leaked = [src("screens/manager.rs", "use crate::sim::SimBackend;")];
    assert_eq!(
        scan(&leaked, &[]).unexpected,
        vec![Finding {
            token: "sim ::",
            file: "screens/manager.rs".into(),
            line: 1
        }]
    );
}

#[test]
fn the_script_player_lives_only_in_sim() {
    let player = "pub trait SessionScript {}\npub enum Step {}\npub struct AgentProcess;\npub struct ScriptPane;\nimpl PaneBody for ScriptPane {}";
    assert_eq!(scan(&[src("sim/pane.rs", player)], &[]), Report::default());
    for (token, text) in [
        ("ScriptPane", "pub struct ScriptPane;"),
        ("AgentProcess", "pub struct AgentProcess;"),
        ("SessionScript", "pub trait SessionScript {}"),
    ] {
        for path in ["panes.rs", "backend.rs", "screens/capsule.rs", "world.rs"] {
            let found = scan(&[src(path, text)], &[]).unexpected;
            assert_eq!(found.len(), 1, "{path}: {found:?}");
            assert_eq!(found[0].token, token, "{path}");
        }
    }
}

#[test]
fn word_boundaries() {
    let tree = [src(
        "world.rs",
        "const SYSTEM_CLOCK: u8 = 0; struct ScenarioLike; fn simulate() {} let fixtures_dir = 1; let sim_x = a::b;",
    )];
    assert_eq!(scan(&tree, &[]), Report::default());
    let tree = [src(
        "world.rs",
        "let c = CLOCK;\nlet s = Scenario::Returning;",
    )];
    let tokens: Vec<_> = scan(&tree, &[])
        .unexpected
        .iter()
        .map(|f| (f.token, f.line))
        .collect();
    assert_eq!(tokens, vec![("Scenario", 2), ("CLOCK", 1)]);
}

#[test]
fn test_cfg_demo_module_is_reported() {
    let tree = [src(
        "screens/x.rs",
        "#[cfg(all(test, feature = \"demo\"))]\nmod tests {\n    use crate::fixtures::world_for;\n}\n",
    )];
    let report = scan(&tree, &[]);
    assert_eq!(
        report.unexpected,
        vec![Finding {
            token: "fixtures ::",
            file: "screens/x.rs".into(),
            line: 3
        }]
    );
}

#[test]
fn stale_pending_entry_fails() {
    let tree = [src("world.rs", "fn clean() {}")];
    let pending = [("CLOCK", "world.rs", 1, "B2c")];
    let report = scan(&tree, &pending);
    assert!(report.unexpected.is_empty());
    assert_eq!(
        report.stale,
        vec![r#"("CLOCK", "world.rs", 1, "B2c") matches 0 time(s)"#.to_owned()]
    );
    let tree = [src("world.rs", "let c = CLOCK;")];
    assert!(scan(&tree, &pending).is_clean());
}

#[test]
fn a_pending_row_counts_occurrences() {
    let pending = [("payments-platform", "screens/capsule.rs", 1, "B3b")];
    let one = [src(
        "screens/capsule.rs",
        "const A: &str = \"payments-platform\";",
    )];
    assert!(scan(&one, &pending).is_clean());
    let two = [src(
        "screens/capsule.rs",
        "const A: &str = \"payments-platform\";\nconst B: &str = \"payments-platform\";",
    )];
    assert_eq!(
        scan(&two, &pending).unexpected,
        vec![Finding {
            token: "payments-platform",
            file: "screens/capsule.rs".into(),
            line: 2
        }]
    );
    let two_rows = [("payments-platform", "screens/capsule.rs", 2, "B3b")];
    assert!(scan(&two, &two_rows).is_clean());
    assert_eq!(scan(&one, &two_rows).stale.len(), 1);
}

#[test]
fn demo_modules_are_not_reachable_through_an_alias() {
    for (text, name) in [
        ("pub use crate::sim as simulated;", "sim"),
        ("use crate::sim;", "sim"),
        ("use crate::{fixtures, world};", "fixtures"),
        ("use crate::{world, sim as s};", "sim"),
        ("use jackin_console::fixtures as demo;", "fixtures"),
    ] {
        let found = scan(&[src("domain/workspace.rs", text)], &[]).unexpected;
        assert_eq!(found.len(), 1, "{text}: {found:?}");
        assert_eq!(found[0].token, name, "{text}");
    }
    let quiet = [src(
        "domain/workspace.rs",
        "use crate::world::World;\nfn f(sim: u8) -> u8 { sim }\nlet fixtures_dir = a::b;",
    )];
    assert_eq!(scan(&quiet, &[]), Report::default());
    let demo_side = [src("sim/mod.rs", "pub use crate::fixtures as f;")];
    assert_eq!(scan(&demo_side, &[]), Report::default());
}

#[test]
fn demo_literal_in_domain_fails() {
    for literal in ["payments-platform", "roles-experimental", "Paste clipboard"] {
        let text = format!("fn f() -> &'static str {{ \"the {literal} row\" }}");
        let tree = [src("domain/workspace.rs", &text)];
        let found = scan(&tree, &[]).unexpected;
        assert_eq!(found.len(), 1, "{literal}: {found:?}");
        assert_eq!(found[0].token, literal);
    }
    let commented = [src(
        "domain/x.rs",
        "// payments-platform\n/* roles-experimental */ fn f() {}",
    )];
    assert_eq!(scan(&commented, &[]), Report::default());
    let sentence = "const A: &str = \"1Password locked: unlock the app and retry\";";
    let scoped = [
        src("screens/accounts.rs", sentence),
        src("launch.rs", sentence),
        src("backend.rs", sentence),
    ];
    let found = scan(&scoped, &[]).unexpected;
    let files: Vec<&str> = found.iter().map(|f| f.file.as_str()).collect();
    assert_eq!(files, ["screens/accounts.rs", "launch.rs"]);
}

#[test]
fn the_locked_sentence_is_scanned_in_the_cockpit() {
    let sentence = "const A: &str = \"1Password locked: unlock the app and retry\";";
    let found = scan(&[src("screens/cockpit.rs", sentence)], &[]).unexpected;
    assert_eq!(found.len(), 1, "{found:?}");
    assert_eq!(found[0].token, "1Password locked: unlock the app and retry");
}

#[test]
fn the_cockpit_invents_no_telemetry_key_or_timing() {
    for (token, text) in [
        (
            "otlp://",
            "fn f() -> String { format!(\"run {} -> otlp://collector.internal:4317\", id) }",
        ),
        (
            "Q1zx",
            "fn f() -> String { crate::domain::account::masked(\"Q1zx\") }",
        ),
        (
            "typed once for this launch",
            "fn f() { let v = \"typed once for this launch · never stored\"; }",
        ),
        (
            "ticks as f64",
            "fn f(s: &S) -> f64 { s.stages.get(1).ticks as f64 * 2.0 }",
        ),
        ("0 . 0 3 3", "fn f(t: u64) -> f64 { t as f64 * 0.033 }"),
    ] {
        for path in ["screens/cockpit.rs", "launch.rs", "domain/launch.rs"] {
            let found = scan(&[src(path, text)], &[]).unexpected;
            assert_eq!(found.len(), 1, "{token} in {path}: {found:?}");
            assert_eq!(found[0].token, token, "{path}");
        }
        for path in ["sim/launch.rs", "fixtures/mod.rs"] {
            assert_eq!(scan(&[src(path, text)], &[]), Report::default(), "{path}");
        }
    }
}

#[test]
fn the_cockpit_draws_no_refusal_of_its_own() {
    for (token, text) in [
        (
            "Role not allowed",
            "fn f() -> &'static str { \"Role not allowed\" }",
        ),
        (
            "does not allow role",
            "fn f(w: &str, r: &str) -> String { format!(\"Workspace {w} does not allow role {r}.\") }",
        ),
        (
            "nothing was created",
            "const STATE: &str = \"nothing was created\";",
        ),
    ] {
        for path in ["screens/cockpit.rs", "launch.rs"] {
            let found = scan(&[src(path, text)], &[]).unexpected;
            assert_eq!(found.len(), 1, "{token} in {path}: {found:?}");
            assert_eq!(found[0].token, token, "{path}");
        }
        for path in [
            "domain/launch.rs",
            "sim/launch.rs",
            "sim/mod.rs",
            "fixtures/surfaces.rs",
        ] {
            assert_eq!(scan(&[src(path, text)], &[]), Report::default(), "{path}");
        }
    }
}

#[test]
fn product_cfg_is_rejected() {
    let text = "#[cfg(feature = \"product\")]\nfn f() {}";
    for path in [
        "screens/x.rs",
        "fixtures/mod.rs",
        "bin/jackin-preview/main.rs",
    ] {
        let found = scan(&[src(path, text)], &[]).unexpected;
        assert_eq!(found.len(), 1, "{path}: {found:?}");
        assert_eq!(found[0].token, "feature = \"product\"");
    }
}

#[test]
fn demo_modules_are_cfg_gated() {
    let lib_rs = std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("src/lib.rs"))
        .expect("read lib.rs");
    assert_eq!(gate_problems(&lib_rs), Vec::<String>::new());
}

#[test]
fn an_ungated_demo_module_is_reported() {
    let gated =
        "#[cfg(feature = \"demo\")]\npub mod fixtures;\n#[cfg(feature = \"demo\")]\npub mod sim;\n";
    assert!(gate_problems(gated).is_empty());
    let sim_open = "#[cfg(feature = \"demo\")]\npub mod fixtures;\npub mod sim;\n";
    assert_eq!(gate_problems(sim_open).len(), 1);
    assert!(gate_problems(sim_open)[0].contains("`mod sim;`"));
    let fixtures_open = "pub mod fixtures;\n#[cfg(feature = \"demo\")]\npub mod sim;\n";
    assert!(gate_problems(fixtures_open)[0].contains("`mod fixtures;`"));
    let wrong_feature = "#[cfg(feature = \"fixtures\")]\npub mod fixtures;\n#[cfg(feature = \"demo\")]\npub mod sim;\n";
    assert_eq!(gate_problems(wrong_feature).len(), 1);
    let missing = "#[cfg(feature = \"demo\")]\npub mod fixtures;\n";
    assert!(gate_problems(missing)[0].contains("does not declare `mod sim;`"));
}

#[test]
fn a_demo_literal_split_over_concat_or_a_line_continuation_is_still_found() {
    for (name, text) in [
        (
            "concat",
            "const A: &str = concat!(\"the payments\", \"-platform row\");",
        ),
        (
            "concat over lines",
            "const A: &str = concat!(\n    \"the payments-\",\n    \"platform row\",\n);",
        ),
        (
            "continuation",
            "const A: &str = \"the payments-\\\n        platform row\";",
        ),
    ] {
        let found = scan(&[src("domain/workspace.rs", text)], &[]).unexpected;
        assert_eq!(found.len(), 1, "{name}: {found:?}");
        assert_eq!(found[0].token, "payments-platform", "{name}");
    }
    let apart = [src(
        "domain/workspace.rs",
        "const A: &str = concat!(\"the payments\", other(), \"-platform row\");",
    )];
    assert_eq!(scan(&apart, &[]), Report::default());
}
