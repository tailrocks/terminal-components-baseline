use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::Value;
use tui_test::profile::Rgb;
use tui_test::{
    SnapshotCell, SnapshotColor, SnapshotCursor, SnapshotCursorShape, SnapshotMetadata,
    SnapshotParts, SnapshotScope, SnapshotUnderline, TerminalSnapshot,
};

use crate::support::{
    App, Launch, SHOWCASE, SessionMode, file_sha256, reference_binary, reference_digest, text,
    try_launch,
};

const REFERENCE_SHA: &str = "fffe002e936d3621f22f9132144066a1374d5d74";
const SHOWCASE_REFERENCE_SHA256: &str =
    "430460e90a752587d7f1f6d2e37b4d8343076a2787a343048fbd3448b3d54a0b";
const COMPARATOR_REV: &str = "92ef2d9ee082b35f68e066c5129fedbf07675a80";
const ACCEPTANCE_COMPARATOR_REV: &str = "d8d54155947f6303a41dfd6079b53c0abc16f29a";
const ACCEPTANCE_LOCKFILE_SHA256: &str =
    "582b6cd514d0bd9c90966bdea90028ce24b652a0607d0ef7b8369a1a7532e319";

fn manifest() -> Value {
    let path =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("docs/testing/reference-manifest.json");
    serde_json::from_slice(&std::fs::read(&path).expect("reference manifest")).expect("manifest")
}

fn reference_showcase() -> PathBuf {
    let bin = reference_binary("showcase");
    assert!(
        bin.is_file(),
        "missing reference showcase {}",
        bin.display()
    );
    assert_eq!(file_sha256(&bin), reference_digest("showcase"));
    bin
}

fn launch_showcase(name: &str) -> App {
    let mut launch = Launch::new(name, SHOWCASE);
    launch.args = vec![
        "--page".to_string(),
        "overview".to_string(),
        "--frame".to_string(),
        "40".to_string(),
    ];
    launch.motion = Some("paused".to_string());
    launch.color = Some("truecolor".to_string());
    launch.cols = 120;
    launch.rows = 40;
    try_launch(launch).expect("launch candidate showcase")
}

fn baseline_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/functional/baselines")
}

fn tree_digest(root: &Path) -> String {
    let mut files = Vec::new();
    let mut dirs = vec![root.to_path_buf()];
    while let Some(dir) = dirs.pop() {
        let mut entries = std::fs::read_dir(&dir)
            .unwrap_or_else(|error| panic!("read {}: {error}", dir.display()))
            .collect::<Result<Vec<_>, _>>()
            .unwrap_or_else(|error| panic!("entry: {error}"));
        entries.sort_by_key(std::fs::DirEntry::file_name);
        for entry in entries {
            let path = entry.path();
            let kind = entry.file_type().expect("file type");
            assert!(!kind.is_symlink(), "symlink {}", path.display());
            if kind.is_dir() {
                dirs.push(path);
            } else {
                files.push(path);
            }
        }
    }
    files.sort();
    let output = if Path::new("/usr/bin/shasum").is_file() {
        Command::new("/usr/bin/shasum")
            .arg("-a")
            .arg("256")
            .args(&files)
            .output()
    } else {
        Command::new("/usr/bin/sha256sum").args(&files).output()
    }
    .expect("hash baseline tree");
    assert!(
        output.status.success(),
        "baseline tree hash failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).expect("hash output")
}

fn line_index(actions: &str, exact: &str) -> usize {
    actions
        .lines()
        .position(|line| line == exact)
        .unwrap_or_else(|| panic!("missing {exact:?}\n{actions}"))
}

fn prefix_index(actions: &str, prefix: &str) -> usize {
    actions
        .lines()
        .position(|line| line.starts_with(prefix))
        .unwrap_or_else(|| panic!("missing prefix {prefix:?}\n{actions}"))
}

fn read_disposition_sample(path: &Path) -> tuiscotti_latest::sample::Sample {
    tuiscotti_latest::sample::Sample::from_file(path)
        .unwrap_or_else(|error| panic!("read disposition sample {}: {error}", path.display()))
}

fn active_baseline_path(root: &Path, test_id: &str) -> PathBuf {
    root.join(format!("{test_id}.tui.pb"))
}

fn disposition_sample_fixture(cols: u32, rows: u32) -> TerminalSnapshot {
    let cell = SnapshotCell::new(
        "x".to_string(),
        1,
        SnapshotColor::Default,
        SnapshotColor::Default,
        tui_test::terminal::cell::Attrs::empty(),
        SnapshotUnderline::None,
        SnapshotColor::Default,
        None,
        None,
    )
    .expect("fixture cell");
    let cell_count = usize::try_from(cols * rows).expect("fixture cell count fits usize");
    TerminalSnapshot::try_new(SnapshotParts {
        cols,
        rows,
        scope: SnapshotScope::Viewport,
        viewport_origin: 0,
        cells: vec![cell; cell_count],
        palette: [Rgb::new(0, 0, 0); 256],
        default_foreground: Rgb::new(255, 255, 255),
        default_background: Rgb::new(0, 0, 0),
        default_cursor: Rgb::new(255, 255, 255),
        cursor: SnapshotCursor::new(
            0,
            0,
            true,
            true,
            SnapshotCursorShape::Block,
            Rgb::new(255, 255, 255),
            None,
        ),
        title: None,
        metadata: SnapshotMetadata::default(),
    })
    .expect("fixture snapshot")
}

fn assert_suspect_dispositions(active_root: &Path) {
    let path =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("docs/testing/baseline-dispositions.json");
    let value: Value =
        serde_json::from_slice(&std::fs::read(&path).expect("dispositions")).expect("dispositions");
    let rows = value.as_array().expect("dispositions array");
    let expected = [
        (
            "functional/holla_resize_narrow",
            120_u64,
            40_u64,
            "functional/holla_browser_80x24",
            80_u64,
            24_u64,
        ),
        (
            "functional/tablepro_resize_60x20",
            120_u64,
            40_u64,
            "functional/tablepro_workbench_60x20",
            60_u64,
            20_u64,
        ),
        (
            "functional/showcase_help_resize",
            160_u64,
            50_u64,
            "functional/showcase_help_80x24",
            80_u64,
            24_u64,
        ),
    ];
    for (id, cols, rows_n, replacement, replacement_cols, replacement_rows) in expected {
        let row = rows
            .iter()
            .find(|row| row["id"] == id)
            .unwrap_or_else(|| panic!("missing disposition {id}"));
        assert_eq!(row["status"].as_str(), Some("suspect"), "{id}");
        assert_eq!(row["cols"].as_u64(), Some(cols), "{id}");
        assert_eq!(row["rows"].as_u64(), Some(rows_n), "{id}");
        assert_eq!(row["replacement_id"].as_str(), Some(replacement), "{id}");
        assert!(row.get("retired_id").is_none(), "{id}");
        assert!(row.get("commit").is_none(), "{id}");
        let reason = row["reason"].as_str().expect("reason");
        assert!(
            reason.contains(
                "The file's dimensions are not the dimensions named by the test, so it cannot be the narrow or named-size checkpoint."
            ),
            "{id}: {reason}"
        );
        let baseline =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(row["path"].as_str().expect("path"));
        let file: Value = serde_json::from_slice(&std::fs::read(&baseline).expect("baseline"))
            .expect("baseline json");
        assert_eq!(file["test_id"].as_str(), Some(id));
        assert_eq!(file["snapshot"]["cols"].as_u64(), Some(cols));
        assert_eq!(file["snapshot"]["rows"].as_u64(), Some(rows_n));
        let replacement_row = rows
            .iter()
            .find(|row| row["id"] == replacement)
            .unwrap_or_else(|| panic!("missing replacement disposition {replacement}"));
        assert_eq!(
            replacement_row["status"].as_str(),
            Some("accepted"),
            "{replacement}"
        );
        assert_eq!(
            replacement_row["replaces"].as_str(),
            Some(id),
            "{replacement}"
        );
        let replacement_path = active_baseline_path(active_root, replacement);
        let replacement_sample = read_disposition_sample(&replacement_path);
        assert_eq!(replacement_sample.test_id(), replacement, "{replacement}");
        assert_eq!(
            replacement_sample.snapshot().cols(),
            u32::try_from(replacement_cols).expect("replacement columns fit u32"),
            "{replacement}"
        );
        assert_eq!(
            replacement_sample.snapshot().rows(),
            u32::try_from(replacement_rows).expect("replacement rows fit u32"),
            "{replacement}"
        );
    }
}

#[test]
fn g05_approved_binary_manifest_matches_active_samples() {
    let root = baseline_root();
    let manifest_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("docs/testing/binary-baselines-20261108.tsv");
    let manifest = std::fs::read_to_string(&manifest_path).expect("binary baseline manifest");
    let mut lines = manifest.lines();
    assert_eq!(
        lines.next(),
        Some("test_id\tbaseline_path\tsha256\treceipt_path\tacceptance_sequence\tsource_lineage")
    );
    let mut ids = std::collections::BTreeSet::new();
    let mut paths = std::collections::BTreeSet::new();
    let mut count = 0;
    for line in lines {
        let fields: Vec<_> = line.split('\t').collect();
        assert_eq!(fields.len(), 6, "{line}");
        let [
            test_id,
            baseline_path,
            expected_sha,
            receipt_path,
            sequence,
            source_lineage,
        ] = fields.as_slice()
        else {
            unreachable!("field count checked")
        };
        assert!(ids.insert(*test_id), "duplicate test ID {test_id}");
        assert!(
            !source_lineage.is_empty(),
            "missing source lineage for {test_id}"
        );
        assert!(
            sequence.parse::<u16>().is_ok(),
            "bad acceptance sequence {sequence}"
        );
        assert!(
            baseline_path.ends_with(".tui.pb"),
            "not a PB path: {baseline_path}"
        );
        assert!(baseline_path.starts_with("tests/functional/baselines/"));
        let relative = Path::new(baseline_path)
            .strip_prefix("tests/functional/baselines")
            .expect("baseline-relative path");
        let path = root.join(relative);
        assert!(
            paths.insert(relative.to_path_buf()),
            "duplicate path {baseline_path}"
        );
        assert!(
            !root.join(format!("{test_id}.tui.json")).exists(),
            "active JSON remains for {test_id}"
        );
        let bytes =
            std::fs::read(&path).unwrap_or_else(|error| panic!("{}: {error}", path.display()));
        assert!(
            bytes.starts_with(b"TUISCPB\0"),
            "{} is not a protobuf container",
            path.display()
        );
        assert_eq!(file_sha256(&path), *expected_sha, "{test_id}");
        let sample = read_disposition_sample(&path);
        assert_eq!(sample.test_id(), *test_id);
        let receipt_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(receipt_path);
        let receipt: Value = serde_json::from_slice(
            &std::fs::read(&receipt_path)
                .unwrap_or_else(|error| panic!("{}: {error}", receipt_path.display())),
        )
        .expect("binary acceptance receipt");
        assert_eq!(
            receipt["candidate_sha256"].as_str(),
            Some(*expected_sha),
            "{test_id}"
        );
        count += 1;
    }
    assert_eq!(count, 105);
    assert!(!ids.contains("functional/tablepro_query_syntax_error"));
    let historical_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("docs/testing/acceptance-receipts/historical-binary-20261108/transactions.jsonl");
    let historical = std::fs::read_to_string(&historical_path)
        .unwrap_or_else(|error| panic!("{}: {error}", historical_path.display()));
    let mut historical_ids = std::collections::BTreeSet::new();
    let mut historical_paths = std::collections::BTreeSet::new();
    let mut historical_count = 0;
    for line in historical.lines() {
        let receipt: Value = serde_json::from_str(line).expect("historical transaction receipt");
        assert_eq!(
            receipt["schema"].as_str(),
            Some("tcb.historical-binary-transaction.v1")
        );
        let test_id = receipt["test_id"].as_str().expect("historical test ID");
        assert!(
            historical_ids.insert(test_id.to_string()),
            "duplicate historical test ID {test_id}"
        );
        assert!(
            !ids.contains(test_id),
            "historical ID collides with active ID {test_id}"
        );
        let baseline_path = receipt["baseline_path"].as_str().expect("baseline path");
        assert!(baseline_path.starts_with("tests/functional/baselines/"));
        assert!(baseline_path.ends_with(".tui.pb"));
        let relative = Path::new(baseline_path)
            .strip_prefix("tests/functional/baselines")
            .expect("historical baseline is under baseline root");
        assert!(
            historical_paths.insert(relative.to_path_buf()),
            "duplicate historical path {baseline_path}"
        );
        let path = root.join(relative);
        let expected_sha = receipt["baseline_sha256"].as_str().expect("baseline SHA");
        assert_eq!(file_sha256(&path), expected_sha, "{test_id}");
        assert!(
            std::fs::read(&path)
                .expect("historical sample")
                .starts_with(b"TUISCPB\0")
        );
        let sample = read_disposition_sample(&path);
        assert_eq!(sample.test_id(), test_id, "{test_id}");

        let historical_json = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(
            receipt["historical_json"]
                .as_str()
                .expect("historical JSON path"),
        );
        assert!(historical_json.is_file(), "{}", historical_json.display());
        let expected_path = historical_json
            .strip_prefix(
                PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/functional/baselines"),
            )
            .expect("historical JSON is under baseline root")
            .with_extension("pb");
        assert_eq!(
            relative, expected_path,
            "history-derived path for {test_id}"
        );
        assert_eq!(
            receipt["historical_json_sha256"].as_str().map(str::len),
            Some(64)
        );
        assert!(receipt["path_id"].as_str().is_some());

        let candidate = &receipt["candidate_record"];
        assert_eq!(candidate["test_id"].as_str(), Some(test_id));
        assert_eq!(candidate["candidate_sha256"].as_str(), Some(expected_sha));
        assert!(
            candidate["source"]
                .as_str()
                .is_some_and(|source| !source.is_empty())
        );
        let alternates = candidate["alternates"]
            .as_array()
            .expect("candidate alternatives");
        assert!(
            alternates.is_empty()
                || alternates.iter().any(|alternate| {
                    alternate.as_str() == Some(expected_sha)
                        || alternate["sha256"].as_str() == Some(expected_sha)
                }),
            "selected candidate absent from alternatives for {test_id}"
        );

        let acceptance = &receipt["acceptance"];
        assert_eq!(acceptance["exit_code"].as_i64(), Some(0), "{test_id}");
        assert_eq!(acceptance["cli_version"].as_str(), Some("tuiscotti 0.4.0"));
        assert_eq!(acceptance["cli_sha256"].as_str().map(str::len), Some(64));
        assert_eq!(
            acceptance["staging_baseline_sha256"].as_str(),
            Some(expected_sha)
        );
        let response = &acceptance["stdout"];
        assert_eq!(response["schema"].as_str(), Some("tuiscotti.cli.v1"));
        assert_eq!(response["command"].as_str(), Some("accept"));
        assert_eq!(response["status"].as_str(), Some("pass"));
        assert_eq!(response["candidate_sha256"].as_str(), Some(expected_sha));
        assert_eq!(response["baseline_sha256"].as_str(), Some(expected_sha));
        let raw_response = acceptance["stdout_raw"].as_str().expect("raw CLI result");
        assert_eq!(
            serde_json::from_str::<Value>(raw_response).expect("raw CLI JSON"),
            *response
        );
        let command = acceptance["command"].as_array().expect("accept command");
        assert!(command.iter().any(|part| part == "--expect-baseline"));
        assert!(command.iter().any(|part| part == "missing"));
        historical_count += 1;
    }
    assert_eq!(historical_count, 1868);
    fn collect_pb_paths(root: &Path, dir: &Path, paths: &mut std::collections::BTreeSet<PathBuf>) {
        for entry in std::fs::read_dir(dir).expect("read active baseline directory") {
            let path = entry.expect("baseline entry").path();
            if path.is_dir() {
                collect_pb_paths(root, &path, paths);
            } else if path.extension().is_some_and(|extension| extension == "pb") {
                paths.insert(
                    path.strip_prefix(root)
                        .expect("path is inside baseline root")
                        .to_path_buf(),
                );
            }
        }
    }
    let mut actual_paths = std::collections::BTreeSet::new();
    collect_pb_paths(&root, &root, &mut actual_paths);
    paths.extend(historical_paths);
    assert_eq!(actual_paths, paths);
}

#[test]
fn g05_disposition_sample_reader_accepts_pb_path() {
    let fixture_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("target/functional")
        .join(format!(
            "g05-disposition-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("system clock")
                .as_nanos()
        ));
    std::fs::create_dir_all(&fixture_root).expect("create temporary fixture root");
    let replacements = [
        ("functional/holla_browser_80x24", 80, 24),
        ("functional/tablepro_workbench_60x20", 60, 20),
        ("functional/showcase_help_80x24", 80, 24),
    ];
    for (test_id, cols, rows) in replacements {
        let sample = tuiscotti_latest::sample::Sample::state(
            test_id,
            disposition_sample_fixture(cols, rows),
            tuiscotti_latest::sample::Observation::empty(),
        )
        .expect("build binary sample fixture");
        let bytes = sample.to_bytes().expect("serialize binary sample fixture");
        assert_eq!(&bytes[..8], b"TUISCPB\0", "binary container magic");
        let fixture = active_baseline_path(&fixture_root, test_id);
        std::fs::create_dir_all(fixture.parent().expect("fixture parent"))
            .expect("create temporary baseline directory");
        std::fs::write(&fixture, bytes).expect("write temporary binary fixture");
    }
    assert_suspect_dispositions(&fixture_root);

    std::fs::remove_dir_all(fixture_root).expect("remove temporary fixture root");
}

fn assert_acceptance_receipts() {
    let manifest = manifest();
    let cases = [
        (
            "functional_holla_browser_80x24",
            "holla",
            "functional/holla_browser_80x24",
        ),
        (
            "functional_tablepro_workbench_60x20",
            "tablepro",
            "functional/tablepro_workbench_60x20",
        ),
        (
            "functional_showcase_help_80x24",
            "showcase",
            "functional/showcase_help_80x24",
        ),
        (
            "functional_showcase_keyboard_run_task",
            "showcase",
            "functional/showcase_keyboard_run_task",
        ),
        (
            "functional_tablepro_query_recovered",
            "tablepro",
            "functional/tablepro_query_recovered",
        ),
        (
            "functional_jackin_stop_instance_dialog",
            "jackin",
            "functional/jackin_stop_instance_dialog",
        ),
        (
            "functional_jackin_settings_dirty_leave",
            "jackin",
            "functional/jackin_settings_dirty_leave",
        ),
        (
            "functional_jackin_launch_failed",
            "jackin",
            "functional/jackin_launch_failed",
        ),
        (
            "functional_jackin_launch_cockpit",
            "jackin",
            "functional/jackin_launch_cockpit",
        ),
        (
            "functional_jackin_launch_capsule",
            "jackin",
            "functional/jackin_launch_capsule",
        ),
        (
            "functional_jackin_launch_detached",
            "jackin",
            "functional/jackin_launch_detached",
        ),
        (
            "functional_jackin_launch_reconnected",
            "jackin",
            "functional/jackin_launch_reconnected",
        ),
    ];
    for (file, binary, baseline_id) in cases {
        let receipt_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("docs/testing/acceptance-receipts/protobuf-20261107")
            .join(format!("{file}.json"));
        let receipt: Value =
            serde_json::from_slice(&std::fs::read(&receipt_path).expect("receipt"))
                .expect("receipt json");
        assert_eq!(
            receipt["source_sha"].as_str(),
            Some(REFERENCE_SHA),
            "{file}"
        );
        assert_eq!(
            receipt["lockfile_sha256"].as_str(),
            Some(ACCEPTANCE_LOCKFILE_SHA256),
            "{file}"
        );
        assert_eq!(
            receipt["comparator_rev"].as_str(),
            Some(ACCEPTANCE_COMPARATOR_REV),
            "{file}"
        );
        let expected_previous = match file {
            "functional_tablepro_query_recovered" => {
                "ed9b89699ee57340d92bc47bd37169b523bd39c321cda73d1b55bccf07543c0b"
            }
            "functional_jackin_launch_failed" => {
                "2b2c711b926a175a2bde8fa73921fbc4249828b5d2d66bfd9b71af0a829b67f2"
            }
            _ => "missing",
        };
        assert_eq!(
            receipt["expect_baseline"].as_str(),
            Some(expected_previous),
            "{file}"
        );
        let precondition = receipt["precondition"].as_str().unwrap_or("");
        assert!(!precondition.is_empty(), "{file}");
        let expected_sha = manifest["binaries"][binary]["sha256"]
            .as_str()
            .unwrap_or_else(|| panic!("missing binary {binary}"));
        assert_eq!(
            receipt["binary_sha256"].as_str(),
            Some(expected_sha),
            "{file}"
        );
        let baseline = baseline_root().join(format!("{baseline_id}.tui.pb"));
        assert_eq!(
            receipt["candidate_sha256"].as_str(),
            Some(file_sha256(&baseline).as_str()),
            "{file}"
        );
    }
}

fn refuse_candidate(bin: &Path) {
    let mut launch = Launch::new(
        "g05-candidate-refuses",
        bin.to_str().expect("reference path"),
    );
    launch.mode = SessionMode::Candidate;
    let Err(error) = try_launch(launch) else {
        panic!("candidate must refuse the reference binary");
    };
    let rendered = error.to_string();
    assert!(rendered.contains("reference binary"), "{rendered}");
    assert!(
        error.message.contains("reference binary"),
        "{}",
        error.message
    );
    let saved = std::fs::read_to_string(error.evidence.join("spawn-error.txt")).expect("error");
    assert!(saved.contains("reference binary"), "{saved}");
    assert!(!error.evidence.join("session.json").exists());
}

#[test]
fn g05_candidate_refuses_reference_binary() {
    let bin = reference_showcase();
    refuse_candidate(&bin);
    let value = manifest();
    let platforms = value["binaries"]["showcase"]["platforms"]
        .as_object()
        .expect("showcase platforms");
    for (target, platform) in platforms {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(
            platform["path"]
                .as_str()
                .unwrap_or_else(|| panic!("path for {target}")),
        );
        assert!(
            path.is_file(),
            "missing {target} showcase {}",
            path.display()
        );
        assert_eq!(
            file_sha256(&path),
            platform["sha256"]
                .as_str()
                .unwrap_or_else(|| panic!("sha256 for {target}"))
        );
        if path.canonicalize().expect("platform binary") == bin.canonicalize().expect("host") {
            continue;
        }
        refuse_candidate(&path);
    }
}

#[test]
fn g05_reference_mode_requires_manifest_identity() {
    let candidate = Path::new(SHOWCASE);
    assert!(candidate.is_file(), "missing candidate showcase");
    assert_ne!(file_sha256(candidate), SHOWCASE_REFERENCE_SHA256);
    let mut launch = Launch::new("g05-reference-identity", SHOWCASE);
    launch.mode = SessionMode::Reference;
    let Err(error) = try_launch(launch) else {
        panic!("candidate digest is not the reference identity");
    };
    let rendered = error.to_string();
    assert!(rendered.contains("reference identity"), "{rendered}");
    assert!(
        error.message.contains("reference identity"),
        "{}",
        error.message
    );
    assert!(!error.evidence.join("session.json").exists());
}

#[test]
fn g05_reference_session_receipt_records_identity() {
    let bin = reference_showcase();
    let manifest = manifest();
    assert_eq!(manifest["reference_sha"], REFERENCE_SHA);
    assert_eq!(
        manifest["binaries"]["showcase"]["sha256"],
        SHOWCASE_REFERENCE_SHA256
    );
    assert_eq!(manifest["comparator"]["rev"], COMPARATOR_REV);
    assert_eq!(manifest["acceptance"]["session_file"], "session.json");
    let mut launch = Launch::new(
        "g05-reference-session",
        bin.to_str().expect("reference path"),
    );
    launch.mode = SessionMode::Reference;
    launch.args = vec![
        "--page".to_string(),
        "overview".to_string(),
        "--frame".to_string(),
        "40".to_string(),
    ];
    launch.motion = Some("paused".to_string());
    launch.color = Some("truecolor".to_string());
    launch.cols = 120;
    launch.rows = 40;
    let app = try_launch(launch).expect("launch reference showcase");
    let screen = app.wait_screen("reference showcase painted", |screen| {
        text(screen).chars().any(|ch| ch.is_ascii_alphanumeric())
    });
    assert!(text(&screen).chars().any(|ch| ch.is_ascii_alphanumeric()));
    let evidence = app.evidence_dir();
    let baselines = baseline_root();
    assert!(
        evidence.starts_with(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("target/functional"))
    );
    assert!(!evidence.starts_with(&baselines));
    let session: Value = serde_json::from_slice(
        &std::fs::read(evidence.join("session.json")).expect("session.json"),
    )
    .expect("session json");
    let mut keys: Vec<_> = session
        .as_object()
        .expect("session object")
        .keys()
        .cloned()
        .collect();
    keys.sort();
    assert_eq!(
        keys,
        [
            "TERM",
            "args",
            "binary_path",
            "binary_sha256",
            "clock",
            "color",
            "cols",
            "comparator_name",
            "comparator_rev",
            "history",
            "lockfile_sha256",
            "mode",
            "motion",
            "platform",
            "reference_sha",
            "rows",
        ]
    );
    assert_eq!(session["mode"].as_str(), Some("reference"));
    assert_eq!(session["reference_sha"], manifest["reference_sha"]);
    assert_eq!(session["reference_sha"].as_str(), Some(REFERENCE_SHA));
    assert_eq!(session["lockfile_sha256"], manifest["lockfile_sha256"]);
    assert_eq!(
        session["binary_sha256"].as_str(),
        Some(reference_digest("showcase").as_str())
    );
    let recorded = PathBuf::from(session["binary_path"].as_str().expect("binary path"));
    assert_eq!(
        recorded.canonicalize().expect("binary"),
        bin.canonicalize().expect("bin")
    );
    assert_eq!(
        session["args"],
        serde_json::json!(["--page", "overview", "--frame", "40"])
    );
    assert_eq!(session["cols"].as_u64(), Some(120));
    assert_eq!(session["rows"].as_u64(), Some(40));
    assert_eq!(session["color"].as_str(), Some("truecolor"));
    assert_eq!(session["platform"].as_str(), Some("macos"));
    assert_eq!(session["history"].as_bool(), Some(false));
    assert_eq!(session["motion"].as_str(), Some("paused"));
    assert_eq!(session["clock"].as_bool(), Some(false));
    assert_eq!(session["TERM"].as_str(), Some("xterm-256color"));
    assert_eq!(session["comparator_name"], manifest["comparator"]["name"]);
    assert_eq!(session["comparator_rev"].as_str(), Some(COMPARATOR_REV));
    app.finish();
}

#[test]
fn g05_acceptance_receipts_match_reference_baselines() {
    assert_acceptance_receipts();
}

#[test]
fn g05_check_visual_does_not_write_baselines() {
    assert_suspect_dispositions(&baseline_root());
    let root = baseline_root();
    let before = tree_digest(&root);
    let app = launch_showcase("g05-check-visual");
    app.wait_text("120×40");
    let session: Value = serde_json::from_slice(
        &std::fs::read(app.evidence_dir().join("session.json")).expect("session.json"),
    )
    .expect("session json");
    assert_eq!(session["mode"].as_str(), Some("candidate"));
    let sha = session["binary_sha256"].as_str().expect("sha");
    assert_ne!(sha, SHOWCASE_REFERENCE_SHA256);
    let manifest = manifest();
    for name in ["showcase", "tablepro", "holla", "jackin"] {
        assert_ne!(sha, manifest["binaries"][name]["sha256"].as_str().unwrap());
        let platforms = manifest["binaries"][name]["platforms"]
            .as_object()
            .unwrap_or_else(|| panic!("platforms for {name}"));
        for (target, platform) in platforms {
            assert_ne!(
                sha,
                platform["sha256"]
                    .as_str()
                    .unwrap_or_else(|| panic!("{name} {target}")),
                "{name} {target}"
            );
        }
    }
    assert!(!app.evidence_dir().starts_with(&root));
    let missing = root.join("functional/g05_read_only.tui.pb");
    assert!(!missing.exists());
    let outcome =
        app.check_visual_with_precondition(&root, "functional/g05_read_only", "screen is visible");
    assert!(
        matches!(outcome, tuiscotti_latest::check::CheckOutcome::Failure(_)),
        "missing baseline must not pass: {outcome:?}"
    );
    let _ = app.check_visual(&root, "functional/g05_read_only_empty");
    let actions = std::fs::read_to_string(app.evidence_dir().join("actions.txt")).expect("actions");
    let stated = line_index(&actions, "precondition screen is visible");
    let compared = prefix_index(&actions, "visual functional/g05_read_only candidate ");
    assert!(stated < compared, "{actions}");
    let empty = line_index(&actions, "precondition ");
    let empty_compared = prefix_index(&actions, "visual functional/g05_read_only_empty candidate ");
    assert!(empty < empty_compared, "{actions}");
    assert!(!missing.exists());
    assert!(!root.join("functional/g05_read_only_empty.tui.pb").exists());
    assert_eq!(tree_digest(&root), before);
    app.finish();
}

#[test]
fn g05_tests_do_not_invoke_accept() {
    let needle = concat!("tuiscotti", " ", "accept");
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests");
    let mut pending = vec![root];
    while let Some(dir) = pending.pop() {
        for entry in std::fs::read_dir(&dir)
            .unwrap_or_else(|error| panic!("read {}: {error}", dir.display()))
        {
            let entry = entry.unwrap_or_else(|error| panic!("entry: {error}"));
            let path = entry.path();
            let kind = entry.file_type().expect("file type");
            if kind.is_symlink() {
                continue;
            }
            if kind.is_dir() {
                // Baseline captures are about 1.9 GB. Reading every byte in an
                // unoptimized test takes 27s when the machine is quiet and
                // exceeds the functional profile's 90s kill when it is not.
                // Those files are screenshots, not an accept invocation.
                if path.file_name().is_some_and(|name| name == "baselines") {
                    continue;
                }
                pending.push(path);
                continue;
            }
            let bytes = std::fs::read(&path)
                .unwrap_or_else(|error| panic!("read {}: {error}", path.display()));
            let found = bytes
                .windows(needle.len())
                .any(|window| window == needle.as_bytes());
            assert!(!found, "{} contains the approval command", path.display());
        }
    }
}
