//! End-to-end checks: the language specs against hand-counted complexity, and
//! the JSON contract against the token budget.

use crap_cli::lang::Lang;
use crap_cli::score::{self, Options};
use std::process::Command;
use tree_sitter::Parser;

fn discover(file: &str, lang: Lang, include_tests: bool) -> Vec<score::Func> {
    let src = std::fs::read(format!("tests/fixtures/{file}")).expect("fixture");
    let mut parser = Parser::new();
    parser.set_language(&lang.grammar()).expect("grammar loads");
    let tree = parser.parse(&src, None).expect("parses");
    let funcs = score::discover(
        &tree,
        &src,
        lang,
        Options {
            include_tests,
            snippet: false,
        },
    );
    assert!(!funcs.is_empty(), "{file} produced no functions");
    funcs
}

fn cc(file: &str, lang: Lang) -> Vec<(String, u32)> {
    discover(file, lang, false)
        .into_iter()
        .map(|f| (f.name, f.cc))
        .collect()
}

/// Each fixture annotates its functions with the complexity a human counts by
/// hand. A wrong node kind in the spec fails here.
#[test]
fn rust_complexity_matches_the_hand_count() {
    let got = cc("sample.rs", Lang::Rust);
    for (name, expected) in [
        ("trivial", 1),
        ("binary_ops", 3),
        ("question", 2),
        ("outer", 2),
        ("inner", 2),
        ("matcher", 4),
        ("method", 3),
    ] {
        let found = got.iter().find(|(n, _)| n == name).unwrap_or_else(|| {
            panic!(
                "{name} not discovered; found {:?}",
                got.iter().map(|(n, _)| n).collect::<Vec<_>>()
            )
        });
        assert_eq!(found.1, expected, "{name} complexity");
    }
}

#[test]
fn go_complexity_matches_the_hand_count() {
    let got = cc("sample.go", Lang::Go);
    for (name, expected) in [
        ("Trivial", 1),
        ("BinaryOps", 3),
        ("Outer", 2),
        ("f", 2),
        ("Switcher", 3),
        ("Selector", 2),
        ("Method", 3),
    ] {
        let found = got.iter().find(|(n, _)| n == name).unwrap_or_else(|| {
            panic!(
                "{name} not discovered; found {:?}",
                got.iter().map(|(n, _)| n).collect::<Vec<_>>()
            )
        });
        assert_eq!(found.1, expected, "{name} complexity");
    }
}

#[test]
fn lua_complexity_matches_the_hand_count() {
    let got = cc("sample.lua", Lang::Lua);
    for (name, expected) in [
        ("M.trivial", 1),
        ("M.binary_ops", 3),
        ("M.outer", 2),
        ("inner", 2),
        ("M.loops", 4),
        ("M.chained", 4),
    ] {
        let found = got.iter().find(|(n, _)| n == name).unwrap_or_else(|| {
            panic!(
                "{name} not discovered; found {:?}",
                got.iter().map(|(n, _)| n).collect::<Vec<_>>()
            )
        });
        assert_eq!(found.1, expected, "{name} complexity");
    }
}

#[test]
fn typescript_complexity_matches_the_hand_count() {
    let got = cc("sample.ts", Lang::Ts);
    for (name, expected) in [
        ("trivial", 1),
        ("binaryOps", 3),
        ("nullish", 2),
        ("outer", 2),
        ("inner", 2),
        ("run", 3),
        ("value", 1),
        ("guard", 2),
        ("handler", 2),
    ] {
        let found = got.iter().find(|(n, _)| n == name).unwrap_or_else(|| {
            panic!(
                "{name} not discovered; found {:?}",
                got.iter().map(|(n, _)| n).collect::<Vec<_>>()
            )
        });
        assert_eq!(found.1, expected, "{name} complexity");
    }
}

#[test]
fn test_code_is_skipped_by_default_and_included_on_request() {
    let without = cc("sample.rs", Lang::Rust);
    assert!(!without.iter().any(|(n, _)| n == "test_helper"));

    let with: Vec<String> = discover("sample.rs", Lang::Rust, true)
        .into_iter()
        .map(|f| f.name)
        .collect();
    assert!(with.contains(&"test_helper".to_string()), "found {with:?}");
}

#[test]
fn class_and_impl_scopes_are_captured() {
    let method = discover("sample.ts", Lang::Ts, false)
        .into_iter()
        .find(|f| f.name == "run")
        .expect("run");
    assert_eq!(method.scope.as_deref(), Some("Service"));

    let rust = discover("sample.rs", Lang::Rust, false)
        .into_iter()
        .find(|f| f.name == "method")
        .expect("method");
    assert_eq!(rust.scope.as_deref(), Some("impl Widget"));
}

#[test]
fn signatures_are_opt_in() {
    let plain = discover("sample.ts", Lang::Ts, false);
    assert!(plain.iter().all(|f| f.sig.is_none()));

    let src = std::fs::read("tests/fixtures/sample.ts").unwrap();
    let mut parser = Parser::new();
    parser.set_language(&Lang::Ts.grammar()).unwrap();
    let tree = parser.parse(&src, None).unwrap();
    let with = score::discover(
        &tree,
        &src,
        Lang::Ts,
        Options {
            include_tests: false,
            snippet: true,
        },
    );
    let run = with.iter().find(|f| f.name == "run").expect("run");
    assert_eq!(
        run.sig.as_deref().unwrap().split(" //").next().unwrap(),
        "run(x: number): number {"
    );
}

fn run_cli(args: &[&str]) -> (String, i32) {
    let output = Command::new(env!("CARGO_BIN_EXE_crap"))
        .args(args)
        .output()
        .expect("binary runs");
    (
        String::from_utf8(output.stdout).expect("utf8"),
        output.status.code().unwrap_or(-1),
    )
}

#[test]
fn json_stays_inside_the_token_budget() {
    let (out, code) = run_cli(&[
        "tests/fixtures",
        "--include-tests",
        "--coverage",
        "tests/fixtures/lcov.info",
        "--all",
        "--top",
        "1000",
        "--max-tokens",
        "200",
    ]);
    assert_eq!(code, 0);
    let parsed: serde_json::Value = serde_json::from_str(&out).expect("valid JSON");
    assert!(
        out.len().div_ceil(4) <= 200,
        "budget blown: {} chars",
        out.len()
    );
    assert!(
        parsed["dropped"]["not_shown"].as_u64().unwrap() > 0,
        "nothing was dropped"
    );
    assert!(parsed["summary"]["functions"].as_u64().unwrap() > 0);
}

#[test]
fn tiny_budgets_still_emit_valid_json() {
    for budget in ["1", "40", "90"] {
        let (out, _) = run_cli(&["tests/fixtures", "--include-tests", "--max-tokens", budget]);
        serde_json::from_str::<serde_json::Value>(&out)
            .unwrap_or_else(|e| panic!("budget {budget} produced invalid JSON: {e}\n{out}"));
    }
}

#[test]
fn no_coverage_means_worst_case_and_is_labelled() {
    let (out, _) = run_cli(&[
        "tests/fixtures",
        "--include-tests",
        "--no-coverage",
        "--all",
        "--top",
        "3",
    ]);
    let parsed: serde_json::Value = serde_json::from_str(&out).expect("valid JSON");
    assert!(parsed["coverage"].is_null());
    let findings = parsed["findings"].as_array().unwrap();
    assert!(!findings.is_empty());
    for finding in findings {
        assert!(finding["cov"].is_null());
        let cc = finding["cc"].as_f64().unwrap();
        let crap = finding["crap"].as_f64().unwrap();
        // cc² + cc, within rounding
        assert!(
            (crap - (cc * cc + cc)).abs() < 0.2,
            "cc {cc} -> crap {crap}"
        );
    }
}

#[test]
fn the_threshold_filters_and_reports_what_it_hid() {
    let (all, _) = run_cli(&[
        "tests/fixtures",
        "--include-tests",
        "--no-coverage",
        "--all",
        "--top",
        "1000",
    ]);
    let (gated, _) = run_cli(&[
        "tests/fixtures",
        "--include-tests",
        "--no-coverage",
        "--top",
        "1000",
    ]);
    let all: serde_json::Value = serde_json::from_str(&all).unwrap();
    let gated: serde_json::Value = serde_json::from_str(&gated).unwrap();
    assert!(
        all["findings"].as_array().unwrap().len() > gated["findings"].as_array().unwrap().len()
    );
    assert!(gated["dropped"]["below_threshold"].as_u64().unwrap() > 0);
}

#[test]
fn a_bad_coverage_path_is_an_error_not_a_silent_zero() {
    let (_, code) = run_cli(&["tests/fixtures", "--coverage", "does-not-exist.info"]);
    assert_eq!(code, 2);
}

/// Full chain: parse -> attribute coverage -> score -> JSON. `lcov.info` marks
/// lines 2-4 covered and 6-7 not, and `sample.rs` puts `trivial` on line 2, so
/// it must come out fully covered and score exactly its complexity of 1.
#[test]
fn coverage_is_attributed_to_the_right_functions() {
    let (out, code) = run_cli(&[
        "tests/fixtures/sample.rs",
        "--coverage",
        "tests/fixtures/lcov.info",
        "--all",
        "--top",
        "100",
    ]);
    assert_eq!(code, 0);
    let parsed: serde_json::Value = serde_json::from_str(&out).expect("valid JSON");
    assert_eq!(parsed["coverage"]["format"], "lcov");

    let findings = parsed["findings"].as_array().unwrap();
    let trivial = findings
        .iter()
        .find(|f| f["name"] == "trivial")
        .expect("trivial");
    assert_eq!(trivial["cov"], 100.0);
    assert_eq!(trivial["crap"], 1.0);
    assert_eq!(trivial["sev"], "ok");

    // Past line 7 the report says nothing, which is `null`, not 0%: an
    // unmentioned function is unmeasured rather than proven untested.
    let matcher = findings
        .iter()
        .find(|f| f["name"] == "matcher")
        .expect("matcher");
    assert!(matcher["cov"].is_null());
    assert_eq!(matcher["crap"], 20.0); // cc 4 scored as uncovered
}

#[test]
fn a_missing_path_is_an_error_not_a_clean_report() {
    let (out, code) = run_cli(&["definitely/not/here"]);
    assert_eq!(code, 2, "reported {out}");
    assert!(out.is_empty());
}

#[test]
fn unparseable_files_are_reported_rather_than_scored_as_empty() {
    let dir = std::env::temp_dir().join("crap-unparsed-fixture");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("broken.rs"), "fn (((( \nif if if\n").unwrap();

    let (out, code) = run_cli(&[dir.to_str().unwrap(), "--no-coverage"]);
    assert_eq!(code, 0);
    let parsed: serde_json::Value = serde_json::from_str(&out).expect("valid JSON");
    assert_eq!(parsed["summary"]["unparsed"], 1);

    // and a clean run doesn't pay for the field at all
    let (clean, _) = run_cli(&["src", "--top", "1"]);
    let clean: serde_json::Value = serde_json::from_str(&clean).unwrap();
    assert!(clean["summary"].get("unparsed").is_none());
}

/// A patch touching the body of `outer` (line 13) must report `outer` and its
/// nested closure but leave the untouched `matcher` and `method` alone.
#[test]
fn diff_mode_scores_only_the_functions_a_patch_touched() {
    let (out, code) = run_cli(&[
        "tests/fixtures/sample.rs",
        "--diff-file",
        "tests/fixtures/changed.diff",
        "--no-coverage",
        "--all",
        "--top",
        "100",
    ]);
    assert_eq!(code, 0);
    let parsed: serde_json::Value = serde_json::from_str(&out).expect("valid JSON");

    assert_eq!(parsed["diff"]["matched_files"], 1);
    let names: Vec<&str> = parsed["findings"]
        .as_array()
        .unwrap()
        .iter()
        .map(|f| f["name"].as_str().unwrap())
        .collect();
    assert!(names.contains(&"outer"), "found {names:?}");
    assert!(
        names.contains(&"inner"),
        "the nested closure is in the changed span"
    );
    assert!(!names.contains(&"matcher"), "matcher was not touched");
    assert!(!names.contains(&"trivial"), "trivial was not touched");

    // the whole file was still scanned, and the untouched remainder is accounted for
    assert_eq!(parsed["summary"]["functions"], 7);
    assert_eq!(parsed["diff"]["functions"], 2);
    assert_eq!(parsed["dropped"]["unchanged"], 5);
    assert_eq!(parsed["coverage"], serde_json::Value::Null);
}

#[test]
fn a_patch_that_matches_nothing_reports_zero_rather_than_everything() {
    let dir = std::env::temp_dir().join("crap-unmatched-diff");
    std::fs::create_dir_all(&dir).unwrap();
    let patch = dir.join("other.diff");
    std::fs::write(&patch, "+++ b/somewhere/else.ts\n@@ -0,0 +1,1 @@\n+x\n").unwrap();

    let (out, code) = run_cli(&[
        "tests/fixtures",
        "--diff-file",
        patch.to_str().unwrap(),
        "--no-coverage",
        "--all",
    ]);
    assert_eq!(code, 0);
    let parsed: serde_json::Value = serde_json::from_str(&out).expect("valid JSON");
    assert_eq!(parsed["diff"]["files"], 1);
    assert_eq!(
        parsed["diff"]["matched_files"], 0,
        "the mismatch must be visible"
    );
    assert!(parsed["findings"].as_array().unwrap().is_empty());
}

/// The git path, end to end: a real repo, a real commit, a real change.
#[test]
fn diff_mode_reads_git_and_includes_untracked_files() {
    if Command::new("git").arg("--version").output().is_err() {
        eprintln!("skipping: git not available");
        return;
    }
    let dir = std::env::temp_dir().join("crap-git-diff-fixture");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let git = |args: &[&str]| {
        let out = Command::new("git")
            .args(args)
            .current_dir(&dir)
            .env("GIT_AUTHOR_NAME", "t")
            .env("GIT_AUTHOR_EMAIL", "t@example.com")
            .env("GIT_COMMITTER_NAME", "t")
            .env("GIT_COMMITTER_EMAIL", "t@example.com")
            .output()
            .expect("git runs");
        assert!(
            out.status.success(),
            "git {args:?}: {}",
            String::from_utf8_lossy(&out.stderr)
        );
    };
    git(&["init", "-q"]);
    std::fs::write(dir.join("clean.rs"), "fn untouched() -> i32 { 1 }\n").unwrap();
    git(&["add", "."]);
    git(&["commit", "-qm", "init"]);
    // one modified tracked file and one brand new untracked file
    std::fs::write(
        dir.join("clean.rs"),
        "fn untouched() -> i32 { 1 }\nfn edited(a: bool) -> i32 { if a { 1 } else { 0 } }\n",
    )
    .unwrap();
    std::fs::write(
        dir.join("brand_new.rs"),
        "fn fresh(a: bool, b: bool) -> bool { a && b }\n",
    )
    .unwrap();

    let (out, code) = run_cli(&[dir.to_str().unwrap(), "--diff", "--all", "--top", "100"]);
    assert_eq!(code, 0, "{out}");
    let parsed: serde_json::Value = serde_json::from_str(&out).expect("valid JSON");
    let names: Vec<&str> = parsed["findings"]
        .as_array()
        .unwrap()
        .iter()
        .map(|f| f["name"].as_str().unwrap())
        .collect();
    assert!(names.contains(&"edited"), "found {names:?}");
    assert!(
        names.contains(&"fresh"),
        "untracked files count as new: {names:?}"
    );
    assert!(!names.contains(&"untouched"), "found {names:?}");
    assert_eq!(parsed["diff"]["source"], "HEAD");
}

/// git answers some failures with its entire usage text. A wrong rev must not
/// dump that into a caller's context window.
#[test]
fn git_failures_are_quoted_briefly() {
    let dir = std::env::temp_dir().join("crap-not-a-repo");
    std::fs::create_dir_all(&dir).unwrap();
    // Skip if this machine really does have a repo at /tmp.
    if Command::new("git")
        .args(["rev-parse", "--show-toplevel"])
        .current_dir(&dir)
        .output()
        .is_ok_and(|o| o.status.success())
    {
        eprintln!("skipping: temp dir is inside a git repository");
        return;
    }

    let out = Command::new(env!("CARGO_BIN_EXE_crap"))
        .args([dir.to_str().unwrap(), "--diff"])
        .output()
        .expect("binary runs");
    assert_eq!(out.status.code(), Some(2));
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.len() < 300,
        "error output was {} bytes: {stderr}",
        stderr.len()
    );
    assert!(
        stderr.to_lowercase().contains("not a git repository"),
        "{stderr}"
    );
    assert!(
        stderr.contains("--diff-file"),
        "the error should say what to do instead"
    );
}
