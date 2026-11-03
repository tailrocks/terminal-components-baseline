use std::path::{Path, PathBuf};

const DEMO_DIRS: [&str; 3] = ["fixtures/", "sim/", "bin/"];

#[derive(Clone, Copy)]
enum Scope {
    OutsideDemo,
    Everywhere,
    OnlyIn(&'static str),
}

struct Rule {
    text: &'static str,
    literal: bool,
    scope: Scope,
}

const fn code(text: &'static str) -> Rule {
    Rule {
        text,
        literal: false,
        scope: Scope::OutsideDemo,
    }
}

const fn literal(text: &'static str) -> Rule {
    Rule {
        text,
        literal: true,
        scope: Scope::OutsideDemo,
    }
}

const RULES: &[Rule] = &[
    code("sim ::"),
    code("fixtures ::"),
    code("Scenario"),
    code("AgentProcess"),
    code("SessionScript"),
    code("LaunchPlan"),
    code("refresh_fails"),
    code("takeover_at_ms"),
    code("last_seen_secs = w . clock . now_secs"),
    code("next_workspace_id"),
    code("CLOCK"),
    Rule {
        text: "feature = \"product\"",
        literal: false,
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
    literal("Close tab?"),
    literal("Close pane?"),
    literal("Reap every pane in this tab"),
    literal("Reap the focused pane's agent"),
    literal("spawning"),
    literal("Pane cleared"),
    Rule {
        text: "1Password locked: unlock the app and retry",
        literal: true,
        scope: Scope::OnlyIn("launch.rs"),
    },
    literal("tty004"),
    literal("EACCES"),
];

/// A token that still sits in the library, the file it sits in and the step that moves it.
/// A moving step deletes its rows; a row that no longer matches fails the scan.
type Pending = (&'static str, &'static str, &'static str);

const PENDING: &[Pending] = &[
    ("About jackin-preview", "app.rs", "B3b"),
    ("/.jackin/", "screens/capsule.rs", "B3b"),
    ("About jackin-preview", "screens/capsule.rs", "B3b"),
    ("Close pane?", "screens/capsule.rs", "B3b"),
    ("Close tab?", "screens/capsule.rs", "B3b"),
    ("Not in this preview", "screens/capsule.rs", "B3b"),
    ("Pane cleared", "screens/capsule.rs", "B3b"),
    ("Preview clipboard is empty", "screens/capsule.rs", "B3b"),
    ("Reap every pane in this tab", "screens/capsule.rs", "B3b"),
    ("Reap the focused pane's agent", "screens/capsule.rs", "B3b"),
    (
        "file:///Users/alexey/.jackin/logs/",
        "screens/capsule.rs",
        "B3b",
    ),
    ("in the preview", "screens/capsule.rs", "B3b"),
    ("payments-platform", "screens/capsule.rs", "B3b"),
    ("revealed in Finder", "screens/capsule.rs", "B3b"),
    ("spawning", "screens/capsule.rs", "B3b"),
    ("0.6.4 · preview", "screens/cockpit.rs", "B3b"),
    ("/.jackin/", "screens/editor.rs", "B3b and B6"),
    ("/.jackin/", "screens/settings.rs", "B3b and B6"),
    ("refresh_fails", "screens/accounts.rs", "B6"),
    ("EACCES", "screens/editor.rs", "B6"),
    ("next_workspace_id", "screens/editor.rs", "B6"),
    ("roles-experimental", "screens/editor.rs", "B6"),
    (
        "last_seen_secs = w . clock . now_secs",
        "screens/manager.rs",
        "B6",
    ),
    ("refresh_fails", "screens/manager.rs", "B6"),
    ("next_workspace_id", "screens/prelude.rs", "B6"),
    ("EACCES", "screens/settings.rs", "B6"),
    ("next_workspace_id", "world.rs", "B6"),
    ("refresh_fails", "world.rs", "B6"),
    ("takeover_at_ms", "world.rs", "B6"),
    ("tty004", "world.rs", "B6"),
    (
        "1Password locked: unlock the app and retry",
        "launch.rs",
        "B7a",
    ),
    ("LaunchPlan", "backend.rs", "B7a"),
    ("LaunchPlan", "launch.rs", "B7a"),
    ("SessionScript", "backend.rs", "B8a"),
    ("AgentProcess", "panes.rs", "B8a"),
    ("SessionScript", "panes.rs", "B8a"),
];

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
        Scope::OnlyIn(file) => path == file || path.ends_with(&format!("/{file}")),
    }
}

fn matches_at(rule: &Rule, tokens: &[(usize, Lexeme)]) -> Vec<usize> {
    if rule.literal {
        return tokens
            .iter()
            .filter_map(|(line, l)| match l {
                Lexeme::Str(s) if s.contains(rule.text) => Some(*line),
                _ => None,
            })
            .collect();
    }
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

fn scan(sources: &[Source], pending: &[Pending]) -> Report {
    let mut report = Report::default();
    let mut used = vec![false; pending.len()];
    for source in sources {
        let tokens = lex(&source.text);
        for rule in RULES.iter().filter(|r| applies(r, &source.path)) {
            for line in matches_at(rule, &tokens) {
                let slot = pending
                    .iter()
                    .position(|(t, f, _)| *t == rule.text && *f == source.path);
                match slot {
                    Some(k) => used[k] = true,
                    None => report.unexpected.push(Finding {
                        token: rule.text,
                        file: source.path.clone(),
                        line,
                    }),
                }
            }
        }
    }
    for (k, (token, file, step)) in pending.iter().enumerate() {
        if !used[k] {
            report
                .stale
                .push(format!("({token:?}, {file:?}, {step:?})"));
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
    let pending = [("CLOCK", "world.rs", "B2c")];
    let report = scan(&tree, &pending);
    assert!(report.unexpected.is_empty());
    assert_eq!(
        report.stale,
        vec![r#"("CLOCK", "world.rs", "B2c")"#.to_owned()]
    );
    let tree = [src("world.rs", "let c = CLOCK;")];
    assert!(scan(&tree, &pending).is_clean());
}

#[test]
fn demo_literal_in_domain_fails() {
    for literal in ["payments-platform", "roles-experimental", "Close tab?"] {
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
    let scoped = [
        src(
            "screens/accounts.rs",
            "const A: &str = \"1Password locked: unlock the app and retry\";",
        ),
        src(
            "launch.rs",
            "const A: &str = \"1Password locked: unlock the app and retry\";",
        ),
    ];
    let found = scan(&scoped, &[]).unexpected;
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].file, "launch.rs");
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
