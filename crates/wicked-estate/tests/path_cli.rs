//! `wicked-estate path` end to end, through the real binary.
//!
//! These assertions are about exit codes, usage lines, `--help` text and the stdout
//! document — all properties of `main.rs`, which an in-process test cannot reach. So this
//! spawns the built binary, following the same pattern as `repo_flag_cli.rs`.
//!
//! The argument cases are not incidental. The binary's shared parser pushes every token it
//! does not recognise into `positional`, so a two-operand command that merely scans for
//! `--json` (the way the one-operand `blast-radius` arm does) resolves `to` to `"--json"`
//! on `path A --json`. Each case below fails against that implementation.

use std::fs;
use std::path::PathBuf;
use std::process::{Command, Output};

fn bin() -> &'static str {
    env!("CARGO_BIN_EXE_wicked-estate")
}

/// A Rust source tree with a call chain `f0 → f1 → … → fN`, indexed into a scratch db.
/// Returns the directory; the db lives at `<dir>/graph.db`.
fn indexed_chain(tag: &str, depth: usize) -> PathBuf {
    let d = std::env::temp_dir().join(format!("ci_pathcli_{tag}_{}", std::process::id()));
    let _ = fs::remove_dir_all(&d);
    fs::create_dir_all(d.join("src")).unwrap();

    // f0 calls f1 calls f2 … so the dependency direction runs f0 → fN.
    let mut src = String::new();
    for i in 0..=depth {
        if i == depth {
            src.push_str(&format!("fn f{i}() {{}}\n"));
        } else {
            src.push_str(&format!("fn f{i}() {{ f{}(); }}\n", i + 1));
        }
    }
    fs::write(d.join("src/a.rs"), src).unwrap();

    let out = Command::new(bin())
        .current_dir(&d)
        .args(["index", ".", "--db", "graph.db"])
        .output()
        .expect("spawn index");
    assert!(
        out.status.success(),
        "index failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    d
}

fn path_in(dir: &PathBuf, args: &[&str]) -> Output {
    Command::new(bin())
        .current_dir(dir)
        .arg("path")
        .args(args)
        .args(["--db", "graph.db"])
        .output()
        .expect("spawn wicked-estate path")
}

fn stdout_of(out: &Output) -> String {
    String::from_utf8_lossy(&out.stdout).to_string()
}

fn json_of(out: &Output) -> serde_json::Value {
    let s = stdout_of(out);
    serde_json::from_str(s.trim()).unwrap_or_else(|e| {
        panic!("stdout must be exactly one JSON document; parse failed: {e}\n--- stdout ---\n{s}")
    })
}

// ── the route itself ─────────────────────────────────────────────────────────

#[test]
fn text_mode_prints_one_line_per_hop_with_kind_and_confidence() {
    let d = indexed_chain("text", 3);
    let out = path_in(&d, &["f0", "f3"]);
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let s = stdout_of(&out);

    let hop_lines: Vec<&str> = s.lines().filter(|l| l.contains("->")).collect();
    assert_eq!(hop_lines.len(), 3, "one line per hop, got:\n{s}");
    for line in &hop_lines {
        assert!(
            line.contains("confidence"),
            "each hop names its confidence: {line}"
        );
        assert!(line.contains('['), "each hop names its edge kind: {line}");
    }
    // In order, f0 → f1 → f2 → f3.
    assert!(hop_lines[0].contains("f0") && hop_lines[0].contains("f1"));
    assert!(hop_lines[2].contains("f2") && hop_lines[2].contains("f3"));
}

#[test]
fn json_mode_emits_one_document_with_denormalized_endpoints() {
    let d = indexed_chain("json", 2);
    let out = path_in(&d, &["f0", "f2", "--json"]);
    assert!(out.status.success());
    let doc = json_of(&out);

    assert_eq!(doc["found"], true);
    assert!(doc["depth_bounded"].is_boolean());
    assert!(doc["node_bounded"].is_boolean());
    assert_eq!(doc["unresolved"], serde_json::Value::Null);

    let hops = doc["hops"].as_array().expect("hops array");
    assert_eq!(hops.len(), 2);
    for hop in hops {
        for end in ["source", "target"] {
            let e = &hop[end];
            for field in ["symbol", "name", "kind", "file", "line", "line_1based"] {
                assert!(
                    !e[field].is_null(),
                    "{end}.{field} must be present — a bare id would force the caller to \
                     run a second command per hop\n{e}"
                );
            }
        }
        assert!(hop["confidence"].is_number());
        assert!(!hop["provenance"].is_null());
        assert!(!hop["resolved_by"].is_null());
    }
}

#[test]
fn json_mode_emits_no_staleness_notice() {
    let d = indexed_chain("quiet", 2);
    let out = path_in(&d, &["f0", "f2", "--json"]);
    let s = stdout_of(&out);
    assert!(
        !s.contains("STALENESS") && !s.contains("stale"),
        "notices would corrupt the single-document contract:\n{s}"
    );
    let _ = json_of(&out); // parses as exactly one document
}

// ── honesty on absence ───────────────────────────────────────────────────────

#[test]
fn unresolvable_operand_is_distinguishable_from_a_proven_absence() {
    let d = indexed_chain("unres", 2);

    let missing = json_of(&path_in(&d, &["nope_not_here", "f2", "--json"]));
    assert_eq!(missing["found"], false);
    assert_eq!(
        missing["unresolved"], "from",
        "an unresolvable input must not look like a proven absence (R3)"
    );

    // f2 is the chain's tail, so nothing is reachable from it — a real absence.
    let real = json_of(&path_in(&d, &["f2", "f0", "--json"]));
    assert_eq!(real["found"], false);
    assert_eq!(real["unresolved"], serde_json::Value::Null);
    assert_ne!(
        missing.to_string(),
        real.to_string(),
        "the two cases must not serialize identically"
    );
}

#[test]
fn text_mode_states_the_depth_bound_when_the_walk_is_cut_off() {
    let d = indexed_chain("bound", 6);
    let out = path_in(&d, &["f0", "f6", "--max-depth", "2"]);
    let s = stdout_of(&out);
    assert!(s.contains("no path found"), "{s}");
    assert!(
        s.contains("depth frontier"),
        "text mode must say the absence is bounded, not proven:\n{s}"
    );
}

// ── the argument contract ────────────────────────────────────────────────────

#[test]
fn flags_are_consumed_before_operands_in_either_position() {
    let d = indexed_chain("flagpos", 4);
    for args in [
        vec!["--max-depth", "4", "f0", "f4"],
        vec!["f0", "f4", "--max-depth", "4"],
    ] {
        let out = path_in(&d, &args);
        assert!(
            out.status.success(),
            "{args:?} must resolve from=f0 to=f4 depth=4: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        assert!(stdout_of(&out).contains("4 hop(s)"), "{args:?}");
    }
}

#[test]
fn one_operand_plus_a_flag_is_a_usage_error() {
    let d = indexed_chain("arity1", 2);
    // `--json` is consumed as a flag, leaving one operand — this must NOT resolve
    // `to` to "--json".
    let out = path_in(&d, &["f0", "--json"]);
    assert!(!out.status.success(), "expected non-zero exit");
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(err.contains("<from>") && err.contains("<to>"), "{err}");
}

#[test]
fn missing_operands_are_usage_errors() {
    let d = indexed_chain("arity0", 2);
    for args in [vec![], vec!["f0"]] {
        let out = path_in(&d, &args);
        assert!(!out.status.success(), "{args:?} must exit non-zero");
        let err = String::from_utf8_lossy(&out.stderr);
        assert!(
            err.contains("<from>") && err.contains("<to>"),
            "{args:?}: {err}"
        );
    }
}

#[test]
fn an_unknown_double_dash_token_is_rejected_not_taken_as_an_operand() {
    let d = indexed_chain("unknownflag", 2);
    let out = path_in(&d, &["--depth", "2", "f0", "f2"]);
    assert!(
        !out.status.success(),
        "an unrecognised --flag must not be resolved as a symbol name"
    );
    assert!(String::from_utf8_lossy(&out.stderr).contains("unknown flag"));
}

#[test]
fn max_depth_range_is_enforced_and_clamped() {
    let d = indexed_chain("range", 3);

    // Above the ceiling clamps rather than erroring.
    let clamped = path_in(&d, &["f0", "f3", "--max-depth", "17"]);
    assert!(clamped.status.success(), "17 must clamp to 16, not error");
    assert!(stdout_of(&clamped).contains("3 hop(s)"));

    for bad in ["0", "abc"] {
        let out = path_in(&d, &["f0", "f3", "--max-depth", bad]);
        assert!(
            !out.status.success(),
            "--max-depth {bad} must exit non-zero"
        );
        assert!(
            String::from_utf8_lossy(&out.stderr).contains("1..=16"),
            "the error must name the accepted range"
        );
    }
}

#[test]
fn default_depth_is_twelve() {
    // A 12-hop chain is found by default; a 13-hop chain is not.
    let ok = indexed_chain("def12", 12);
    let out = path_in(&ok, &["f0", "f12"]);
    assert!(stdout_of(&out).contains("12 hop(s)"), "{}", stdout_of(&out));

    let too_deep = indexed_chain("def13", 13);
    let out = path_in(&too_deep, &["f0", "f13"]);
    assert!(
        stdout_of(&out).contains("no path found"),
        "a 13-hop chain must exceed the default depth of 12:\n{}",
        stdout_of(&out)
    );
}

#[test]
fn help_lists_path_with_its_flags() {
    let out = Command::new(bin()).arg("--help").output().expect("spawn");
    let s = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    let line = s
        .lines()
        .find(|l| l.contains("wicked-estate path"))
        .unwrap_or_else(|| panic!("--help must list `path`:\n{s}"));
    assert!(line.contains("--max-depth"), "{line}");
    assert!(line.contains("--json"), "{line}");
}
